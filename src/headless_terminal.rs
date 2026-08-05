mod args;

#[cfg(target_os = "macos")]
mod tty;

pub(crate) use args::{
    classify, is_requested, usage, HeadlessTerminalArgs, HeadlessTerminalDispatch,
};
