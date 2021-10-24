use super::*;
pub use crate::common::{
    check_clipboard, ClipboardContext, CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
    CONTENT,
};
use clipboard_master::{CallbackResult, ClipboardHandler, Master};
use hbb_common::ResultType;
use std::{sync, sync::mpsc::Sender, thread};

struct State {
    ctx: Option<ClipboardContext>,
}

impl Default for State {
    fn default() -> Self {
        let ctx = match ClipboardContext::new() {
            Ok(ctx) => Some(ctx),
            Err(err) => {
                log::error!("Failed to start {}: {}", NAME, err);
                None
            }
        };
        Self { ctx }
    }
}

impl super::service::Reset for State {
    fn reset(&mut self) {
        *CONTENT.lock().unwrap() = Default::default();
    }
}

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.run::<_>(listen::run);
    sp
}

mod listen {
    use super::*;

    struct ClipHandle {
        tx: Sender<()>,
    }

    impl ClipboardHandler for ClipHandle {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            let _ = self.tx.send(());
            CallbackResult::Next
        }
    }

    fn notify(tx: Sender<()>) -> ResultType<()> {
        Master::new(ClipHandle { tx }).run()?;
        Ok(())
    }

    pub fn run(sp: GenericService) -> ResultType<()> {
        let mut state = State::default();
        let (tx, rx) = sync::mpsc::channel::<()>();
        thread::spawn(|| {
            let _ = notify(tx);
        });

        while sp.ok() {
            sp.snapshot(|sps| {
                let txt = crate::CONTENT.lock().unwrap().clone();
                if !txt.is_empty() {
                    let msg_out = crate::create_clipboard_msg(txt);
                    sps.send_shared(Arc::new(msg_out));
                }
                Ok(())
            })?;

            if let Ok(()) = rx.recv() {
                if let Some(ctx) = state.ctx.as_mut() {
                    if let Some(msg) = check_clipboard(ctx, None) {
                        sp.send(msg);
                    }
                }
            }
        }
        Ok(())
    }
}
