# RDH Headless Terminal CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a macOS RDH CLI command that opens an interactive RustDesk terminal to a saved peer without starting Flutter or creating a window.

**Architecture:** Add a native Rust `Session<HeadlessTerminalHandler>` frontend that reuses the existing terminal connection, authentication, encryption, and protobuf paths. Keep parsing, TTY ownership, UI-event adaptation, and lifecycle orchestration in focused files under `src/headless_terminal/`; route the combined flags before the existing Flutter URL/IPC path.

**Tech Stack:** Rust 2021, existing RustDesk `Session`/`client::io_loop`, `TerminalAction`/`TerminalResponse` protobuf, macOS termios/ioctl through `hbb_common::libc`, the existing `hbb_common::tokio::signal` re-export for event-driven terminal signals, Python RDH source-contract tests, GitHub Actions macOS arm64 candidate build.

**Delivery:** After implementation verification and required real acceptance,
open a Draft PR and stop. Independent review is outside this plan.

## Global Constraints

- Implement the macOS controller path only; the first real target is Windows peer `175116438`.
- Preserve ordinary `--terminal <peer-id>` Flutter behavior exactly.
- Accept both `--terminal --headless <peer-id>` and `--terminal <peer-id> --headless`.
- Support only interactive TTY mode, optional `--relay`, and optional `--persistent`; reject one-shot commands, pipes, `--password`, and headless `--terminal-admin`.
- Reuse RDH credential ordering, password hashing, 2FA, rendezvous, transport security, terminal service ID, and persistence format.
- Never expose plaintext credentials in argv, environment variables, process titles, stdout, stderr, or logs.
- Keep remote bytes on stdout and local prompts/diagnostics on stderr.
- Enter raw mode only after `TerminalOpened(success=true)` and restore the original TTY before every interceptable exit.
- Forward `Ctrl+C` and `Ctrl+D`; reserve `Ctrl+]` as the local detach byte.
- Do not add automatic reconnect, remote protocol changes, Flutter changes, or a hidden window.
- Preserve the existing unstaged `implementation-notes.md`; never stage or overwrite it.
- Do not modify, stage, or clean the older checkout's untracked `poc/` directory.
- Do not install or restart RDH until a verified CI artifact exists, the official RustDesk rescue connection is active, and the user explicitly approves installation.
- Any feature-branch push, CI dispatch, installation, or Draft PR is performed by the root/controller agent at its explicit boundary, not by an implementation subagent.

## File Structure

- Create `src/headless_terminal.rs`: module root, CLI entrypoint, shared error/exit mapping, and submodule registration.
- Create `src/headless_terminal/args.rs`: pure command classification and validation.
- Create `src/headless_terminal/tty.rs`: macOS TTY checks, password prompts, raw-mode guard, size reads, input splitting, and signal events.
- Create `src/headless_terminal/handler.rs`: `InvokeUiSession` adapter and typed runtime events.
- Create `src/headless_terminal/runtime.rs`: session construction, event-driven authentication, network/thread coordination, state machine, terminal actions, and shutdown.
- Modify `src/lib.rs`: register the desktop parser/module root while keeping the runtime macOS-only.
- Modify `src/core_main.rs`: bypass Flutter dispatch for the combined flags and run the headless CLI after logging is initialized.
- Modify `src/ui_session_interface.rs`: add a byte-preserving terminal-input method while retaining the existing string method.
- Modify `tests/test_herbin_branding.py`: enforce the RDH source and CI contract for the new CLI.
- Modify `.github/workflows/codex-macos-herbin.yml`: run focused headless-terminal Rust tests before the release build.
- Modify `docs/rdh-upgrade-runbook.md`: document invocation, verification, and rollback acceptance.

---

### Task 1: Parse the headless terminal command without changing Flutter dispatch

**Files:**
- Create: `src/headless_terminal.rs`
- Create: `src/headless_terminal/args.rs`
- Modify: `src/lib.rs:1-15`

**Interfaces:**
- Consumes: the complete argument vector collected by `core_main`.
- Produces: `HeadlessTerminalArgs`, `HeadlessTerminalDispatch`, `classify(args, is_macos)`, `is_requested(args)`, and `usage()`.

- [ ] **Step 1: Register an empty desktop module root**

Add this registration near `window_targeting` in `src/lib.rs`:

```rust
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod headless_terminal;
```

Create `src/headless_terminal.rs` with:

```rust
mod args;

pub(crate) use args::{classify, is_requested, usage, HeadlessTerminalArgs, HeadlessTerminalDispatch};
```

The parser remains available on every desktop target so unsupported platforms
can return a usage error instead of accidentally falling through to Flutter.
The TTY, handler, and runtime submodules added later remain macOS-only.

- [ ] **Step 2: Write parser tests before implementing the parser**

Create `src/headless_terminal/args.rs` with the following public type shells and test module. Leave `classify` and `is_requested` as `unimplemented!()` for the red run only:

```rust
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

pub(crate) fn is_requested(_args: &[String]) -> bool {
    unimplemented!()
}

pub(crate) fn classify(_args: &[String], _is_macos: bool) -> HeadlessTerminalDispatch {
    unimplemented!()
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
            args(&["--terminal", "--headless", "--password", "secret", "175116438"]),
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
```

- [ ] **Step 3: Run the parser tests and verify the red failure**

Run:

```bash
cargo test --lib headless_terminal::args::tests -- --nocapture
```

Expected: the test binary panics at `unimplemented!()` in `classify`; it must not fail because of an unrelated compile or link error.

- [ ] **Step 4: Implement the minimal pure parser**

Implement `is_requested` as exact `--headless` membership combined with either
exact `--terminal` or exact `--terminal-admin` membership. Claiming the admin
form here is necessary so it is rejected by this parser rather than falling
through to another CLI path. Implement `classify` with these rules:

```rust
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
```

Do not add core dispatch or a runtime stub in this task.

- [ ] **Step 5: Run the parser tests and formatting check**

Run:

