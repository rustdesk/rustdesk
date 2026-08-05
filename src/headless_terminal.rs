mod args;

#[cfg(target_os = "macos")]
mod handler;

#[cfg(target_os = "macos")]
mod runtime;

#[cfg(target_os = "macos")]
mod tty;

pub(crate) use args::{
    classify, is_requested, usage, HeadlessTerminalArgs, HeadlessTerminalDispatch,
};

pub(crate) fn run_cli(args: &[String]) -> i32 {
    match classify(args, cfg!(target_os = "macos")) {
        HeadlessTerminalDispatch::NotRequested => {
            eprintln!("{}", usage());
            2
        }
        HeadlessTerminalDispatch::Invalid(reason) => {
            eprintln!("{reason}\n{}", usage());
            2
        }
        HeadlessTerminalDispatch::Run(parsed) => {
            #[cfg(target_os = "macos")]
            {
                runtime::run(parsed)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = parsed;
                2
            }
        }
    }
}
