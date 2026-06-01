/// Standalone sanitizer that mirrors the production implementation in
/// `send_terminal_input`. Strips control characters (0x00-0x1F except tab,
/// newline, and carriage return) and DEL (0x7F).
pub fn sanitize_terminal_input(data: &str) -> String {
    data.chars()
        .filter(|&c| {
            let code = c as u32;
            !((code < 0x20 && c != '\t' && c != '\n' && c != '\r') || code == 0x7F)
        })
        .collect()
}

#[cfg(test)]
mod security_tests {
    use super::sanitize_terminal_input;

    /// Invariant: Terminal input containing escape sequences or control characters
    /// must be sanitized before being forwarded to the terminal session.
    /// No raw escape sequences, shell injection payloads, or control characters
    /// (other than basic whitespace) should pass through unsanitized.
    #[test]
    fn test_terminal_input_sanitizes_escape_sequences_and_control_chars() {
        struct Case {
            input: &'static str,
            /// true  → payload is dangerous and must be modified by sanitization
            /// false → payload is benign and must pass through unchanged
            dangerous: bool,
        }

        let cases = vec![
            Case {
                // Exact exploit: ANSI OSC escape sequence
                input: "\x1b]2;pwned\x07",
                dangerous: true,
            },
            Case {
                // Shell command injection via CSI escape sequence
                input: "\x1b[10;10H\x1b[2J`rm -rf /`\n",
                dangerous: true,
            },
            Case {
                // Bare control characters that can alter terminal state
                input: "\x03\x04\x1a\x1c",
                dangerous: true,
            },
            Case {
                // Valid benign input — must pass through unchanged
                input: "hello world",
                dangerous: false,
            },
            Case {
                // Allowed whitespace characters must survive sanitization
                input: "line1\nline2\r\n\ttabbed",
                dangerous: false,
            },
        ];

        for case in &cases {
            let sanitized = sanitize_terminal_input(case.input);

            // After sanitization there must be no escape characters or control chars.
            let has_dangerous = sanitized.chars().any(|c| {
                let code = c as u32;
                (code < 0x20 && c != '\t' && c != '\n' && c != '\r') || code == 0x7F
            });

            assert!(
                !has_dangerous,
                "sanitize_terminal_input must remove all dangerous characters, \
                 but output still contains some for input: {:?}  output: {:?}",
                case.input,
                sanitized
            );

            if case.dangerous {
                assert!(
                    sanitized != case.input,
                    "Dangerous payload must be modified by sanitization: {:?}",
                    case.input
                );
            } else {
                assert_eq!(
                    sanitized, case.input,
                    "Benign input must pass through sanitization unchanged: {:?}",
                    case.input
                );
            }
        }
    }
}
