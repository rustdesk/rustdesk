use hbb_common::{log, ResultType};
use std::sync::Mutex;

use crate::CliprdrServiceContext;

const CLIPBOARD_RESPONSE_WAIT_TIMEOUT_SECS: u32 = 30;

lazy_static::lazy_static! {
    static ref CONTEXT_SEND: ContextSend = ContextSend{addr: Mutex::new(None)};
}

pub struct ContextSend {
    addr: Mutex<Option<Box<dyn CliprdrServiceContext>>>,
}

impl ContextSend {
    #[inline]
    pub fn is_enabled() -> bool {
        CONTEXT_SEND.addr.lock().unwrap().is_some()
    }

    pub fn set_is_stopped() {
        let _res = Self::proc(|c| c.set_is_stopped().map_err(|e| e.into()));
    }

    pub fn enable(enabled: bool) {
        let mut lock = CONTEXT_SEND.addr.lock().unwrap();
        if enabled {
            if lock.is_none() {
                match crate::create_cliprdr_context(
                    true,
                    false,
                    CLIPBOARD_RESPONSE_WAIT_TIMEOUT_SECS,
                ) {
                    Ok(context) => {
                        log::info!("clipboard context for file transfer created.");
                        *lock = Some(context)
                    }
                    Err(err) => {
                        log::error!(
                            "Create clipboard context for file transfer: {}",
                            err.to_string()
                        );
                    }
                }
            }
        } else if let Some(_clp) = lock.take() {
            *lock = None;
            log::info!("clipboard context for file transfer destroyed.");
        }
    }

    pub fn proc<F: FnOnce(&mut Box<dyn CliprdrServiceContext>) -> ResultType<()>>(
        f: F,
    ) -> ResultType<()> {
        let mut lock = CONTEXT_SEND.addr.lock().unwrap();
        match lock.as_mut() {
            Some(context) => f(context),
            None => Ok(()),
        }
    }
}
