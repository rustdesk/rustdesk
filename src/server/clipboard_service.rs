use super::*;
pub use crate::clipboard::{
    check_clipboard, get_cache_msg, ClipboardContext, ClipboardSide,
    CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
};

#[derive(Default)]
struct State {
    ctx: Option<ClipboardContext>,
}

impl super::service::Reset for State {
    fn reset(&mut self) {
        crate::clipboard::reset_cache();
        self.ctx = None;
    }

    fn init(&mut self) {
        let ctx = match ClipboardContext::new(true) {
            Ok(ctx) => Some(ctx),
            Err(err) => {
                log::error!("Failed to start {}: {}", NAME, err);
                None
            }
        };
        self.ctx = ctx;
    }
}

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::repeat::<State, _, _>(&svc.clone(), INTERVAL, run);
    svc.sp
}

fn run(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
    if let Some(msg) = check_clipboard(&mut state.ctx, ClipboardSide::Host) {
        sp.send(msg);
    }
    sp.snapshot(|sps| {
        // Just create a message with multi clipboards here
        // The actual peer version and peer platform will be checked again before sending.
        if let Some(msg) = get_cache_msg("1.2.7", "Windows") {
            sps.send_shared(Arc::new(msg));
        }
        Ok(())
    })?;
    Ok(())
}
