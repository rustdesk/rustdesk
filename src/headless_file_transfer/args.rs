#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransferDirection {
    Push,
    Pull,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeadlessFileTransferArgs {
    pub(crate) peer_id: String,
    pub(crate) direction: TransferDirection,
    pub(crate) source: String,
    pub(crate) destination: String,
    pub(crate) force_relay: bool,
    pub(crate) overwrite: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadlessFileTransferDispatch {
    NotRequested,
    Run(HeadlessFileTransferArgs),
    Invalid(String),
}

pub(crate) const fn usage() -> &'static str {
    "Usage: RustDesk-Herbin --file-transfer --headless [--relay] [--overwrite] <peer-id> <push|pull> <source-file> <destination-file>"
}

pub(crate) fn is_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--file-transfer") && args.iter().any(|arg| arg == "--headless")
}

pub(crate) fn classify(args: &[String], is_macos: bool) -> HeadlessFileTransferDispatch {
    if !is_requested(args) {
        return HeadlessFileTransferDispatch::NotRequested;
    }
    if !is_macos {
        return HeadlessFileTransferDispatch::Invalid(
            "headless file transfer is supported by RDH on macOS only".into(),
        );
    }

    let mut force_relay = false;
    let mut overwrite = false;
    let mut positionals = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--file-transfer" | "--headless" => {}
            "--relay" if positionals.is_empty() => force_relay = true,
            "--overwrite" if positionals.is_empty() => overwrite = true,
            value if value.starts_with('-') => {
                return HeadlessFileTransferDispatch::Invalid(format!(
                    "unsupported headless file-transfer option: {value}"
                ));
            }
            value if value.is_empty() => {
                return HeadlessFileTransferDispatch::Invalid("empty argument".into());
            }
            value => positionals.push(value.to_owned()),
        }
    }
    if positionals.len() != 4 {
        return HeadlessFileTransferDispatch::Invalid(
            "headless file transfer requires peer, operation, source, and destination".into(),
        );
    }
    if positionals[0].chars().any(char::is_whitespace) {
        return HeadlessFileTransferDispatch::Invalid("invalid peer ID".into());
    }
    let direction = match positionals[1].as_str() {
        "push" => TransferDirection::Push,
        "pull" => TransferDirection::Pull,
        _ => {
            return HeadlessFileTransferDispatch::Invalid("operation must be push or pull".into());
        }
    };
    HeadlessFileTransferDispatch::Run(HeadlessFileTransferArgs {
        peer_id: positionals.remove(0),
        direction,
        source: positionals.remove(1),
        destination: positionals.remove(1),
        force_relay,
        overwrite,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn accepts_push_pull_and_optional_flags() {
        assert_eq!(
            classify(
                &args(&[
                    "--file-transfer",
                    "--headless",
                    "--relay",
                    "--overwrite",
                    "175116438",
                    "push",
                    "/tmp/a b.bin",
                    r"C:\Users\82520\a b.bin",
                ]),
                true
            ),
            HeadlessFileTransferDispatch::Run(HeadlessFileTransferArgs {
                peer_id: "175116438".into(),
                direction: TransferDirection::Push,
                source: "/tmp/a b.bin".into(),
                destination: r"C:\Users\82520\a b.bin".into(),
                force_relay: true,
                overwrite: true,
            })
        );
        assert!(matches!(
            classify(
                &args(&[
                    "--file-transfer",
                    "--headless",
                    "175116438",
                    "pull",
                    r"C:\Users\82520\a.bin",
                    "/tmp/a.bin",
                ]),
                true
            ),
            HeadlessFileTransferDispatch::Run(HeadlessFileTransferArgs {
                direction: TransferDirection::Pull,
                ..
            })
        ));
    }

    #[test]
    fn leaves_gui_and_terminal_commands_unclaimed() {
        assert_eq!(
            classify(&args(&["--file-transfer", "175116438"]), true),
            HeadlessFileTransferDispatch::NotRequested
        );
        assert_eq!(
            classify(&args(&["--terminal", "--headless", "175116438"]), true),
            HeadlessFileTransferDispatch::NotRequested
        );
    }

    #[test]
    fn rejects_invalid_or_unsafe_shapes() {
        for values in [
            vec!["--file-transfer", "--headless"],
            vec![
                "--file-transfer",
                "--headless",
                "175116438",
                "copy",
                "a",
                "b",
            ],
            vec!["--file-transfer", "--headless", "175116438", "push", "a"],
            vec![
                "--file-transfer",
                "--headless",
                "175116438",
                "push",
                "a",
                "b",
                "c",
            ],
            vec![
                "--file-transfer",
                "--headless",
                "--password",
                "secret",
                "175116438",
                "push",
                "a",
                "b",
            ],
            vec![
                "--file-transfer",
                "--headless",
                "--persistent",
                "175116438",
                "push",
                "a",
                "b",
            ],
            vec!["--file-transfer", "--headless", "bad id", "push", "a", "b"],
        ] {
            assert!(matches!(
                classify(&args(&values), true),
                HeadlessFileTransferDispatch::Invalid(_)
            ));
        }
    }

    #[test]
    fn rejects_headless_file_transfer_outside_macos() {
        assert!(matches!(
            classify(
                &args(&[
                    "--file-transfer",
                    "--headless",
                    "175116438",
                    "push",
                    "a",
                    "b",
                ]),
                false
            ),
            HeadlessFileTransferDispatch::Invalid(_)
        ));
    }
}
