#[cfg(quartz)]
extern crate block;
#[macro_use]
extern crate cfg_if;
pub extern crate libc;
#[cfg(dxgi)]
extern crate winapi;

pub use common::*;

#[cfg(quartz)]
pub mod quartz;

#[cfg(x11)]
pub mod x11;

#[cfg(all(x11, feature="wayland"))]
pub mod wayland;

#[cfg(dxgi)]
pub mod dxgi;

mod common;