```bash
cargo test --lib headless_terminal::args::tests -- --nocapture
cargo fmt -- --check
git diff --check
```

Expected: all parser tests pass and ordinary terminal classification remains `NotRequested`.

- [ ] **Step 6: Commit the parser slice**

```bash
git add src/lib.rs src/headless_terminal.rs src/headless_terminal/args.rs
git diff --cached --check
git diff --cached --name-status
git commit -m "feat: parse headless terminal CLI"
```

Expected staged paths: exactly the three listed files.

---

### Task 2: Preserve arbitrary terminal input bytes

**Files:**
- Modify: `src/ui_session_interface.rs:792-840`

**Interfaces:**
- Consumes: `Session<T>::send(Data::Message)` and `TerminalData`.
- Produces: `Session<T>::send_terminal_input_bytes(terminal_id: i32, data: Vec<u8>)`.

- [ ] **Step 1: Write a red byte-preservation test**

Extract message construction into a private helper beside the existing terminal methods and add this test before defining the helper:

```rust
#[cfg(test)]
mod terminal_input_tests {
    use super::*;

    #[test]
    fn terminal_input_message_preserves_non_utf8_bytes() {
        let payload = vec![0x00, 0x03, 0x1b, 0xff];
        let message = terminal_input_message(7, payload.clone());
        let action = message.terminal_action();
        let data = action.data();

        assert_eq!(data.terminal_id, 7);
        assert_eq!(data.data.as_ref(), payload.as_slice());
    }
}
```

- [ ] **Step 2: Run the focused test and verify the red compile failure**

```bash
cargo test --lib terminal_input_message_preserves_non_utf8_bytes -- --nocapture
```

Expected: compilation fails because `terminal_input_message` does not exist.

- [ ] **Step 3: Add the byte-safe helper and public session method**

Add:

```rust
fn terminal_input_message(terminal_id: i32, data: Vec<u8>) -> Message {
    let mut action = TerminalAction::new();
    action.set_data(TerminalData {
        terminal_id,
        data: bytes::Bytes::from(data),
        ..Default::default()
    });
    let mut message = Message::new();
    message.set_terminal_action(action);
    message
}
```

Replace the current string-only body with these two methods:

```rust
pub fn send_terminal_input(&self, terminal_id: i32, data: String) {
    self.send_terminal_input_bytes(terminal_id, data.into_bytes());
}

pub fn send_terminal_input_bytes(&self, terminal_id: i32, data: Vec<u8>) {
    self.send(Data::Message(terminal_input_message(terminal_id, data)));
}
```

Do not alter `open_terminal`, `resize_terminal`, or `close_terminal`.

- [ ] **Step 4: Run focused green tests and regression tests**

```bash
cargo test --lib terminal_input_message_preserves_non_utf8_bytes -- --nocapture
cargo fmt -- --check
git diff --check
```

Expected: the byte payload remains exact and the existing string API continues to compile.

- [ ] **Step 5: Commit the byte-safe terminal primitive**

```bash
git add src/ui_session_interface.rs
git diff --cached --check
git diff --cached --name-status
git commit -m "feat: preserve headless terminal input bytes"
```

---

### Task 3: Own and restore the local macOS TTY

**Files:**
- Create: `src/headless_terminal/tty.rs`
- Modify: `src/headless_terminal.rs`

**Interfaces:**
- Consumes: stdin/stdout file descriptors, macOS termios/ioctl, process signals.
- Produces: `LocalTtyGuard`, `SignalForwarder`, `TtySize`, `InputChunk`, `SignalEvent`, and line/secret/confirmation prompts.

- [ ] **Step 1: Reuse the existing Tokio signal runtime**

Use `hbb_common::tokio::signal::unix::{signal, SignalKind}`. The workspace's
`hbb_common` dependency already enables Tokio's full feature set and re-exports
Tokio, so this task must not modify `Cargo.toml` or `Cargo.lock`. Confirm the
boundary before coding:

```bash
rg -n 'pub use tokio' libs/hbb_common/src/lib.rs
rg -n 'tokio = .*features = \["full"\]' libs/hbb_common/Cargo.toml
git diff --exit-code -- Cargo.toml Cargo.lock
```

Expected: both source checks match and the dependency files are unchanged.

- [ ] **Step 2: Write pure input-splitting and exit-mapping tests**

Create `src/headless_terminal/tty.rs` with type shells and these tests:

```rust
pub(crate) const DETACH_BYTE: u8 = 0x1d;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TtySize {
    pub(crate) rows: u16,
    pub(crate) cols: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InputChunk {
    Data(Vec<u8>),
    Detach,
}

pub(crate) fn split_input(_bytes: &[u8]) -> Vec<InputChunk> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_input_forwards_control_bytes_except_detach() {
        assert_eq!(
            split_input(&[b'a', 0x03, 0x04, b'b']),
            vec![InputChunk::Data(vec![b'a', 0x03, 0x04, b'b'])]
        );
    }

    #[test]
    fn split_input_stops_at_local_detach_byte() {
        assert_eq!(
            split_input(&[b'a', b'b', DETACH_BYTE, b'c']),
            vec![InputChunk::Data(vec![b'a', b'b']), InputChunk::Detach]
        );
    }

    #[test]
    fn split_input_emits_only_detach_when_escape_is_first() {
        assert_eq!(split_input(&[DETACH_BYTE]), vec![InputChunk::Detach]);
    }
}
```

- [ ] **Step 3: Run the input tests and verify the red failure**

```bash
cargo test --lib headless_terminal::tty::tests -- --nocapture
```

Expected: panic at `split_input`'s `unimplemented!()`.

- [ ] **Step 4: Implement input splitting and testable TTY boundaries**

Implement `split_input` so bytes before the first detach byte form at most one
`Data` chunk, followed by exactly one `Detach`; discard bytes after detach.

Define a syscall boundary:

