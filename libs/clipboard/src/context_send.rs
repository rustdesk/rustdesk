use crate::cliprdr::*;
use hbb_common::log;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref CONTEXT_SEND: Arc<Mutex<ContextSend>> = Arc::new(Mutex::new(ContextSend::new()));
}

pub struct ContextSend {
    server_enabled: bool,
    addr: u64,
}

impl ContextSend {
    fn new() -> Self {
        Self {
            server_enabled: false,
            addr: 0,
        }
    }

    #[inline]
    pub fn is_server_enabled() -> bool {
        CONTEXT_SEND.lock().unwrap().server_enabled
    }

    pub fn enable(enabled: bool, is_server_side: bool, is_server_process: bool) {
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
            if is_server_side {
                lock.server_enabled = true;
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
                lock.server_enabled = false;
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
