use super::*;
pub use crate::common::{
    check_clipboard, ClipboardContext, CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
    CONTENT,
};
use clipboard_master::{CallbackResult, ClipboardHandler, Master};
use hbb_common::{anyhow, ResultType};
use std::{
    io, sync,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    time::Duration,
};

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.run::<_>(listen::run);
    sp
}

mod listen {
    use super::*;

    static RUNNING: AtomicBool = AtomicBool::new(true);
    static WAIT: Duration = Duration::from_millis(33);

    struct ClipHandle {
        tx: SyncSender<()>,
    }

    impl ClipboardHandler for ClipHandle {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            if !RUNNING.load(Ordering::SeqCst) {
                return CallbackResult::Stop;
            }

            let _ = self.tx.send(());
            CallbackResult::Next
        }

        fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
            if !RUNNING.load(Ordering::SeqCst) {
                CallbackResult::Stop
            } else {
                CallbackResult::StopWithError(error)
            }
        }
    }

    #[tokio::main]
    pub async fn run(sp: GenericService) -> ResultType<()> {
        let mut ctx = match ClipboardContext::new() {
            Ok(ctx) => ctx,
            Err(err) => {
                log::error!("Failed to start {}: {}", NAME, err);
                return Err(anyhow::Error::from(err));
            }
        };

        if !RUNNING.load(Ordering::SeqCst) {
            RUNNING.store(true, Ordering::SeqCst);
        }

        let (tx, rx) = sync::mpsc::sync_channel(12);
        let listener = tokio::spawn(async {
            log::info!("Clipboard listener running!");
            let _ = Master::new(ClipHandle { tx }).run();
        });

        check_clipboard(&mut ctx, None); // initialize CONTENT for snapshot
        while sp.ok() {
            let mut update = None;
            sp.snapshot(|sps| {
                if sps.has_subscribes() {
                    update = check_clipboard(&mut ctx, None);
                }
                // if there is update, msg will be later together,
                // otherwise it will be only sent to new subscriber,
                // but old subscribers ignored
                if update.is_none() {
                    let txt = crate::CONTENT.lock().unwrap().clone();
                    if !txt.is_empty() {
                        let msg_out = crate::create_clipboard_msg(txt);
                        sps.send_shared(Arc::new(msg_out));
                    }
                }
                Ok(())
            })?;
            if let Some(msg) = update {
                sp.send(msg);
            }

            if let Ok(_) = rx.recv_timeout(WAIT) {
                if let Some(msg) = check_clipboard(&mut ctx, None) {
                    sp.send(msg);
                }
            }
        }

        RUNNING.store(false, Ordering::SeqCst);
        trigger(&mut ctx);
        let _ = listener.await;
        log::info!("Clipboard listener stopped!");

        *CONTENT.lock().unwrap() = Default::default();
        Ok(())
    }

    fn trigger(ctx: &mut ClipboardContext) {
        let mut old_text = "".to_owned();
        let _ = match ctx.get_text() {
            Ok(text) => {
                old_text = text;
            }
            Err(_) => {}
        };
        ctx.set_text(old_text).ok();
    }
}
