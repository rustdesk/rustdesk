use std::sync::mpsc::Sender;

use clipboard_master::{CallbackResult, ClipboardHandler, Master};

use super::*;
pub use crate::common::{
    check_clipboard, ClipboardContext, CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
    CONTENT,
};

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

struct ClipHandle {
    tx: Sender<bool>,
}

impl ClipboardHandler for ClipHandle {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        let _ = self.tx.send(true);
        CallbackResult::Next
    }
}

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.listen::<State, _, _>(notify, run);
    sp
}

fn notify(tx: Sender<bool>) -> ResultType<()> {
    Master::new(ClipHandle { tx }).run()?;
    Ok(())
}

fn run(sp: GenericService, state: &mut State) -> ResultType<()> {
    if let Some(ctx) = state.ctx.as_mut() {
        if let Some(msg) = check_clipboard(ctx, None) {
            sp.send(msg);
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
    Ok(())
}
