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
    thread::{self},
    time::Duration,
};

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);

    // Listening service needs to run for a long time,
    // otherwise it will cause part of the content to
    // be missed during the closure of the clipboard
    // (during the closure, the content copied by the
    // remote machine will be missed, and the clipboard
    // will not be synchronized immediately when it is
    // opened again), and CONTENT will not be updated
    thread::spawn(|| {
        let _ = listen::notify();
    });

    sp.run::<_>(listen::run);
    sp
}

mod listen {
    use super::*;

    static mut CHANNEL: Option<(Sender<()>, Receiver<()>)> = None;
    static mut CTX: Option<ClipboardContext> = None;
    static WAIT: Duration = Duration::from_millis(1500);

    struct ClipHandle;

    impl ClipboardHandler for ClipHandle {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            if let Some((tx, _rx)) = unsafe { CHANNEL.as_ref() } {
                let _ = tx.send(());
            }
            CallbackResult::Next
        }
    }

    pub fn notify() -> ResultType<()> {
        Master::new(ClipHandle).run()?;
        Ok(())
    }

    pub fn run(sp: GenericService) -> ResultType<()> {
        unsafe {
            if CHANNEL.is_none() {
                CHANNEL = Some(sync::mpsc::channel());
            }

            if CTX.is_none() {
                match ClipboardContext::new() {
                    Ok(ctx) => {
                        CTX = Some(ctx);
                    }
                    Err(err) => {
                        log::error!("Failed to start {}: {}", NAME, err);
                        return Err(anyhow::Error::from(err));
                    }
                };
            }
        }

        while sp.ok() {
            if let Some((_tx, rx)) = unsafe { CHANNEL.as_ref() } {
                if let Ok(_) = rx.recv_timeout(WAIT) {
                    if let Some(mut ctx) = unsafe { CTX.as_mut() } {
                        if let Some(msg) = check_clipboard(&mut ctx, None) {
                            sp.send(msg);
                        }
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