```rust
pub(crate) trait TtyBackend: Send + Sync + 'static {
    type Snapshot: Send + 'static;
    fn stdin_is_tty(&self) -> bool;
    fn stdout_is_tty(&self) -> bool;
    fn capture(&self) -> io::Result<Self::Snapshot>;
    fn enter_raw(&self, snapshot: &Self::Snapshot) -> io::Result<()>;
    fn restore(&self, snapshot: &Self::Snapshot) -> io::Result<()>;
    fn size(&self) -> io::Result<TtySize>;
}
```

Add `SystemTtyBackend` using `std::io::IsTerminal`,
`hbb_common::libc::tcgetattr`, `cfmakeraw`, `tcsetattr`, and
`ioctl(TIOCGWINSZ)`. Reject zero rows or columns with `io::ErrorKind::InvalidData`.
Register it from the module root without exposing it on other targets:

```rust
#[cfg(target_os = "macos")]
mod tty;
```

Add:

```rust
pub(crate) struct LocalTtyGuard<B: TtyBackend> {
    backend: Arc<B>,
    snapshot: Option<B::Snapshot>,
}
```

`LocalTtyGuard::enter(backend)` captures once, enters raw mode, and stores the
snapshot only after success. `restore(&mut self)` is idempotent. `Drop` calls
`restore` and writes a restoration failure to stderr without panicking.

- [ ] **Step 5: Add fake-backend restoration tests**

Use a fake backend with atomic capture/raw/restore counters and an injected raw
failure. Expose a `counts() -> (usize, usize, usize)` helper, then add these
complete assertions:

```rust
#[test]
fn tty_guard_restores_exactly_once_on_drop() {
    let backend = Arc::new(FakeTtyBackend::succeeding());
    {
        let _guard = LocalTtyGuard::enter(backend.clone()).unwrap();
    }
    assert_eq!(backend.counts(), (1, 1, 1));
}

#[test]
fn tty_guard_does_not_restore_when_enter_raw_fails() {
    let backend = Arc::new(FakeTtyBackend::failing_raw());
    assert!(LocalTtyGuard::enter(backend.clone()).is_err());
    assert_eq!(backend.counts(), (1, 1, 0));
}

#[test]
fn explicit_restore_makes_drop_idempotent() {
    let backend = Arc::new(FakeTtyBackend::succeeding());
    {
        let mut guard = LocalTtyGuard::enter(backend.clone()).unwrap();
        guard.restore().unwrap();
        assert_eq!(backend.counts(), (1, 1, 1));
    }
    assert_eq!(backend.counts(), (1, 1, 1));
}
```

- [ ] **Step 6: Add prompt and signal helpers**

Add:

```rust
pub(crate) fn prompt_line(prompt: &str) -> io::Result<Option<String>>
pub(crate) fn prompt_secret(prompt: &str) -> io::Result<Option<String>>
pub(crate) fn prompt_confirmation(prompt: &str) -> io::Result<bool>

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignalEvent {
    Resize,
    Shutdown,
}

pub(crate) struct SignalForwarder {
    shutdown_tx: Option<hbb_common::tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

pub(crate) fn spawn_signal_forwarder<F>(emit: F) -> io::Result<SignalForwarder>
where
    F: FnMut(SignalEvent) + Send + 'static,
{
    use hbb_common::tokio;
    use tokio::signal::unix::{signal, SignalKind};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let runtime_guard = runtime.enter();
    let mut resize = signal(SignalKind::window_change())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut hangup = signal(SignalKind::hangup())?;
    drop(runtime_guard);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let thread = std::thread::spawn(move || runtime.block_on(async move {
        let mut emit = emit;
        tokio::select! {
            biased;
            _ = shutdown_rx => {}
            _ = terminate.recv() => emit(SignalEvent::Shutdown),
            _ = hangup.recv() => emit(SignalEvent::Shutdown),
            _ = async {
                loop {
                    if resize.recv().await.is_none() {
                        break;
                    }
                    emit(SignalEvent::Resize);
                }
            } => {}
        }
    }));
    Ok(SignalForwarder {
        shutdown_tx: Some(shutdown_tx),
        thread: Some(thread),
    })
}

impl Drop for SignalForwarder {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("RDH headless terminal signal thread panicked");
            }
        }
    }
}
```

`prompt_line` writes the prompt to stderr, reads stdin in canonical mode,
returns `None` for EOF, and trims only trailing CR/LF. `prompt_secret` uses the
same contract while temporarily clearing only `ECHO`, restoring echo through
its own guard, printing a newline to stderr, and returning `None` when the
trimmed line consists of the single `DETACH_BYTE` (the user submits `Ctrl+]`
with Enter before raw mode). `prompt_confirmation` accepts case-insensitive
`y` or `yes`; empty input and all other values return false.

The signal runtime listens for `SIGWINCH`, `SIGTERM`, and `SIGHUP`. It sends
`Resize` for `SIGWINCH` and one `Shutdown` before terminating for either of the
other two. It must not register `SIGINT`, because raw-mode `Ctrl+C` is remote
input. `SignalForwarder::drop` sends the oneshot shutdown and joins the signal
thread so a completed CLI invocation does not leave a blocked helper thread
behind.

- [ ] **Step 7: Run focused TTY tests and formatting**

```bash
cargo test --lib headless_terminal::tty::tests -- --nocapture
cargo fmt -- --check
git diff --check
git diff --exit-code -- Cargo.toml Cargo.lock
```

Expected: all fake-backend and pure input tests pass; dependency manifests and lockfile remain byte-identical.

- [ ] **Step 8: Commit the TTY slice**

```bash
git add src/headless_terminal.rs src/headless_terminal/tty.rs
git diff --cached --check
git diff --cached --name-status
git commit -m "feat: manage headless terminal TTY"
```

---

### Task 4: Adapt RustDesk UI callbacks into typed headless events

**Files:**
- Create: `src/headless_terminal/handler.rs`
- Modify: `src/headless_terminal.rs`

**Interfaces:**
- Consumes: `InvokeUiSession`, `TerminalResponse`, UI message-box types, and `ConnType::TERMINAL` connection notifications.
- Produces: `HeadlessTerminalHandler`, `HeadlessEvent`, `AuthPrompt`, and byte-preserving terminal events.

