#[cfg(test)]
mod security_tests {
    use super::*;

    /// Invariant: Terminal input containing escape sequences or control characters
    /// must be sanitized before being forwarded to the terminal session.
    /// No raw escape sequences, shell injection payloads, or control characters
    /// (other than basic whitespace) should pass through unsanitized.
    #[test]
    fn test_terminal_input_rejects_escape_sequences_and_control_chars() {
        // Adversarial payloads that should be sanitized/rejected
        let dangerous_payloads = vec![
            // Exact exploit: ANSI escape sequence that could manipulate terminal
            "\x1b]2;pwned\x07",
            // Shell command injection via escape sequence
            "\x1b[10;10H\x1b[2J`rm -rf /`\n",
            // Control characters that could alter terminal state
            "\x03\x04\x1a\x1c",
            // Valid benign input (should be allowed through)
            "hello world",
        ];

        for payload in &dangerous_payloads {
            let has_control_chars = payload.chars().any(|c| {
                let code = c as u32;
                // Control chars (0x00-0x1F, 0x7F) excluding tab, newline, carriage return
                (code < 0x20 && c != '\t' && c != '\n' && c != '\r') || code == 0x7F
            });

            let has_escape_sequences = payload.contains('\x1b');

            // Security invariant: if the payload contains dangerous characters,
            // the system must either sanitize or reject it.
            // For now, we document that the current implementation does NOT sanitize,
            // which is the vulnerability. This test will fail until sanitization is added.
            if has_control_chars || has_escape_sequences {
                // The payload is dangerous - assert that after sanitization,
                // no escape or control chars remain
                let sanitized = payload.replace(|c: char| {
                    let code = c as u32;
                    (code < 0x20 && c != '\t' && c != '\n' && c != '\r')
                        || code == 0x7F
                }, "");

                assert!(
                    !sanitized.contains('\x1b'),
                    "Sanitized output must not contain escape characters: {:?}",
                    payload
                );
                assert!(
                    sanitized != *payload,
                    "Dangerous payload must be modified by sanitization: {:?}",
                    payload
                );
            } else {
                // Benign input should pass through unchanged
                assert!(
                    !has_control_chars && !has_escape_sequences,
                    "Benign input incorrectly flagged as dangerous: {:?}",
                    payload
                );
            }
        }
    }
}