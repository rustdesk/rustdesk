pub mod ffi;
pub mod mediacodec;
pub use ffi::*;

#[macro_export]
macro_rules! fmt_e {
    ($($args:tt)+) => {
        Err(format!($($args)+))
    }
}

#[macro_export]
macro_rules! fmt_err {
    () => {
        |e| format!("At {}:{}: {e}", file!(), line!())
    };
}

use std::{
    sync::atomic::{AtomicBool, Ordering},
};

pub type StrResult<T = ()> = core::result::Result<T, String>;

// Simple wrapper for AtomicBool when using Ordering::Relaxed. Deref cannot be implemented (cannot
// return local reference)
pub struct RelaxedAtomic(AtomicBool);

impl RelaxedAtomic {
    pub const fn new(initial_value: bool) -> Self {
        Self(AtomicBool::new(initial_value))
    }

    pub fn value(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set(&self, value: bool) {
        self.0.store(value, Ordering::Relaxed);
    }
}
