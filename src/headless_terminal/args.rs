#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HeadlessTerminalArgs {
    pub(crate) peer_id: String,
    pub(crate) force_relay: bool,
    pub(crate) persistent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadlessTerminalDispatch {
    NotRequested,
    Run(HeadlessTerminalArgs),
    Invalid(String),
}

pub(crate) const fn usage() -> &'static str {
    "Usage: RustDesk-Herbin --terminal --headless [--relay] [--persistent] <peer-id>"
}

pub(crate) fn is_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--headless")
        && args
            .iter()
            .any(|arg| arg == "--terminal" || arg == "--terminal-admin")
}

pub(crate) fn classify(args: &[String], is_macos: bool) -> HeadlessTerminalDispatch {
    if !is_requested(args) {
        return HeadlessTerminalDispatch::NotRequested;
    }
    if !is_macos {
        return HeadlessTerminalDispatch::Invalid(
            "headless terminal is supported by RDH on macOS only".to_owned(),
        );
    }
    if args.iter().any(|arg| arg == "--terminal-admin") {
        return HeadlessTerminalDispatch::Invalid(
            "--terminal-admin is not supported with --headless".to_owned(),
        );
    }

    let mut force_relay = false;
    let mut persistent = false;
    let mut peer_id = None;
    for arg in args {
        match arg.as_str() {
            "--terminal" | "--headless" => {}
            "--relay" => force_relay = true,
            "--persistent" => persistent = true,
            value if value.starts_with('-') => {
                return HeadlessTerminalDispatch::Invalid(format!(
                    "unsupported headless terminal option: {value}"
                ));
            }
            value if value.trim().is_empty() || value.chars().any(char::is_whitespace) => {
                return HeadlessTerminalDispatch::Invalid("invalid peer ID".to_owned());
            }
            value => {
                if peer_id.replace(value.to_owned()).is_some() {
                    return HeadlessTerminalDispatch::Invalid(
                        "headless terminal accepts exactly one peer ID".to_owned(),
                    );
                }
            }
        }
    }

    match peer_id {
        Some(peer_id) => HeadlessTerminalDispatch::Run(HeadlessTerminalArgs {
            peer_id,
            force_relay,
            persistent,
        }),
        None => HeadlessTerminalDispatch::Invalid("missing peer ID".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn expected(force_relay: bool, persistent: bool) -> HeadlessTerminalDispatch {
        HeadlessTerminalDispatch::Run(HeadlessTerminalArgs {
            peer_id: "175116438".to_owned(),
            force_relay,
            persistent,
        })
    }

    #[test]
    fn accepts_both_supported_argument_orders() {
        assert_eq!(
            classify(&args(&["--terminal", "--headless", "175116438"]), true),
            expected(false, false)
        );
        assert_eq!(
            classify(&args(&["--terminal", "175116438", "--headless"]), true),
            expected(false, false)
        );
    }

    #[test]
    fn accepts_relay_and_persistent_in_any_flag_order() {
        assert_eq!(
            classify(
                &args(&[
                    "--persistent",
                    "--terminal",
                    "175116438",
                    "--relay",
                    "--headless",
                ]),
                true,
            ),
            expected(true, true)
        );
    }

    #[test]
    fn leaves_ordinary_terminal_and_other_commands_unclaimed() {
        assert_eq!(
            classify(&args(&["--terminal", "175116438"]), true),
            HeadlessTerminalDispatch::NotRequested
        );
        assert_eq!(
            classify(&args(&["--connect", "175116438"]), true),
            HeadlessTerminalDispatch::NotRequested
        );
    }

    #[test]
    fn rejects_invalid_headless_combinations() {
        for invalid in [
            args(&["--terminal", "--headless"]),
            args(&["--terminal", "--headless", "175116438", "other"]),
            args(&["--terminal", "--headless", "bad id"]),
            args(&[
                "--terminal",
                "--headless",
                "--password",
                "secret",
                "175116438",
            ]),
            args(&["--terminal-admin", "--headless", "175116438"]),
            args(&["--terminal", "--headless", "--unknown", "175116438"]),
        ] {
            assert!(matches!(
                classify(&invalid, true),
                HeadlessTerminalDispatch::Invalid(_)
            ));
        }
    }

    #[test]
    fn rejects_headless_terminal_outside_macos() {
        assert!(matches!(
            classify(&args(&["--terminal", "--headless", "175116438"]), false),
            HeadlessTerminalDispatch::Invalid(_)
        ));
    }
}
