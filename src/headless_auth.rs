use std::{
    io::{self, IsTerminal, Write},
    mem::MaybeUninit,
};

use hbb_common::libc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthPrompt {
    Password { retry: bool },
    TwoFactor,
    InsecureConnection,
}

pub(crate) fn stdin_is_tty() -> bool {
    io::stdin().is_terminal()
}

pub(crate) fn get_stdin_attributes() -> io::Result<libc::termios> {
    let mut attributes = MaybeUninit::<libc::termios>::uninit();
    // SAFETY: `attributes` points to writable storage for a termios value, and
    // `STDIN_FILENO` remains owned by the process for the duration of the call.
    if unsafe { libc::tcgetattr(libc::STDIN_FILENO, attributes.as_mut_ptr()) } == -1 {
        return Err(last_stdin_error("failed to capture stdin TTY attributes"));
    }

    // SAFETY: tcgetattr returned success and initialized the termios value.
    Ok(unsafe { attributes.assume_init() })
}

pub(crate) fn set_stdin_attributes(attributes: &libc::termios) -> io::Result<()> {
    // SAFETY: `attributes` is a valid termios value and remains borrowed for
    // the duration of the tcsetattr call.
    if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, attributes) } == -1 {
        Err(last_stdin_error("failed to update stdin TTY attributes"))
    } else {
        Ok(())
    }
}

fn last_stdin_error(context: &str) -> io::Error {
    let error = io::Error::last_os_error();
    io::Error::new(error.kind(), format!("{context}: {error}"))
}

struct EchoGuard {
    snapshot: Option<libc::termios>,
}

impl EchoGuard {
    fn disable() -> io::Result<Self> {
        let snapshot = get_stdin_attributes()?;
        let mut attributes = snapshot;
        attributes.c_lflag &= !libc::ECHO;
        set_stdin_attributes(&attributes)?;
        Ok(Self {
            snapshot: Some(snapshot),
        })
    }

    fn restore(&mut self) -> io::Result<()> {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Ok(());
        };
        set_stdin_attributes(snapshot)?;
        self.snapshot.take();
        Ok(())
    }
}

impl Drop for EchoGuard {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("RDH headless CLI failed to restore stdin echo: {error}");
        }
    }
}

fn trim_line_endings(line: String) -> String {
    line.trim_end_matches(['\r', '\n']).to_owned()
}

fn secret_from_line(line: Option<String>, cancel_byte: Option<u8>) -> Option<String> {
    line.filter(|line| match cancel_byte {
        Some(cancel_byte) => line.as_bytes() != [cancel_byte].as_slice(),
        None => true,
    })
}

fn confirmation_from_line(line: Option<&str>) -> Option<bool> {
    line.map(|value| value.eq_ignore_ascii_case("y") || value.eq_ignore_ascii_case("yes"))
}

fn read_prompt_line(prompt: &str) -> io::Result<Option<String>> {
    {
        let mut stderr = io::stderr().lock();
        stderr.write_all(prompt.as_bytes())?;
        stderr.flush()?;
    }

    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        Ok(None)
    } else {
        Ok(Some(trim_line_endings(line)))
    }
}

pub(crate) fn prompt_line(prompt: &str) -> io::Result<Option<String>> {
    read_prompt_line(prompt)
}

pub(crate) fn prompt_secret(prompt: &str) -> io::Result<Option<String>> {
    prompt_secret_with_cancel(prompt, None)
}

pub(crate) fn prompt_secret_with_cancel(
    prompt: &str,
    cancel_byte: Option<u8>,
) -> io::Result<Option<String>> {
    let mut echo_guard = EchoGuard::disable()?;
    let line_result = read_prompt_line(prompt);
    let restore_result = echo_guard.restore();
    let newline_result = {
        let mut stderr = io::stderr().lock();
        stderr.write_all(b"\n").and_then(|()| stderr.flush())
    };

    let line = line_result?;
    restore_result?;
    newline_result?;
    Ok(secret_from_line(line, cancel_byte))
}

pub(crate) fn prompt_confirmation(prompt: &str) -> io::Result<Option<bool>> {
    let line = prompt_line(prompt)?;
    Ok(confirmation_from_line(line.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_trimming_preserves_non_line_ending_whitespace() {
        assert_eq!(trim_line_endings(" value \t\r\n".to_owned()), " value \t");
    }

    #[test]
    fn secret_cancel_byte_is_opt_in() {
        assert_eq!(
            secret_from_line(Some("\u{1d}".into()), None),
            Some("\u{1d}".into())
        );
        assert_eq!(secret_from_line(Some("\u{1d}".into()), Some(0x1d)), None);
        assert_eq!(
            secret_from_line(Some("\u{1d}secret".into()), Some(0x1d)),
            Some("\u{1d}secret".into())
        );
        assert_eq!(secret_from_line(Some(String::new()), Some(0x1d)), Some(String::new()));
        assert_eq!(secret_from_line(None, Some(0x1d)), None);
    }

    #[test]
    fn confirmation_is_explicit_and_case_insensitive() {
        assert_eq!(confirmation_from_line(Some("yes")), Some(true));
        assert_eq!(confirmation_from_line(Some("Y")), Some(true));
        assert_eq!(confirmation_from_line(Some("no")), Some(false));
        assert_eq!(confirmation_from_line(Some(" yes ")), Some(false));
        assert_eq!(confirmation_from_line(Some("yeah")), Some(false));
        assert_eq!(confirmation_from_line(Some("")), Some(false));
        assert_eq!(confirmation_from_line(None), None);
    }
}
