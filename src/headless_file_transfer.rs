mod args;
mod completion;
mod error;
mod handler;
mod paths;
#[cfg(target_os = "macos")]
mod runtime;
#[cfg(target_os = "macos")]
mod signals;
mod state;

pub(crate) use args::{
    classify, is_requested, usage, HeadlessFileTransferArgs, HeadlessFileTransferDispatch,
    TransferDirection,
};
pub(crate) use error::HeadlessFileTransferError;
pub(crate) use paths::{
    inspect_pull_destination, inspect_push_source, single_regular_file_size,
    split_remote_file_path, verify_source_unchanged, FileSnapshot, RemoteFilePath,
};
pub(crate) use state::{
    RuntimeEvent, TransferAction, TransferBackend, TransferCoordinator, TransferSignal,
};

pub(crate) fn run_cli(args: &[String]) -> i32 {
    match classify(args, cfg!(target_os = "macos")) {
        HeadlessFileTransferDispatch::NotRequested => {
            eprintln!("{}", usage());
            2
        }
        HeadlessFileTransferDispatch::Invalid(reason) => {
            eprintln!("{reason}\n{}", usage());
            2
        }
        HeadlessFileTransferDispatch::Run(parsed) => {
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

#[cfg(test)]
mod tests {
    use super::run_cli;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn run_cli_returns_usage_status_for_unclaimed_or_invalid_commands() {
        assert_eq!(run_cli(&args(&["--file-transfer", "175116438"])), 2);
        assert_eq!(
            run_cli(&args(&[
                "--file-transfer",
                "--headless",
                "175116438",
                "copy",
                "source",
                "destination",
            ])),
            2
        );
    }
}