- [ ] **Step 1: Define typed events and write red response tests**

Create these types:

```rust
// In src/headless_terminal.rs
#[cfg(target_os = "macos")]
mod handler;

// In src/headless_terminal/handler.rs
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthPrompt {
    Password { retry: bool },
    TwoFactor,
    InsecureConnection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadlessEvent {
    Connected,
    Auth(AuthPrompt),
    Opened {
        terminal_id: i32,
        success: bool,
        message: String,
        pid: u32,
        service_id: String,
        persistent_sessions: Vec<i32>,
        replay_terminal_output: bool,
    },
    Output {
        terminal_id: i32,
        data: Vec<u8>,
    },
    Closed {
        terminal_id: i32,
        exit_code: i32,
    },
    Failed {
        terminal_id: i32,
        message: String,
    },
    ConnectionFailed(String),
}
```

Add tests that construct `TerminalResponse` values for opened, compressed and
uncompressed data, closed, and error variants, call
`handler.handle_terminal_response(response)`, and assert the exact event
received from `std::sync::mpsc::Receiver<HeadlessEvent>`.

The compressed test must compress a byte vector containing ANSI escape bytes
and assert the decompressed event bytes match exactly. The opened test must set
and assert `pid`, `service_id`, `persistent_sessions`, and
`replay_terminal_output`; these fields are required for persistent-session
acceptance rather than being discarded as GUI-only metadata.

- [ ] **Step 2: Run the handler tests and verify the red compile failure**

```bash
cargo test --lib headless_terminal::handler::tests -- --nocapture
```

Expected: compilation fails because `HeadlessTerminalHandler` and its callback implementation do not exist.

- [ ] **Step 3: Implement the event sender and relevant callbacks**

Define:

```rust
#[derive(Clone, Default)]
pub(crate) struct HeadlessTerminalHandler {
    event_tx: Arc<Mutex<Option<std::sync::mpsc::Sender<HeadlessEvent>>>>,
}
```

`new(tx)` stores the sender. A private `emit(event)` returns `bool`; if the
receiver is absent or closed, log a contextual error without panicking.

In `InvokeUiSession`:

- `on_connected(ConnType::TERMINAL)` emits `Connected`;
- `handle_terminal_response` maps all four known variants and decompresses data
  exactly like `FlutterHandler`, including every opened metadata field above;
- `msgbox` maps `input-password` to `Password { retry: false }`,
  `re-input-password` to `Password { retry: true }`, `input-2fa` to
  `TwoFactor`, and insecure-connection message types to `InsecureConnection`;
- other error message boxes emit `ConnectionFailed` using non-empty `text`,
  then non-empty `title`, then `msgtype` as the fallback message;
- `set_connection_type`, `set_fingerprint`, and unrelated UI callbacks do not
  emit terminal state changes.

Implement every remaining required `InvokeUiSession` method with an explicit
no-op or neutral return matching the trait signature. Use
`std::ptr::null()` for `get_rgba`. Do not add default method bodies to the
shared trait.

- [ ] **Step 4: Add prompt-mapping and no-spurious-event tests**

Add tests for each mapped prompt and assert that calling representative no-op
callbacks (`set_cursor_id`, `update_quality_status`, `next_rgba`) leaves the
event receiver empty.

- [ ] **Step 5: Run focused green tests and trait regression compilation**

```bash
cargo test --lib headless_terminal::handler::tests -- --nocapture
cargo fmt -- --check
git diff --check
```

Expected: handler tests pass and the unchanged `InvokeUiSession` trait still
compiles through the normal library target.

- [ ] **Step 6: Commit the handler slice**

```bash
git add src/headless_terminal.rs src/headless_terminal/handler.rs
git diff --cached --check
git diff --cached --name-status
git commit -m "feat: adapt terminal events for headless CLI"
```

---

### Task 5: Run the interactive headless terminal lifecycle

**Files:**
- Create: `src/headless_terminal/runtime.rs`
- Modify: `src/headless_terminal.rs`
- Modify: `src/core_main.rs:42-123, 149-170, 970-1021`

**Interfaces:**
- Consumes: `HeadlessTerminalArgs`, `HeadlessEvent`, `LocalTtyGuard`, `Session<HeadlessTerminalHandler>`, `io_loop`, and byte-safe terminal actions.
- Produces: desktop-safe `run_cli(args: &[String]) -> i32`, a macOS
  `runtime::run(HeadlessTerminalArgs)`, deterministic state transitions,
  stdout/stderr separation, and process exit mapping.

- [ ] **Step 1: Write the runtime state-machine tests before implementation**

Define shells for:

```rust
const TERMINAL_ID: i32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Authenticating,
    Opening,
    Active,
    Closing,
    Closed,
    Failed,
}

struct RuntimeState {
    phase: Phase,
    persistent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SessionAction {
    OpenTerminal { terminal_id: i32, rows: u32, cols: u32 },
    SendInput { terminal_id: i32, data: Vec<u8> },
    ResizeTerminal { terminal_id: i32, rows: u32, cols: u32 },
    CloseTerminal { terminal_id: i32 },
    CloseTransport,
}
```

Expose focused state methods (`connected`, `opened`, `output`, `begin_close`,
`closed`, and `fail`) and a pure `detach_actions`. Add these complete state
assertions:

