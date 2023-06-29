use crate::cliprdr::*;
use hbb_common::log;
use std::sync::Mutex;

const CLIPBOARD_RESPONSE_WAIT_TIMEOUT_SECS: u32 = 30;

lazy_static::lazy_static! {
    static ref CONTEXT_SEND: ContextSend = ContextSend{addr: Mutex::new(0)};
}

pub struct ContextSend {
    addr: Mutex<u64>,
}

impl ContextSend {
    #[inline]
    pub fn is_enabled() -> bool {
        *CONTEXT_SEND.addr.lock().unwrap() != 0
    }

    pub fn set_is_stopped() {
        let _res = Self::proc(|c| {
            c.IsStopped = TRUE;
            0
        });
    }

    pub fn enable(enabled: bool) {
        let mut lock = CONTEXT_SEND.addr.lock().unwrap();
        if enabled {
            if *lock == 0 {
                match crate::create_cliprdr_context(
                    true,
                    false,
                    CLIPBOARD_RESPONSE_WAIT_TIMEOUT_SECS,
                ) {
                    Ok(context) => {
                        log::info!("clipboard context for file transfer created.");
                        *lock = Box::into_raw(context) as _;
                    }
                    Err(err) => {
                        log::error!(
                            "Create clipboard context for file transfer: {}",
                            err.to_string()
                        );
                    }
                }
            }
        } else {
            if *lock != 0 {
                unsafe {
                    let _ = Box::from_raw(*lock as *mut CliprdrClientContext);
                }
                log::info!("clipboard context for file transfer destroyed.");
                *lock = 0;
            }
        }
    }

    pub fn proc<F: FnOnce(&mut Box<CliprdrClientContext>) -> u32>(f: F) -> u32 {
        let lock = CONTEXT_SEND.addr.lock().unwrap();
        if *lock != 0 {
            unsafe {
                let mut context = Box::from_raw(*lock as *mut CliprdrClientContext);
                let code = f(&mut context);
                std::mem::forget(context);
                code
            }
        } else {
            0
        }
    }
}
