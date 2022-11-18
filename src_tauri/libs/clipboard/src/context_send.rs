use crate::cliprdr::*;
use hbb_common::log;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref CONTEXT_SEND: ContextSend = ContextSend{addr: Mutex::new(0)};
}

pub struct ContextSend {
    addr: Mutex<u64>,
}

impl ContextSend {
    pub fn is_enabled() -> bool {
        *CONTEXT_SEND.addr.lock().unwrap() != 0
    }

    pub fn enable(enabled: bool) {
        let mut lock = CONTEXT_SEND.addr.lock().unwrap();
        if enabled {
            if *lock == 0 {
                match crate::create_cliprdr_context(true, false, crate::ProcessSide::ClientSide) {
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
        let mut lock = CONTEXT_SEND.addr.lock().unwrap();
        if *lock != 0 {
            unsafe {
                let mut context = Box::from_raw(*lock as *mut CliprdrClientContext);
                let res = f(&mut context);
                *lock = Box::into_raw(context) as _;
                res
            }
        } else {
            0
        }
    }
}