```rust
#[test]
fn successful_flow_is_authenticating_opening_active_closed() {
    let mut state = RuntimeState::new(false);
    assert_eq!(state.phase, Phase::Authenticating);
    state.connected().unwrap();
    assert_eq!(state.phase, Phase::Opening);
    state.opened(TERMINAL_ID, true).unwrap();
    assert_eq!(state.phase, Phase::Active);
    state.closed(TERMINAL_ID).unwrap();
    assert_eq!(state.phase, Phase::Closed);
}

#[test]
fn data_before_opened_is_a_protocol_failure() {
    let mut state = RuntimeState::new(false);
    assert!(state.output(TERMINAL_ID).is_err());
    assert_eq!(state.phase, Phase::Failed);
}

#[test]
fn wrong_terminal_id_is_a_protocol_failure() {
    let mut state = RuntimeState::new(false);
    state.connected().unwrap();
    assert!(state.opened(2, true).is_err());
    assert_eq!(state.phase, Phase::Failed);
}

#[test]
fn duplicate_opened_or_closed_is_rejected() {
    let mut state = RuntimeState::new(false);
    state.connected().unwrap();
    state.opened(TERMINAL_ID, true).unwrap();
    assert!(state.opened(TERMINAL_ID, true).is_err());

    let mut state = RuntimeState::new(false);
    state.connected().unwrap();
    state.opened(TERMINAL_ID, true).unwrap();
    state.closed(TERMINAL_ID).unwrap();
    assert!(state.closed(TERMINAL_ID).is_err());
}

#[test]
fn detach_closes_terminal_only_when_not_persistent() {
    assert_eq!(
        detach_actions(false),
        vec![
            SessionAction::CloseTerminal { terminal_id: TERMINAL_ID },
            SessionAction::CloseTransport,
        ]
    );
    assert_eq!(
        detach_actions(true),
        vec![SessionAction::CloseTransport]
    );
}

#[test]
fn remote_exit_status_maps_to_local_contract() {
    assert_eq!(local_exit_status(0), 0);
    assert_eq!(local_exit_status(1), 1);
    assert_eq!(local_exit_status(125), 125);
    assert_eq!(local_exit_status(126), 1);
    assert_eq!(local_exit_status(-1), 1);
}
```

- [ ] **Step 2: Run the state tests and verify the red failure**

```bash
cargo test --lib headless_terminal::runtime::tests -- --nocapture
```

Expected: compilation fails because runtime transitions and `local_exit_status` are undefined.

- [ ] **Step 3: Implement the state machine and error type**

Add `HeadlessTerminalError` variants for Usage, Tty, Authentication,
Connection, and Protocol. Implement `Display` without embedding credentials.
Map them to statuses 2, 3, 4, 5, and 5 respectively.

Implement state methods that accept only these sequences:

```text
Authenticating -> Opening -> Active -> Closed
                              |-> Closing -> Closed or transport completion
          any nonterminal state -> Failed
```

`Connected` moves to Opening. Successful `Opened` for terminal ID 1 moves to
Active. Failed `Opened`, `Failed`, or connection errors move to Failed. Output
is valid only in Active. Closed is valid in Active or Closing. Duplicate or
wrong-ID terminal events are protocol failures; no branch silently resets the
state or starts another connection round.

- [ ] **Step 4: Construct the session and preserve RDH's authentication order**

Before constructing a session or spawning a thread, require
`stdin_is_tty() && stdout_is_tty()` and successfully read a non-zero terminal
size. Return TTY status 3 without starting network I/O when either descriptor
is not interactive or the size lookup fails. Add separate fake-backend tests
for non-TTY stdin, non-TTY stdout, zero rows/columns, and size syscall failure.

Construct the session with shared channels and explicit fields:

```rust
let session: Session<HeadlessTerminalHandler> = Session {
    password: String::new(),
    ui_handler: HeadlessTerminalHandler::new(event_tx),
    server_keyboard_enabled: Arc::new(RwLock::new(true)),
    server_file_transfer_enabled: Arc::new(RwLock::new(true)),
    server_clipboard_enabled: Arc::new(RwLock::new(true)),
    reconnect_count: Arc::new(AtomicUsize::new(0)),
    ..Default::default()
};
session.lc.write().unwrap().initialize(
    args.peer_id.clone(),
    ConnType::TERMINAL,
    None,
    args.force_relay,
    None,
    None,
    None,
);
session
    .lc
    .write()
    .unwrap()
    .get_config()
    .terminal_persistent
    .v = args.persistent;
```

Do not pre-read, copy, or reinterpret saved credentials. Leaving
`Session.password` empty makes existing `client::handle_hash` try the live
connection token, peer config, personal address book, and configured default
password in RDH's current order. It emits `input-password` only when those
sources are unavailable, and `re-input-password` when a submitted/saved value
is rejected.

Track `password_submission_pending: bool` in the runtime. On
`Password { retry: false }`, prompt only when the flag is false; call
`prompt_secret("Password: ")`, ask
`prompt_confirmation("Save password for this peer? [y/N] ")`, then move the
owned password into `session.login(String::new(), String::new(), password,
remember)` and set the flag true. Ignore a duplicate non-retry password event
while a submission is pending. On `Password { retry: true }`, clear the flag
and prompt again. Clear it on `Connected`. Password prompts are valid only in
`Authenticating`, before raw mode exists; an auth prompt after `Opened` is a
protocol failure. Do not log or mirror the password and do not add a second
plaintext copy to `Session.password`.

For `TwoFactor`, call `prompt_line("2FA code: ")` and
`session.send2fa(code, false)`. For `InsecureConnection`, call
`session.continue_insecure_connection(false)`; this sends
`Data::RejectInsecureConnection` without offering a bypass. If a prompt returns
EOF, send `Data::Close` and return authentication status 4.

Set `terminal_persistent.v` directly on the initialized in-memory peer config
for this connection, to exactly `args.persistent`. Do not call `toggle_option`
and do not persist this CLI override into the peer's saved GUI preference.

- [ ] **Step 5: Implement the active runtime loop**

Define one coordinator-only event type:

```rust
enum RuntimeEvent {
    Remote(HeadlessEvent),
    Input(InputChunk),
    Signal(SignalEvent),
    StdinClosed(Option<String>),
    TransportClosed,
}
```

Create a `std::sync::mpsc` channel for `HeadlessEvent` and a second one for
`RuntimeEvent`. A small adapter thread forwards handler events as
`RuntimeEvent::Remote`; stdin and the signal callback send their variants
directly to the runtime channel. Start `io_loop` in exactly one network thread
with one `connection_round_state.new_round()`. When `io_loop` returns, that
thread sends `RuntimeEvent::TransportClosed`; it never starts a replacement
round.

