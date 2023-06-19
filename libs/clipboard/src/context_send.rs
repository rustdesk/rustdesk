use crate::cliprdr::*;
use hbb_common::log;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref CONTEXT_SEND: Arc<Mutex<ContextSend>> = Arc::new(Mutex::new(ContextSend::new()));
}

pub struct ContextSend {
    cm_enabled: bool,
    addr: u64,
}

impl ContextSend {
    fn new() -> Self {
        Self {
            cm_enabled: false,
            addr: 0,
        }
    }

    #[inline]
    pub fn is_cm_enabled() -> bool {
        CONTEXT_SEND.lock().unwrap().cm_enabled
    }

    pub fn set_is_stopped() {
        let _res = Self::proc(|c| {
            c.IsStopped = TRUE;
            0
        });
    }

    pub fn enable(enabled: bool, is_cm_side: bool, is_server_process: bool) {
        let mut lock = CONTEXT_SEND.lock().unwrap();
        if enabled {
            if lock.addr == 0 {
                match crate::create_cliprdr_context(true, false) {
                    Ok(context) => {
                        log::info!("clipboard context for file transfer created.");
                        lock.addr = Box::into_raw(context) as _;
                    }
                    Err(err) => {
                        log::error!(
                            "Create clipboard context for file transfer: {}",
                            err.to_string()
                        );
                    }
                }
            }
            if is_cm_side {
                lock.cm_enabled = true;
            }
        } else {
            if lock.addr != 0 {
                if is_server_process {
                    unsafe {
                        let _ = Box::from_raw(lock.addr as *mut CliprdrClientContext);
                    }
                    log::info!("clipboard context for file transfer destroyed.");
                    lock.addr = 0;
                }
                lock.cm_enabled = false;
            }
        }
    }

    pub fn proc<F: FnOnce(&mut Box<CliprdrClientContext>) -> u32>(f: F) -> u32 {
        let lock = CONTEXT_SEND.lock().unwrap();
        if lock.addr != 0 {
            unsafe {
                let mut context = Box::from_raw(lock.addr as *mut CliprdrClientContext);
                let code = f(&mut context);
                std::mem::forget(context);
                code
            }
        } else {
            0
        }
    }
}
