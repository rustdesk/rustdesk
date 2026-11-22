use super::{Cursor, CustomEvent};
use crate::{
    ipc::{self, Data},
    CHILD_PROCESS,
};
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail, log, sleep,
    tokio::{
        self,
        sync::mpsc::{unbounded_channel, UnboundedSender},
        time::interval_at,
    },
    ResultType,
};
use lazy_static::lazy_static;
use std::{collections::HashMap, sync::RwLock, time::Instant};

lazy_static! {
    static ref TX_WHITEBOARD: RwLock<Option<UnboundedSender<(String, CustomEvent)>>> =
        RwLock::new(None);
    static ref CONNS: RwLock<HashMap<String, Conn>> = Default::default();
}

struct Conn {
    last_cursor_pos: (f32, f32), // For click ripple
    last_cursor_evt: LastCursorEvent,
}

struct LastCursorEvent {
    evt: Option<CustomEvent>,
    tm: Instant,
    c: usize,
}

#[inline]
pub fn get_key_cursor(conn_id: i32) -> String {
    format!("{}-cursor", conn_id)
}

pub fn register_whiteboard(k: String) {
    std::thread::spawn(|| {
        allow_err!(start_whiteboard_());
    });
    let mut conns = CONNS.write().unwrap();
    if !conns.contains_key(&k) {
        conns.insert(
            k,
            Conn {
                last_cursor_pos: (0.0, 0.0),
                last_cursor_evt: LastCursorEvent {
                    evt: None,
                    tm: Instant::now(),
                    c: 0,
                },
            },
        );
    }
}

pub fn unregister_whiteboard(k: String) {
    let mut conns = CONNS.write().unwrap();
    conns.remove(&k);
    let is_conns_empty = conns.is_empty();
    drop(conns);

    TX_WHITEBOARD.read().unwrap().as_ref().map(|tx| {
        allow_err!(tx.send((k, CustomEvent::Clear)));
    });
    if is_conns_empty {
        std::thread::spawn(|| {
            let mut whiteboard = TX_WHITEBOARD.write().unwrap();
            whiteboard.as_ref().map(|tx| {
                allow_err!(tx.send(("".to_string(), CustomEvent::Exit)));
                // Simple sleep to wait the whiteboard process exiting.
                std::thread::sleep(std::time::Duration::from_millis(3_00));
            });
            whiteboard.take();
        });
    }
}

pub fn update_whiteboard(k: String, e: CustomEvent) {
    let mut conns = CONNS.write().unwrap();
    let Some(conn) = conns.get_mut(&k) else {
        return;
    };
    match &e {
        CustomEvent::Cursor(cursor) => {
            conn.last_cursor_evt.c += 1;
            conn.last_cursor_evt.tm = Instant::now();
            if cursor.btns == 0 {
                // Send one movement event every 4.
                if conn.last_cursor_evt.c > 3 {
                    conn.last_cursor_evt.c = 0;
                    conn.last_cursor_evt.evt = None;
                    tx_send_event(conn, k, e);
                } else {
                    conn.last_cursor_evt.evt = Some(e);
                }
            } else {
                if let Some(evt) = conn.last_cursor_evt.evt.take() {
                    tx_send_event(conn, k.clone(), evt);
                    conn.last_cursor_evt.c = 0;
                }
                let click_evt = CustomEvent::Cursor(Cursor {
                    x: conn.last_cursor_pos.0,
                    y: conn.last_cursor_pos.1,
                    argb: cursor.argb,
                    btns: cursor.btns,
                    text: cursor.text.clone(),
                });
                tx_send_event(conn, k, click_evt);
            }
        }
        _ => {
            tx_send_event(conn, k, e);
        }
    }
}

#[inline]
fn tx_send_event(conn: &mut Conn, k: String, event: CustomEvent) {
    if let CustomEvent::Cursor(cursor) = &event {
        if cursor.btns == 0 {
            conn.last_cursor_pos = (cursor.x, cursor.y);
        }
    }

    TX_WHITEBOARD.read().unwrap().as_ref().map(|tx| {
        allow_err!(tx.send((k, event)));
    });
}

#[tokio::main(flavor = "current_thread")]
async fn start_whiteboard_() -> ResultType<()> {
    let mut tx_whiteboard = TX_WHITEBOARD.write().unwrap();
    if tx_whiteboard.is_some() {
        log::warn!("Whiteboard already started");
        return Ok(());
    }

    loop {
        if !crate::platform::is_prelogin() {
            break;
        }
        sleep(1.).await;
    }
    let mut stream = None;
    if let Ok(s) = ipc::connect(1000, "_whiteboard").await {
        stream = Some(s);
    } else {
        #[allow(unused_mut)]
        #[allow(unused_assignments)]
        let mut args = vec!["--whiteboard"];
        #[allow(unused_mut)]
        #[cfg(target_os = "linux")]
        let mut user = None;

        let run_done;
        if crate::platform::is_root() {
            let mut res = Ok(None);
            for _ in 0..10 {
                #[cfg(not(any(target_os = "linux")))]
                {
                    log::debug!("Start whiteboard");
                    res = crate::platform::run_as_user(args.clone());
                }
                #[cfg(target_os = "linux")]
                {
                    log::debug!("Start whiteboard");
                    res = crate::platform::run_as_user(
                        args.clone(),
                        user.clone(),
                        None::<(&str, &str)>,
                    );
                }
                if res.is_ok() {
                    break;
                }
                log::error!("Failed to run whiteboard: {res:?}");
                sleep(1.).await;
            }
            if let Some(task) = res? {
                CHILD_PROCESS.lock().unwrap().push(task);
            }
            run_done = true;
        } else {
            run_done = false;
        }
        if !run_done {
            log::debug!("Start whiteboard");
            CHILD_PROCESS.lock().unwrap().push(crate::run_me(args)?);
        }
        for _ in 0..20 {
            sleep(0.3).await;
            if let Ok(s) = ipc::connect(1000, "_whiteboard").await {
                stream = Some(s);
                break;
            }
        }
        if stream.is_none() {
            bail!("Failed to connect to connection manager");
        }
    }

    let mut stream = stream.ok_or(anyhow!("none stream"))?;
    let (tx, mut rx) = unbounded_channel();
    tx_whiteboard.replace(tx);
    drop(tx_whiteboard);
    let _call_on_ret = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let _ = TX_WHITEBOARD.write().unwrap().take();
        }),
    };

    let dur = tokio::time::Duration::from_millis(300);
    let mut timer = interval_at(tokio::time::Instant::now() + dur, dur);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            res = rx.recv() => {
                match res {
                    Some(data) => {
                        if matches!(data.1, CustomEvent::Exit) {
                            break;
                        } else {
                            allow_err!(stream.send(&Data::Whiteboard(data)).await);
                            timer.reset();
                        }
                    }
                    None => {
                        bail!("expected");
                    }
                }
            },
            _ = timer.tick() => {
                let mut conns = CONNS.write().unwrap();
                for (k, conn) in conns.iter_mut() {
                    if conn.last_cursor_evt.tm.elapsed().as_millis() > 300 {
                        if let Some(evt) = conn.last_cursor_evt.evt.take() {
                            allow_err!(stream.send(&Data::Whiteboard((k.clone(), evt))).await);
                            conn.last_cursor_evt.c = 0;
                        }
                    }
                }
            }
        }
    }
    allow_err!(
        stream
            .send(&Data::Whiteboard(("".to_string(), CustomEvent::Exit)))
            .await
    );
    Ok(())
}