On `Connected`, read the current size and call:

```rust
session.open_terminal(TERMINAL_ID, size.rows.into(), size.cols.into());
```

On successful Opened, write one credential-free diagnostic to stderr containing
the remote `pid`, `service_id`, and `replay_terminal_output`, enter raw mode,
spawn one stdin reader thread, and retain one `SignalForwarder` guard. The stdin
reader sends `InputChunk` values to the runtime channel; EOF sends
`StdinClosed(None)` and a read error sends `StdinClosed(Some(error))`. It must
not write the RustDesk sender directly. Treat non-empty
`persistent_sessions` metadata as informational; terminal ID 1 is the seed
session that the existing server remaps to the lowest surviving PTY on a
persistent reconnect.

Runtime actions are:

- `InputChunk::Data(bytes)` -> `send_terminal_input_bytes`;
- `InputChunk::Detach` -> restore the TTY, enter Closing, if non-persistent call
  `close_terminal`, then send `Data::Close` and wait for `TransportClosed`;
- `SignalEvent::Resize` -> query latest size and call `resize_terminal` only if
  it differs from the last sent size;
- `SignalEvent::Shutdown` -> same close/detach policy as `Ctrl+]`;
- `StdinClosed(None)` -> same policy as local detach;
- `StdinClosed(Some(error))` -> restore the TTY, report a local TTY failure,
  send `Data::Close`, and finish with status 3;
- `HeadlessEvent::Output` -> `stdout.write_all` followed by `flush`;
- `HeadlessEvent::Closed` -> store `local_exit_status`, restore the TTY, send
  `Data::Close`, and wait for `TransportClosed`;
- `TransportClosed` in Closing or Closed -> return the stored local result;
- unexpected `TransportClosed` -> restore the TTY and return connection status 5;
- failure event -> restore TTY, print one stderr line, and return status 4 or 5.

Do not join a stdin thread blocked in `read`; returning from the CLI after the
guard is dropped terminates the process. Dropping `SignalForwarder` does close
and join its own signal thread. Do not call `std::process::exit` while
`LocalTtyGuard` or `SignalForwarder` is alive.

- [ ] **Step 6: Add channel-driven integration tests**

Use fake TTY and fake session-action adapters to assert exact ordered actions
for:

```text
Connected -> OpenTerminal(1, rows, cols)
Opened -> EnterRaw
Data -> stdout bytes only
Resize -> ResizeTerminal only after size changed
Ctrl+] default -> CloseTerminal then CloseTransport
Ctrl+] persistent -> CloseTransport only
Closed(7) -> RestoreTty then Exit(7)
TransportClosed before Opened -> RestoreTty then Exit(5)
TransportClosed while Active -> RestoreTty then stderr then Exit(5)
Connection failure while Opening -> RestoreTty then stderr then Exit(5)
Connection failure while Active -> RestoreTty then stderr then Exit(5)
Closed(0) -> RestoreTty then Exit(0)
Closed(125) -> RestoreTty then Exit(125)
Closed(126) -> RestoreTty then Exit(1)
```

Add a test whose remote output contains invalid UTF-8 and ANSI bytes; compare
the fake stdout byte vector exactly and assert fake stderr contains no remote
bytes. Add authentication tests proving:

```text
saved credential success -> no prompt and OpenTerminal
first Password -> PromptSecret, PromptRemember, Login exactly once
duplicate non-retry Password while pending -> no second prompt or Login
retry Password -> a second prompt and Login
TwoFactor -> PromptLine then Send2fa(code, false)
InsecureConnection -> RejectInsecureConnection
```

Add a session-construction test around a focused
`apply_cli_persistence(&mut LoginConfigHandler, bool)` helper. Begin with an
in-memory peer config whose GUI terminal-persistent setting is true, apply both
CLI modes to separate instances, and assert false without `--persistent` and
true with it. The helper only mutates `get_config().terminal_persistent.v`; it
must not call `toggle_option`, `save_config`, or `PeerConfig::store`.

- [ ] **Step 7: Integrate core dispatch with red classification tests**

In `src/core_main.rs`, add a pure helper:

```rust
fn should_dispatch_flutter_connection(args: &[String], requested: bool) -> bool {
    requested && !crate::headless_terminal::is_requested(args)
}
```

Add tests proving ordinary terminal returns true and both headless argument
orders return false. Also prove `--terminal-admin --headless` is claimed for a
usage error rather than falling through. Run them before changing production
dispatch; expected failure is the missing helper.

- [ ] **Step 8: Route headless CLI before Flutter and after log initialization**

Use `should_dispatch_flutter_connection(&args,
_is_flutter_invoke_new_connection)` at the existing line that calls
`core_main_invoke_new_connection`. After `init_log` and before other CLI
handlers, add on every desktop target:

```rust
if crate::headless_terminal::is_requested(&args) {
    let exit_code = crate::headless_terminal::run_cli(&args);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    return None;
}
```

Register `runtime` only on macOS and add the desktop-safe root function:

```rust
#[cfg(target_os = "macos")]
mod runtime;

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
```

`classify(args, cfg!(target_os = "macos"))` rejects Run before the non-macOS
fallback is reached. This keeps Linux/Windows builds type-safe while ensuring
the combined flags never open Flutter there.

- [ ] **Step 9: Run all headless tests and existing CLI regressions**

```bash
cargo test --lib headless_terminal -- --nocapture
cargo test --lib core_main::tests -- --nocapture
cargo test --lib terminal_input_message_preserves_non_utf8_bytes -- --nocapture
cargo fmt -- --check
git diff --check
```

Expected: all focused tests pass; ordinary `--terminal` remains classified for Flutter.

- [ ] **Step 10: Commit the runnable CLI slice**

