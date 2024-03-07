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

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::repeat::<State, _, _>(&svc.clone(), INTERVAL, run);
    svc.sp
}

fn run(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
    if let Some(msg) = check_clipboard(&mut state.ctx, None) {
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
    Ok(())
}
