use super::*;
pub use crate::common::{
    check_clipboard, ClipboardContext, CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
    CONTENT,
};
use clipboard_master::{CallbackResult, ClipboardHandler, Master};
use hbb_common::{anyhow, ResultType};
use std::{
    sync,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::Duration,
};

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);

    thread::spawn(|| {
        let (tx, rx) = sync::mpsc::channel();
        unsafe {
            listen::RECEIVER = Some(rx);
        }
        let _ = listen::notify(tx);
    });

    sp.run::<_>(listen::run);
    sp
}

mod listen {
    use super::*;

    pub(super) static mut RECEIVER: Option<Receiver<()>> = None;
    static mut CTX: Option<ClipboardContext> = None;
    static WAIT: Duration = Duration::from_millis(1000);

    struct ClipHandle {
        tx: Sender<()>,
    }

    impl ClipboardHandler for ClipHandle {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            let _ = self.tx.send(());
            CallbackResult::Next
        }
    }

    pub fn notify(tx: Sender<()>) -> ResultType<()> {
        Master::new(ClipHandle { tx }).run()?;
        Ok(())
    }

    pub fn run(sp: GenericService) -> ResultType<()> {
        if unsafe { CTX.as_ref() }.is_none() {
            match ClipboardContext::new() {
                Ok(ctx) => unsafe {
                    CTX = Some(ctx);
                },
                Err(err) => {
                    log::error!("Failed to start {}: {}", NAME, err);
                    return Err(anyhow::Error::from(err));
                }
            };
        }

        while sp.ok() {
            if let Ok(_) = unsafe { RECEIVER.as_ref() }.unwrap().recv_timeout(WAIT) {
                if let Some(mut ctx) = unsafe { CTX.as_mut() } {
                    if let Some(msg) = check_clipboard(&mut ctx, None) {
                        sp.send(msg);
                    }
                }
            }

            sp.snapshot(|sps| {
                let txt = crate::CONTENT.lock().unwrap().clone();
                if !txt.is_empty() {
                    let msg_out = crate::create_clipboard_msg(txt);
                    sps.send_shared(Arc::new(msg_out));
                }
                Ok(())
            })?;
        }

        *CONTENT.lock().unwrap() = Default::default();
        Ok(())
    }
}