```bash
git add \
  src/core_main.rs \
  src/headless_terminal.rs \
  src/headless_terminal/runtime.rs
git diff --cached --check
git diff --cached --name-status
git commit -m "feat: run headless terminal sessions"
```

Do not stage `implementation-notes.md`.

---

### Task 6: Enforce CI, documentation, and aggregate source contracts

**Files:**
- Modify: `tests/test_herbin_branding.py`
- Modify: `.github/workflows/codex-macos-herbin.yml:40-120`
- Modify: `docs/rdh-upgrade-runbook.md`

**Interfaces:**
- Consumes: committed headless CLI source and existing RDH candidate workflow.
- Produces: source-contract assertions, macOS CI test gate, operator instructions, and a verified local source boundary.

- [ ] **Step 1: Add red RDH source-contract assertions**

Read these files in `main()`:

```python
headless_root_rs = read("src/headless_terminal.rs")
headless_args_rs = read("src/headless_terminal/args.rs")
headless_tty_rs = read("src/headless_terminal/tty.rs")
headless_handler_rs = read("src/headless_terminal/handler.rs")
headless_runtime_rs = read("src/headless_terminal/runtime.rs")
core_main_rs = read("src/core_main.rs")
```

Add assertions:

```python
assert 'mod headless_terminal;' in lib_rs
assert '"--headless"' in headless_args_rs
assert '"--persistent"' in headless_args_rs
assert '"--password"' in headless_args_rs
assert "send_terminal_input_bytes" in headless_runtime_rs
assert "LocalTtyGuard" in headless_tty_rs
assert "SignalKind::window_change" in headless_tty_rs
assert "HeadlessTerminalHandler" in headless_handler_rs
assert "core_main_invoke_new_connection" in core_main_rs
assert "terminal_persistent" in headless_runtime_rs
assert "toggle_option" not in headless_runtime_rs
assert "PeerConfig::store" not in headless_runtime_rs
assert_in_order(
    core_main_rs,
    "should_dispatch_flutter_connection(&args",
    "core_main_invoke_new_connection",
)
assert_in_order(
    core_main_rs,
    "hbb_common::init_log",
    "headless_terminal::run_cli(&args)",
)
assert "cargo test --locked --lib headless_terminal" in macos_workflow
```

The `--password` assertion confirms explicit parser rejection, not acceptance.

- [ ] **Step 2: Run the contract and verify its red CI assertion**

```bash
python3 tests/test_herbin_branding.py
```

Expected: failure because the workflow does not yet run the focused Rust tests.

- [ ] **Step 3: Add the focused macOS CI test step**

After vcpkg dependencies are installed and before the release build, add:

```yaml
      - name: Test RDH headless terminal CLI
        run: |
          MACOSX_DEPLOYMENT_TARGET=10.14 \
            cargo test --locked --lib headless_terminal \
              --features flutter,hwcodec,unix-file-copy-paste,screencapturekit \
              -- --nocapture
```

Do not add a second dependency installation path or a local-machine build requirement.

- [ ] **Step 4: Document operator usage and acceptance**

Add a `Headless terminal CLI` section to `docs/rdh-upgrade-runbook.md` with:

```bash
/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin \
  --terminal --headless 175116438
```

Document `--relay`, `--persistent`, `Ctrl+]`, stdout/stderr separation, statuses
0 through 5, the TTY requirement, and the fact that ordinary `--terminal`
still opens Flutter. State explicitly that `--password` is rejected and that
the command prompts securely when no stored credential is available.

- [ ] **Step 5: Run the aggregate local verification gate**

```bash
python3 tests/test_herbin_branding.py
cargo test --lib headless_terminal -- --nocapture
cargo test --lib core_main::tests -- --nocapture
cargo test --lib terminal_input_message_preserves_non_utf8_bytes -- --nocapture
cargo fmt -- --check
git diff --check
git diff --cached --check
git status --short
```

Expected:

- every command exits 0;
- only planned headless-terminal files and the pre-existing unstaged
  `implementation-notes.md` appear;
- no Flutter source, terminal protobuf, server terminal service, Windows
  helper, installed app, or `poc/` path changed.

- [ ] **Step 6: Commit the CI and operator contract**

```bash
git add \
  .github/workflows/codex-macos-herbin.yml \
  docs/rdh-upgrade-runbook.md \
  tests/test_herbin_branding.py
git diff --cached --check
git diff --cached --name-status
git commit -m "test: verify headless terminal delivery contract"
```

---

### Task 7: Build and verify RDH.12 in GitHub Actions without installing it

**Files:**
- Verify: `.github/workflows/codex-macos-herbin.yml`
- Verify: GitHub Actions artifact, checksum, and metadata for RDH revision 12

**Interfaces:**
- Consumes: the fully verified feature-branch commits.
- Produces: one macOS arm64 RDH.12 candidate whose metadata source SHA equals the feature head.

- [ ] **Step 1: Recheck exact publication scope**

```bash
git status --short
git log --oneline fork/rdh/1.4.9..HEAD
git diff --stat fork/rdh/1.4.9..HEAD
git diff --name-only fork/rdh/1.4.9..HEAD
```

Expected: design, plan, and headless-terminal commits only; `implementation-notes.md` remains unstaged and absent from commit diffs.

- [ ] **Step 2: Ask for explicit feature-branch push and CI-dispatch approval**

The root/controller states the exact branch name, commit SHA, workflow, and
`rdh_revision=12`. Do not push or dispatch until the user approves these
external state changes.

- [ ] **Step 3: Push the feature branch without force**

```bash
git push -u fork HEAD:feat/headless-terminal-cli
```

Read back the remote ref and require exact equality:

```bash
test "$(git rev-parse HEAD)" = "$(git ls-remote fork refs/heads/feat/headless-terminal-cli | awk '{print $1}')"
```

- [ ] **Step 4: Dispatch one RDH macOS candidate build**

Use the existing workflow with:

```text
source_ref=feat/headless-terminal-cli
rdh_revision=12
```

Record the single run URL and source SHA. Do not dispatch a duplicate while the run is healthy and progressing.

