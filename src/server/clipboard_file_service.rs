use super::*;
use clipboard::{create_cliprdr_context, get_rx_client_msg, server_msg, ConnID};
use hbb_common::{
    log,
    tokio::sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex as TokioMutex,
    },
    tokio::time::{self, Duration, Instant},
    ResultType,
};
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::common::CLIPRDR_NAME as NAME;

lazy_static::lazy_static! {
    static ref MSG_CHANNEL_SERVER: (UnboundedSender<(ConnID, Cliprdr)>, TokioMutex<UnboundedReceiver<(ConnID, Cliprdr)>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, TokioMutex::new(rx))
    };
}

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.run::<_>(listen::run);
    sp
}

pub fn handle_serve_cliprdr_msg(id: i32, msg: Cliprdr) {
    if RUNNING.load(Ordering::SeqCst) {
        log::debug!("handle handle_serve_cliprdr_msg");
        MSG_CHANNEL_SERVER
            .0
            .send((
                ConnID {
                    server_conn_id: id as u32,
                    remote_conn_id: 0,
                },
                msg,
            ))
            .unwrap();
    } else {
        // should not reach this branch
    }
}

mod listen {
    use super::*;

    static WAIT: Duration = Duration::from_millis(1500);

    #[tokio::main]
    pub async fn run(sp: GenericService) -> ResultType<()> {
        let mut cliprdr_context = create_cliprdr_context(true, false)?;

        RUNNING.store(false, Ordering::SeqCst);

        let mut timer = time::interval_at(Instant::now() + WAIT, WAIT);
        let mut client_rx = get_rx_client_msg().lock().await;
        let mut server_rx = MSG_CHANNEL_SERVER.1.lock().await;
        while sp.ok() {
            RUNNING.store(true, Ordering::SeqCst);

            tokio::select! {
                msg = client_rx.recv() => {
                    match msg {
                        Some((conn_id, msg)) => {
                            if conn_id.server_conn_id == 0 {
                                sp.send(msg)
                            } else {
                                sp.send_to(msg, conn_id.server_conn_id as i32)
                            }
                        }
                        None => {
                            // unreachable!()
                        }
                    }
                }
                msg = server_rx.recv() => {
                    match msg {
                        Some((conn_id, msg)) => {
                            let res = server_msg(&mut cliprdr_context, conn_id, msg);
                            if res != 0 {
                                // log::warn!("failed to process message for {}", id);
                            }
                        }
                        None => {
                            // unreachable!()
                        }
                    }
                }
                _ = timer.tick() => {},
            }
            sp.snapshot(|_| Ok(()))?;
        }

        RUNNING.store(false, Ordering::SeqCst);
        log::info!("Clipboard listener stopped!");

        Ok(())
    }
}
