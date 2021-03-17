use super::*;
pub use crate::common::{
    check_clipboard, ClipboardContext, CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
    CONTENT,
};

struct State {
    ctx: Option<ClipboardContext>,
    initialized: bool,
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
        Self {
            ctx,
            initialized: false,
        }
    }
}

impl super::service::Reset for State {
    fn reset(&mut self) {
        *CONTENT.lock().unwrap() = Default::default();
        self.initialized = false;
    }
}

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, false);
    sp.repeat::<State, _>(INTERVAL, run);
    sp
}

fn run(sp: GenericService, state: &mut State) -> ResultType<()> {
    if let Some(ctx) = state.ctx.as_mut() {
        if let Some(msg) = check_clipboard(ctx, None) {
            if !state.initialized {
                state.initialized = true;
                // ignore clipboard update before service start
                return Ok(());
            }
            sp.send(msg);
        }
    }
    Ok(())
}