- [ ] **Step 5: Wait event-driven through build and artifact upload**

Require success for source checkout, RDH invariants, focused headless tests,
Rust/Flutter build, bundle identity, ad-hoc signing, DMG creation, checksum,
metadata, and upload. A long vcpkg/build step without errors is not a reason to
cancel or duplicate the run.

- [ ] **Step 6: Download and verify the artifact without mounting it read-write**

Verify:

```text
rdh_revision=12
source_commit=<exact feature HEAD>
architecture=arm64
bundle_id=com.herbin.rustdesk
bundle_name=RustDesk-Herbin
signature=deep and strict valid
checksum=supplied SHA-256 matches DMG bytes
```

Mount the DMG read-only for bundle inspection, unmount it, and verify no mount
point remains. Do not copy the app into `/Applications` in this task.

---

### Task 8: Install transactionally and perform real Work PC acceptance

**Files:**
- Verify: the RDH.12 artifact from Task 7
- Verify: `/Applications/RustDesk.app` rescue channel
- Replace only after approval: `/Applications/RustDesk-Herbin.app`

**Interfaces:**
- Consumes: verified RDH.12 candidate and official RustDesk rescue connection.
- Produces: installed candidate, real no-window terminal evidence, and a validated rollback outcome.

- [ ] **Step 1: Stop at the installation approval boundary**

Report artifact SHA-256, source commit, signature result, current RDH version,
exact installed path, rollback path, and expected RDH connection interruption.
Ask the user to switch to official RustDesk and explicitly approve the
transactional replacement.

- [ ] **Step 2: Verify the rescue channel and rollback inputs**

Read-only checks must prove:

```text
official RustDesk --server is alive
official RustDesk --cm is the active rescue connection
RDH --cm is not the controlling connection
current RDH app passes deep/strict signature verification
candidate app passes deep/strict signature verification
rollback copy hash equals current installed RDH hash
```

Abort before replacement if any condition fails.

- [ ] **Step 3: Replace only RDH using the existing transactional runbook**

Follow `docs/rdh-upgrade-runbook.md` exactly: stage the candidate under a unique
hidden `/Applications` path, stop only `com.herbin.*` jobs, move the old app to
the verified rollback path, atomically install the candidate, bootstrap only
RDH jobs, and automatically restore the old app if PID, hash, signature, bundle
identity, or CLI smoke checks fail. Do not stop or restart official RustDesk.

- [ ] **Step 4: Verify headless startup with no Flutter window**

From a local PTY, run:

```bash
/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin \
  --terminal --headless 175116438
```

Before and after startup, enumerate RDH processes and macOS windows. Require a
new headless CLI process and no new Flutter terminal window. The existing RDH
server may remain running; GUI absence is determined by window/process role,
not by requiring all RDH processes to be absent.

- [ ] **Step 5: Exercise the real PowerShell path**

In the remote shell, verify:

```powershell
Get-Date -Format o
$env:USERNAME
Write-Output "RDH_HEADLESS_OK"
```

Then verify arrow-key history, Backspace editing, pasted Unicode, ANSI output,
and a cancellable command such as `Start-Sleep -Seconds 30` interrupted by
`Ctrl+C`. Resize the local PTY and require the remote rows/columns to change.

- [ ] **Step 6: Verify default close and persistent detach separately**

Default mode:

1. record the remote terminal PID from `TerminalOpened` diagnostics;
2. press `Ctrl+]`;
3. reconnect and prove the prior terminal was closed rather than resumed.

Persistent mode:

1. run with `--persistent`;
2. print a unique marker and record the remote PID;
3. press `Ctrl+]`;
4. reconnect with `--persistent`;
5. require the same server-side session/PID and buffered marker to return.

- [ ] **Step 7: Verify terminal restoration and Flutter regression**

After clean exit, remote non-zero exit, `SIGTERM`, and an induced connection
failure, run `stty -a` and compare the canonical/echo flags with the pre-run
snapshot. Finally run ordinary:

```bash
/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin \
  --terminal 175116438
```

Require the existing Flutter terminal window to open and connect normally.

- [ ] **Step 8: Record the acceptance outcome without touching unrelated notes**

Record command form, candidate commit, CI run URL, artifact checksum, remote
peer, tests performed, persistent PID evidence, Flutter regression result, and
rollback status in the task handoff. Do not edit or stage the pre-existing
`implementation-notes.md` unless the user separately requests that update.

---

### Task 9: Publish a Draft PR and stop

**Files:**
- Verify: complete feature branch diff against `rdh/1.4.9`
- Publish: Draft PR on `Herbin-s/rustdesk`, base `rdh/1.4.9`

**Interfaces:**
- Consumes: committed implementation, green CI, and completed real acceptance.
- Produces: one Draft PR containing the exact validated feature branch.

- [ ] **Step 1: Invoke the delivery skill**

Use `superpowers:finishing-a-development-branch`. Re-run its required aggregate
verification and confirm the branch contains no unstaged feature changes.

- [ ] **Step 2: Verify final PR scope**

```bash
git diff --check fork/rdh/1.4.9...HEAD
git diff --name-status fork/rdh/1.4.9...HEAD
git log --oneline fork/rdh/1.4.9..HEAD
git status --short
```

Expected: only design, plan, headless implementation, tests, workflow, and
runbook changes; the user's unrelated notes remain outside the branch diff.

- [ ] **Step 3: Open the Draft PR against the fork branch**

The PR title is `feat: add headless terminal CLI`. Its body lists the command
contract, architecture, focused tests, CI run, real peer acceptance, security
properties, and rollback result. Open it as Draft against
`Herbin-s/rustdesk:rdh/1.4.9`, never against upstream `rustdesk/rustdesk`.

- [ ] **Step 4: Stop after reporting the Draft PR**

Report the Draft PR URL, head SHA, CI run URL, installed candidate status, and
remaining user-owned dirty file. Do not request or dispatch an independent
review, mark the PR ready, merge it, delete the branch, or remove rollback
artifacts.
