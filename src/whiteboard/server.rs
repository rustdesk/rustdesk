use super::{create_event_loop, CustomEvent};
use crate::ipc::{new_listener, Connection, Data};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use hbb_common::ResultType;
use hbb_common::{
    allow_err, log,
    tokio::{
        self,
        sync::mpsc::{unbounded_channel, UnboundedReceiver},
    },
};
use lazy_static::lazy_static;
use std::sync::RwLock;
use tao::event_loop::EventLoopProxy;

lazy_static! {
    pub(super) static ref EVENT_PROXY: RwLock<Option<EventLoopProxy<(String, CustomEvent)>>> =
        RwLock::new(None);
}

pub fn run() {
    let (tx_exit, rx_exit) = unbounded_channel();
    std::thread::spawn(move || {
        start_ipc(rx_exit);
    });
    if let Err(e) = create_event_loop() {
        log::error!("Failed to create event loop: {}", e);
        tx_exit.send(()).ok();
        return;
    }
}

#[tokio::main(flavor = "current_thread")]
async fn start_ipc(mut rx_exit: UnboundedReceiver<()>) {
    match new_listener("_whiteboard").await {
        Ok(mut incoming) => loop {
            tokio::select! {
                _ = rx_exit.recv() => {
                    log::info!("Exiting IPC");
                    break;
                }
                res = incoming.next() => match res {
                    Some(result) => match result {
                        Ok(stream) => {
                            log::debug!("Got new connection");
                            tokio::spawn(handle_new_stream(Connection::new(stream)));
                        }
                        Err(err) => {
                            log::error!("Couldn't get whiteboard client: {:?}", err);
                        }
                    },
                    None => {
                        log::error!("Failed to get whiteboard client");
                    }
                }
            }
        },
        Err(err) => {
            log::error!("Failed to start whiteboard ipc server: {}", err);
        }
    }
}

async fn handle_new_stream(mut conn: Connection) {
    loop {
        tokio::select! {
            res = conn.next() => {
                match res {
                    Err(err) => {
                        log::info!("whiteboard ipc connection closed: {}", err);
                        break;
                    }
                    Ok(Some(data)) => {
                        match data {
                            Data::Whiteboard((k, evt)) => {
                                if matches!(evt, CustomEvent::Exit) {
                                    log::info!("whiteboard ipc connection closed");
                                    break;
                                } else {
                                    EVENT_PROXY.read().unwrap().as_ref().map(|ep| {
                                        allow_err!(ep.send_event((k, evt)));
                                    });
                                }
                            }
                            _ => {

                            }
                        }
                    }
                    Ok(None) => {
                        log::info!("whiteboard ipc connection closed");
                        break;
                    }
                }
            }
        }
    }
    EVENT_PROXY.read().unwrap().as_ref().map(|ep| {
        allow_err!(ep.send_event(("".to_string(), CustomEvent::Exit)));
    });
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(super) fn get_displays_rect() -> ResultType<(i32, i32, u32, u32)> {
    let displays = crate::server::display_service::try_get_displays()?;
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for display in displays {
        let (x, y) = (display.origin().0 as i32, display.origin().1 as i32);
        let (w, h) = (display.width() as i32, display.height() as i32);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }
    let (x, y) = (min_x, min_y);
    let (w, h) = ((max_x - min_x) as u32, (max_y - min_y) as u32);
    Ok((x, y, w, h))
}
