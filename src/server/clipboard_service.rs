use super::*;
pub use crate::clipboard::{
    check_clipboard, ClipboardContext, CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME,
    CONTENT,
};

#[derive(Default)]
struct State {
    ctx: Option<ClipboardContext>,
}

impl super::service::Reset for State {
    fn reset(&mut self) {
        *CONTENT.lock().unwrap() = Default::default();
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
    if let Some(msg) = check_clipboard(&mut state.ctx, None) {
        sp.send(msg);
    }
    sp.snapshot(|sps| {
        let data = CONTENT.lock().unwrap().clone();
        if !data.is_empty() {
            let msg_out = data.create_msg();
            sps.send_shared(Arc::new(msg_out));
        }
        Ok(())
    })?;
    Ok(())
}
