use super::*;
pub use crate::clipboard::{
    check_clipboard, ClipboardContext, ClipboardSide, CLIPBOARD_INTERVAL as INTERVAL,
    CLIPBOARD_NAME as NAME,
};
use clipboard_master::{CallbackResult, ClipboardHandler};
use std::{
    io,
    sync::mpsc::{channel, RecvTimeoutError, Sender},
    time::Duration,
};

struct Handler {
    sp: EmptyExtraFieldService,
    ctx: Option<ClipboardContext>,
    tx_cb_result: Sender<CallbackResult>,
}

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), run);
    svc.sp
}

fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    let (tx_cb_result, rx_cb_result) = channel();
    let handler = Handler {
        sp: sp.clone(),
        ctx: Some(ClipboardContext::new()?),
        tx_cb_result,
    };

    let (tx_start_res, rx_start_res) = channel();
    let h = crate::clipboard::start_clipbard_master_thread(handler, tx_start_res);
    let shutdown = match rx_start_res.recv() {
        Ok((Some(s), _)) => s,
        Ok((None, err)) => {
            bail!(err);
        }
        Err(e) => {
            bail!("Failed to create clipboard listener: {}", e);
        }
    };

    while sp.ok() {
        match rx_cb_result.recv_timeout(Duration::from_millis(INTERVAL)) {
            Ok(CallbackResult::Stop) => {
                log::debug!("Clipboard listener stopped");
                break;
            }
            Ok(CallbackResult::StopWithError(err)) => {
                bail!("Clipboard listener stopped with error: {}", err);
            }
            Err(RecvTimeoutError::Timeout) => {}
            _ => {}
        }
    }
    shutdown.signal();
    h.join().ok();

    Ok(())
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        self.sp.snapshot(|_sps| Ok(())).ok();
        if let Some(msg) = check_clipboard(&mut self.ctx, ClipboardSide::Host, false) {
            self.sp.send(msg);
        }
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
        self.tx_cb_result
            .send(CallbackResult::StopWithError(error))
            .ok();
        CallbackResult::Next
    }
}
