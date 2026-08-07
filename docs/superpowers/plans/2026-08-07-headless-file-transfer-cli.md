# RDH Headless File Transfer CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a macOS RDH CLI that pushes or pulls one regular file through the native RustDesk file-transfer channel without Flutter or terminal Base64 chunking.

**Architecture:** Add a `Session<HeadlessFileTransferHandler>` frontend using `ConnType::FILE_TRANSFER`, the existing `FileManager`/`Data::SendFiles` boundary, and existing `TransferJob` blocks. Keep argument parsing, path validation, callback adaptation, completion parsing, state coordination, signal handling, and system wiring in focused modules; add one default completion callback to the UI-session boundary so the headless frontend can observe both local-read and local-write job completion without changing protobuf or existing GUI callbacks. After push sends native `FileTransferDone`, use the existing `ReadDir` action to verify that the remote target is a regular file with the expected size.

**Tech Stack:** Rust 2021, existing RustDesk `Session`/`client::io_loop`, `FileManager`, `TransferJob`, `FileAction`/`FileResponse`, macOS termios and Tokio Unix signals through existing dependencies, Python RDH source-contract tests, GitHub Actions macOS arm64 build.

**Delivery:** After implementation verification and required real acceptance,
open a Draft PR and stop. Independent review is outside this plan.

## Global Constraints

- Target an isolated worktree based on RDH commit `cc5b57d`; preserve the dirty `rdh/1.4.9` checkout and its `implementation-notes.md`.
- Implement only macOS controller-side headless file transfer; do not change protobuf or controlled-side service behavior.
- Preserve ordinary `--file-transfer <peer-id>` Flutter behavior and every existing `--terminal --headless` behavior.
- Support only one regular file per invocation: `push` and `pull`, optional `--relay`, and optional `--overwrite`.
- Do not add directories, multiple sources, shell expansion, stdin/stdout file streams, retry, reconnect, or resume.
- Destination existence fails with status 7 unless `--overwrite` is explicit; overwrite always confirms offset block 0.
- Saved credentials work without a TTY; a required password or 2FA prompt without stdin TTY exits 4.
- Reject `--password` and insecure transport; never expose plaintext credentials or raw file blocks.
- stdout is empty until success, when it contains only the destination path and one newline; progress and diagnostics use stderr.
- Do not install, replace, restart, or terminate the running RDH application or service during implementation or CI artifact verification.
- Real work-PC acceptance uses only task-owned test files and removes them after evidence is recorded.
- Commit messages, branch names, code, comments, logs, and PR text remain English.

## File Structure

- Create `src/headless_file_transfer.rs`: module root, parser dispatch, CLI entrypoint, and shared exit mapping.
- Create `src/headless_file_transfer/args.rs`: pure argument classification and validation.
- Create `src/headless_file_transfer/paths.rs`: local file snapshots, pull destination preflight, remote path splitting, and regular-file metadata validation.
- Create `src/headless_file_transfer/completion.rs`: validated parsing of existing transfer-job completion JSON.
- Create `src/headless_file_transfer/handler.rs`: `InvokeUiSession` callback-to-event adapter.
- Create `src/headless_file_transfer/state.rs`: pure one-job state machine and backend action contract.
- Create `src/headless_file_transfer/signals.rs`: macOS SIGINT/SIGTERM forwarding.
- Create `src/headless_file_transfer/runtime.rs`: system session, filesystem, prompts, output, signal, and network-thread wiring.
- Create `src/headless_auth.rs`: narrow shared secure prompts and authentication event type for headless CLIs.
- Modify `src/headless_terminal/handler.rs`, `src/headless_terminal/runtime.rs`, and `src/headless_terminal/tty.rs`: consume the shared authentication boundary without changing terminal lifecycle behavior.
- Modify `src/ui_session_interface.rs`: add a default raw completion callback that leaves existing frontends unchanged.
- Modify `src/client/io_loop.rs`: emit serialized completion for local-read and local-write generic jobs.
- Modify `src/lib.rs` and `src/core_main.rs`: register and route the new CLI before Flutter dispatch.
- Modify `tests/test_herbin_branding.py`: enforce the source, dispatch, security, CI, and documentation contract.
- Modify `.github/workflows/codex-macos-herbin.yml`: run focused headless file-transfer tests before the release build.
- Modify `docs/rdh-upgrade-runbook.md` and `implementation-notes.md`: preserve the permanent RDH CLI contract and verification boundary.

---

### Task 1: Parse the headless file-transfer command

**Files:**
- Create: `src/headless_file_transfer.rs`
- Create: `src/headless_file_transfer/args.rs`
- Modify: `src/lib.rs:1-5`

**Interfaces:**
- Consumes: complete argument vector without argv[0].
- Produces: `TransferDirection`, `HeadlessFileTransferArgs`, `HeadlessFileTransferDispatch`, `usage()`, `is_requested(args)`, and `classify(args, is_macos)`.

- [ ] **Step 1: Write failing parser tests**

Create the module files and add these type shells plus tests to `args.rs`. Leave `classify` as `unimplemented!()` only for the red run.

```rust
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

#[test]
fn accepts_push_pull_and_optional_flags() {
    assert_eq!(
        classify(&args(&[
            "--file-transfer", "--headless", "--relay", "--overwrite",
            "175116438", "push", "/tmp/a b.bin", r"C:\Users\82520\a b.bin",
        ]), true),
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
        classify(&args(&[
            "--file-transfer", "--headless", "175116438", "pull",
            r"C:\Users\82520\a.bin", "/tmp/a.bin",
        ]), true),
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
        vec!["--file-transfer", "--headless", "175116438", "copy", "a", "b"],
        vec!["--file-transfer", "--headless", "175116438", "push", "a"],
        vec!["--file-transfer", "--headless", "175116438", "push", "a", "b", "c"],
        vec!["--file-transfer", "--headless", "--password", "secret", "175116438", "push", "a", "b"],
        vec!["--file-transfer", "--headless", "--persistent", "175116438", "push", "a", "b"],
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
        classify(&args(&[
            "--file-transfer", "--headless", "175116438", "push", "a", "b"
        ]), false),
        HeadlessFileTransferDispatch::Invalid(_)
    ));
}
```

- [ ] **Step 2: Run the parser tests and verify RED**

Run:

```bash
source /Volumes/DevData/Development/RustDesk-Herbin/tools/devdata-env.zsh
cargo test --lib headless_file_transfer::args::tests -- --nocapture
```

Expected: FAIL because `classify` is not implemented.

- [ ] **Step 3: Implement the parser**

Implement the exact usage line and a pure positional parser. Optional flags may occur only before the peer ID; source and destination remain opaque nonempty strings.

```rust
pub(crate) const fn usage() -> &'static str {
    "Usage: RustDesk-Herbin --file-transfer --headless [--relay] [--overwrite] <peer-id> <push|pull> <source-file> <destination-file>"
}

pub(crate) fn is_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--file-transfer")
        && args.iter().any(|arg| arg == "--headless")
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
        _ => return HeadlessFileTransferDispatch::Invalid("operation must be push or pull".into()),
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
```

Register only the module and parser re-exports in `src/lib.rs` / `src/headless_file_transfer.rs`; do not route `core_main` yet.

- [ ] **Step 4: Run parser and terminal parser tests**

```bash
cargo test --lib headless_file_transfer::args::tests -- --nocapture
cargo test --lib headless_terminal::args::tests -- --nocapture
```

Expected: all tests PASS.

- [ ] **Step 5: Commit the parser**

```bash
git add src/lib.rs src/headless_file_transfer.rs src/headless_file_transfer/args.rs
git commit -m "feat: parse headless file transfer CLI"
```

### Task 2: Validate local and remote file boundaries

**Files:**
- Create: `src/headless_file_transfer/error.rs`
- Create: `src/headless_file_transfer/paths.rs`
- Modify: `src/headless_file_transfer.rs`

**Interfaces:**
- Consumes: parsed source/destination strings, peer platform, and `Vec<FileEntry>` metadata.
- Produces: `HeadlessFileTransferError`, `FileSnapshot`, `RemoteFilePath`, `inspect_push_source`, `inspect_pull_destination`, `verify_source_unchanged`, `split_remote_file_path`, and `single_regular_file_size`.

- [ ] **Step 1: Write failing filesystem and path tests**

Add tests that create a regular file, directory, symlink, Unix socket, existing destination, and missing destination parent.

```rust
#[test]
fn push_accepts_only_a_regular_non_symlink_file() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("probe.bin");
    std::fs::write(&file, b"abc").unwrap();
    assert_eq!(inspect_push_source(&file).unwrap().size, 3);
    assert!(inspect_push_source(temp.path()).is_err());

    let link = temp.path().join("probe-link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    assert!(inspect_push_source(&link).is_err());

    let socket = temp.path().join("probe.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    assert!(inspect_push_source(&socket).is_err());
}

#[test]
fn pull_destination_requires_existing_parent_and_explicit_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target.bin");
    inspect_pull_destination(&target, false).unwrap();
    std::fs::write(&target, b"old").unwrap();
    assert_eq!(inspect_pull_destination(&target, false).unwrap_err().status(), 7);
    inspect_pull_destination(&target, true).unwrap();
    assert!(inspect_pull_destination(&temp.path().join("missing/target.bin"), true).is_err());
}

#[test]
fn splits_windows_and_unix_remote_paths_for_postflight() {
    assert_eq!(
        split_remote_file_path(r"C:\Users\82520\probe.bin", "Windows").unwrap(),
        RemoteFilePath { parent: r"C:\Users\82520".into(), name: "probe.bin".into() }
    );
    assert_eq!(
        split_remote_file_path("/tmp/probe.bin", "Linux").unwrap(),
        RemoteFilePath { parent: "/tmp".into(), name: "probe.bin".into() }
    );
    assert_eq!(
        split_remote_file_path("probe.bin", "Windows").unwrap(),
        RemoteFilePath { parent: ".".into(), name: "probe.bin".into() }
    );
}

#[test]
fn accepts_exactly_one_regular_file_entry() {
    let file = FileEntry { entry_type: FileType::File.into(), size: 42, ..Default::default() };
    assert_eq!(single_regular_file_size(&[file]).unwrap(), 42);
    assert!(single_regular_file_size(&[]).is_err());
    assert!(single_regular_file_size(&[
        FileEntry { entry_type: FileType::Dir.into(), ..Default::default() }
    ]).is_err());
}
```

- [ ] **Step 2: Run the path tests and verify RED**

```bash
cargo test --lib headless_file_transfer::paths::tests -- --nocapture
```

Expected: FAIL because the boundary functions do not exist.

- [ ] **Step 3: Implement typed errors and path validation**

Define exact status mapping:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadlessFileTransferError {
    Internal(String),
    Usage(String),
    LocalPrecondition(String),
    Authentication(String),
    Connection(String),
    Transfer(String),
    DestinationExists(String),
    Protocol(String),
    Interrupted,
    Terminated,
}

impl HeadlessFileTransferError {
    pub(crate) const fn status(&self) -> i32 {
        match self {
            Self::Internal(_) => 1,
            Self::Usage(_) => 2,
            Self::LocalPrecondition(_) => 3,
            Self::Authentication(_) => 4,
            Self::Connection(_) | Self::Protocol(_) => 5,
            Self::Transfer(_) => 6,
            Self::DestinationExists(_) => 7,
            Self::Interrupted => 130,
            Self::Terminated => 143,
        }
    }
}
```

Use `symlink_metadata` so a symlink to a regular file remains rejected. `FileSnapshot` stores `path: PathBuf`, `size: u64`, and `modified: SystemTime`; `verify_source_unchanged` compares both size and modification time. `inspect_pull_destination` rejects symlinks and directories even with `--overwrite`.

For remote postflight, split on `\\` or `/` only for Windows, on `/` for other peers, reject an empty filename or trailing separator, and use `.` as the parent for a bare filename. `single_regular_file_size` requires one entry whose protobuf type resolves to `FileType::File`.

- [ ] **Step 4: Run focused path tests**

```bash
cargo test --lib headless_file_transfer::paths::tests -- --nocapture
```

Expected: all tests PASS.

- [ ] **Step 5: Commit boundary validation**

```bash
git add src/headless_file_transfer.rs src/headless_file_transfer/error.rs src/headless_file_transfer/paths.rs
git commit -m "feat: validate headless transfer paths"
```

### Task 3: Extract the shared secure authentication prompt boundary

**Files:**
- Create: `src/headless_auth.rs`
- Modify: `src/lib.rs:1-8`
- Modify: `src/headless_terminal/handler.rs:1-25`
- Modify: `src/headless_terminal/runtime.rs:1-20, 370-425, 600-620`
- Modify: `src/headless_terminal/tty.rs:1-260`

**Interfaces:**
- Consumes: authentication message-box types and stdin.
- Produces: shared `AuthPrompt`, `stdin_is_tty`, `prompt_line`, `prompt_secret`, `prompt_secret_with_cancel`, and `prompt_confirmation`.

- [ ] **Step 1: Write failing shared-auth tests**

Create `headless_auth.rs` with type/function shells and tests for line trimming, confirmation, and optional cancel-byte behavior.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthPrompt {
    Password { retry: bool },
    TwoFactor,
    InsecureConnection,
}

#[test]
fn secret_cancel_byte_is_opt_in() {
    assert_eq!(secret_from_line(Some("\u{1d}".into()), None), Some("\u{1d}".into()));
    assert_eq!(secret_from_line(Some("\u{1d}".into()), Some(0x1d)), None);
}

#[test]
fn confirmation_is_explicit_and_case_insensitive() {
    assert_eq!(confirmation_from_line(Some("yes")), Some(true));
    assert_eq!(confirmation_from_line(Some("Y")), Some(true));
    assert_eq!(confirmation_from_line(Some("no")), Some(false));
    assert_eq!(confirmation_from_line(None), None);
}
```

- [ ] **Step 2: Run the shared-auth tests and verify RED**

```bash
cargo test --lib headless_auth::tests -- --nocapture
```

Expected: FAIL because the helpers are not implemented.

- [ ] **Step 3: Move, generalize, and reuse the existing prompt code**

Move `EchoGuard`, prompt line reading, confirmation parsing, and no-echo secret handling out of `headless_terminal/tty.rs`. Keep terminal cancellation explicit:

```rust
// headless terminal runtime
prompt_secret_with_cancel("Password: ", Some(DETACH_BYTE))

// headless file-transfer runtime later
prompt_secret("Password: ")
```

Change both terminal handler and runtime to import `crate::headless_auth::AuthPrompt` and the shared prompt functions. Keep raw-mode, terminal sizing, detach splitting, and terminal signals in `headless_terminal/tty.rs`. Make restoration errors say `RDH headless CLI failed to restore stdin echo` so the helper is frontend-neutral.

- [ ] **Step 4: Run shared-auth and complete terminal tests**

```bash
cargo test --lib headless_auth -- --nocapture
cargo test --lib headless_terminal -- --nocapture
```

Expected: shared tests PASS and all 54 existing headless terminal tests remain PASS.

- [ ] **Step 5: Commit the narrow extraction**

```bash
git add src/lib.rs src/headless_auth.rs src/headless_terminal/handler.rs src/headless_terminal/runtime.rs src/headless_terminal/tty.rs
git commit -m "refactor: share headless authentication prompts"
```

### Task 4: Expose native transfer-job completion without changing GUI callbacks

**Files:**
- Create: `src/headless_file_transfer/completion.rs`
- Modify: `src/headless_file_transfer.rs`
- Modify: `src/ui_session_interface.rs:1710-1760`
- Modify: `src/client/io_loop.rs:280-300, 1695-1790`

**Interfaces:**
- Consumes: JSON returned by existing `fs::serialize_transfer_job` / `fs::handle_read_jobs`.
- Produces: `TransferCompletion::parse`, and default `InvokeUiSession::file_transfer_job_completed(&self, job_json: &str)`.

- [ ] **Step 1: Write failing completion-parser tests**

```rust
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransferCompletion {
    pub(crate) id: i32,
    pub(crate) file_num: i32,
    pub(crate) total_size: u64,
    pub(crate) finished_size: u64,
    #[serde(default)]
    pub(crate) done: bool,
    #[serde(default)]
    pub(crate) error: String,
}

#[test]
fn parses_completed_job_without_exposing_paths() {
    let completion = TransferCompletion::parse(
        r#"{"id":7,"fileNum":1,"totalSize":42,"finishedSize":42,"done":true,"error":""}"#,
    ).unwrap();
    assert_eq!(completion.id, 7);
    assert_eq!(completion.total_size, 42);
    assert!(completion.done);
}

#[test]
fn rejects_missing_or_inconsistent_completion_fields() {
    assert!(TransferCompletion::parse("{}").is_err());
    assert!(TransferCompletion::parse(
        r#"{"id":7,"fileNum":1,"totalSize":42,"finishedSize":43,"done":true,"error":""}"#
    ).is_err());
}
```

- [ ] **Step 2: Run the completion tests and verify RED**

```bash
cargo test --lib headless_file_transfer::completion::tests -- --nocapture
```

Expected: FAIL because `TransferCompletion::parse` is absent.

- [ ] **Step 3: Implement validated completion parsing**

Parse with `serde_json`, require `id > 0`, `file_num >= 0`, and `finished_size <= total_size`. Return only typed counts/status; never include the serialized job's source or destination path in errors.

- [ ] **Step 4: Add the default UI-session callback and I/O-loop emission**

Add this default method so Sciter, Flutter, terminal, and other frontends require no changes:

```rust
fn file_transfer_job_completed(&self, _job_json: &str) {}
```

In the client read-job timer, retain and forward the nonempty completion string:

```rust
match fs::handle_read_jobs(&mut self.read_jobs, &mut peer).await {
    Ok(job_json) => {
        if !job_json.is_empty() {
            self.handler.file_transfer_job_completed(&job_json);
        }
    }
    Err(error) => {
        self.handler.msgbox("error", "Connection Error", &error.to_string(), "");
        break;
    }
}
```

For a received `FileTransferDone`, serialize the removed write job before calling existing `job_done`. For a received `FileTransferError`, serialize a removed job with `done=false` and the existing error. Forward the JSON only through the new callback; retain existing GUI `job_done`/`job_error` calls unchanged.

- [ ] **Step 5: Run completion, client, and existing file tests**

```bash
cargo test --lib headless_file_transfer::completion::tests -- --nocapture
cargo test --lib client:: -- --nocapture
cargo test -p hbb_common fs::tests -- --nocapture
```

Expected: all selected tests PASS; no submodule file is modified.

- [ ] **Step 6: Commit completion visibility**

```bash
git add src/headless_file_transfer.rs src/headless_file_transfer/completion.rs src/ui_session_interface.rs src/client/io_loop.rs
git commit -m "feat: expose native file transfer completion"
```

### Task 5: Adapt file-transfer callbacks into typed headless events

**Files:**
- Create: `src/headless_file_transfer/handler.rs`
- Modify: `src/headless_file_transfer.rs`

**Interfaces:**
- Consumes: `InvokeUiSession` callbacks and completion JSON.
- Produces: `HeadlessFileTransferHandler` and `HeadlessFileTransferEvent`.

- [ ] **Step 1: Write failing handler mapping tests**

Define the event enum and test each active callback:

```rust
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HeadlessFileTransferEvent {
    PeerPlatform(String),
    Connected,
    Auth(AuthPrompt),
    Files {
        id: i32,
        entries: Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    },
    Conflict {
        id: i32,
        file_num: i32,
        destination: String,
        is_upload: bool,
        is_identical: bool,
    },
    Progress {
        id: i32,
        file_num: i32,
        speed: u64,
        finished_size: u64,
    },
    Completed(TransferCompletion),
    JobFailed { id: i32, file_num: i32, message: String },
    ProtocolFailed(String),
    ConnectionFailed(String),
}
```

Tests must prove:

```rust
#[test]
fn emits_only_file_transfer_connection_and_peer_platform() {
    let (handler, rx) = handler();
    handler.set_peer_info(&PeerInfo { platform: "Windows".into(), ..Default::default() });
    handler.on_connected(ConnType::FILE_TRANSFER);
    handler.on_connected(ConnType::TERMINAL);
    assert_eq!(rx.recv().unwrap(), HeadlessFileTransferEvent::PeerPlatform("Windows".into()));
    assert_eq!(rx.recv().unwrap(), HeadlessFileTransferEvent::Connected);
    assert!(rx.try_recv().is_err());
}

#[test]
fn maps_files_conflict_progress_completion_and_failure() {
    let (handler, rx) = handler();
    handler.update_folder_files(7, &vec![file_entry(42)], "remote".into(), false, false);
    handler.override_file_confirm(7, 0, "target".into(), false, true);
    handler.job_progress(7, 0, 1024.0, 42.0);
    handler.file_transfer_job_completed(
        r#"{"id":7,"fileNum":1,"totalSize":42,"finishedSize":42,"done":true,"error":""}"#,
    );
    handler.job_error(7, "permission denied".into(), 0);
    assert!(matches!(rx.recv().unwrap(), HeadlessFileTransferEvent::Files { .. }));
    assert!(matches!(rx.recv().unwrap(), HeadlessFileTransferEvent::Conflict { .. }));
    assert!(matches!(rx.recv().unwrap(), HeadlessFileTransferEvent::Progress { .. }));
    assert!(matches!(rx.recv().unwrap(), HeadlessFileTransferEvent::Completed(_)));
    assert!(matches!(rx.recv().unwrap(), HeadlessFileTransferEvent::JobFailed { .. }));
}
```

- [ ] **Step 2: Run handler tests and verify RED**

```bash
cargo test --lib headless_file_transfer::handler::tests -- --nocapture
```

Expected: FAIL because the handler is not implemented.

- [ ] **Step 3: Implement the handler**

Follow the existing `HeadlessTerminalHandler` sender-lock pattern. Emit events from only these callbacks:

- `set_peer_info`
- `on_connected` for `ConnType::FILE_TRANSFER`
- `update_folder_files`
- `override_file_confirm`
- `job_progress`
- `file_transfer_job_completed`
- `job_error`
- authentication and error `msgbox`

Convert finite nonnegative progress floats to saturated `u64`. A malformed completion JSON emits `ProtocolFailed("invalid file-transfer completion event")` without including raw JSON. Implement every other required `InvokeUiSession` callback as an explicit no-op, with `get_rgba` returning null and `is_multi_ui_session` returning false under its existing cfg.

- [ ] **Step 4: Run handler and terminal handler tests**

```bash
cargo test --lib headless_file_transfer::handler::tests -- --nocapture
cargo test --lib headless_terminal::handler::tests -- --nocapture
```

Expected: all tests PASS.

- [ ] **Step 5: Commit the handler**

```bash
git add src/headless_file_transfer.rs src/headless_file_transfer/handler.rs
git commit -m "feat: adapt headless file transfer events"
```

### Task 6: Implement the one-job state machine

**Files:**
- Create: `src/headless_file_transfer/state.rs`
- Modify: `src/headless_file_transfer.rs`

**Interfaces:**
- Consumes: parsed args, optional push `FileSnapshot`, typed handler events, and local shutdown events.
- Produces: `TransferCoordinator`, `RuntimeEvent`, `TransferAction`, `TransferBackend`, and deterministic final statuses.

- [ ] **Step 1: Write failing state-machine tests**

Use a fake backend recording actions, stderr, stdout, prompts, and source/destination verification. Give `TransferCoordinator` an inherent `handle(&mut self, event: RuntimeEvent, backend: &mut impl TransferBackend) -> Option<i32>` method and implement representative cases exactly:

```rust
#[derive(Default)]
struct FakeBackend {
    actions: Vec<TransferAction>,
    stdout: Vec<String>,
    stderr: Vec<String>,
    stdin_is_tty: bool,
    push_source_valid: bool,
    pull_destination_size: Option<u64>,
}

fn completion(id: i32, size: u64) -> RuntimeEvent {
    RuntimeEvent::Session(HeadlessFileTransferEvent::Completed(
        TransferCompletion {
            id,
            file_num: 1,
            total_size: size,
            finished_size: size,
            done: true,
            error: String::new(),
        },
    ))
}

#[test]
fn connected_starts_exactly_one_push_job() {
    let mut coordinator = push_coordinator(false, 7, 42);
    let mut backend = FakeBackend::default();

    assert_eq!(coordinator.handle(peer_platform("Windows"), &mut backend), None);
    assert_eq!(coordinator.handle(connected(), &mut backend), None);
    assert_eq!(
        backend.actions,
        vec![TransferAction::StartJob {
            id: 7,
            source: "/tmp/source.bin".into(),
            destination: r"C:\Users\82520\target.bin".into(),
            is_remote: false,
        }]
    );
}

#[test]
fn conflict_defaults_to_skip_and_finishes_with_status_seven() {
    let mut coordinator = push_coordinator(false, 7, 42);
    let mut backend = FakeBackend::default();
    coordinator.handle(peer_platform("Windows"), &mut backend);
    coordinator.handle(connected(), &mut backend);

    assert_eq!(
        coordinator.handle(conflict(7, 0, true), &mut backend),
        None
    );
    assert_eq!(
        backend.actions.last(),
        Some(&TransferAction::ConfirmOverwrite {
            id: 7,
            file_num: 0,
            overwrite: false,
            is_upload: true,
        })
    );
    assert_eq!(coordinator.handle(completion(7, 42), &mut backend), None);
    assert_eq!(backend.actions.last(), Some(&TransferAction::CloseTransport));
    assert_eq!(
        coordinator.handle(RuntimeEvent::TransportClosed, &mut backend),
        Some(7)
    );
    assert!(backend.stdout.is_empty());
}

#[test]
fn push_success_requires_source_unchanged_then_remote_read_dir_match() {
    let mut coordinator = push_coordinator(false, 7, 42);
    let mut backend = FakeBackend {
        push_source_valid: true,
        ..Default::default()
    };
    coordinator.handle(peer_platform("Windows"), &mut backend);
    coordinator.handle(connected(), &mut backend);

    assert_eq!(coordinator.handle(completion(7, 42), &mut backend), None);
    assert_eq!(
        backend.actions.last(),
        Some(&TransferAction::ReadRemoteDir {
            path: r"C:\Users\82520".into(),
            include_hidden: true,
        })
    );
    assert_eq!(
        coordinator.handle(
            remote_files(0, r"C:\Users\82520", vec![regular_file("target.bin", 42)]),
            &mut backend,
        ),
        None
    );
    assert_eq!(backend.stdout, vec![r"C:\Users\82520\target.bin"]);
    assert_eq!(backend.actions.last(), Some(&TransferAction::CloseTransport));
    assert_eq!(
        coordinator.handle(RuntimeEvent::TransportClosed, &mut backend),
        Some(0)
    );
}

#[test]
fn interrupt_cancels_before_close_and_returns_130() {
    let mut coordinator = pull_coordinator(false, 7);
    let mut backend = FakeBackend::default();
    coordinator.handle(peer_platform("Windows"), &mut backend);
    coordinator.handle(connected(), &mut backend);

    assert_eq!(
        coordinator.handle(RuntimeEvent::Signal(TransferSignal::Interrupt), &mut backend),
        None
    );
    assert_eq!(
        &backend.actions[1..],
        &[TransferAction::CancelJob { id: 7 }, TransferAction::CloseTransport]
    );
    assert_eq!(
        coordinator.handle(RuntimeEvent::TransportClosed, &mut backend),
        Some(130)
    );
}
```

Add the remaining named tests with these exact event sequences and assertions:

| Test | Event sequence | Required assertion |
|---|---|---|
| `connected_starts_exactly_one_pull_job` | peer platform, connected | one `StartJob` with `is_remote=true`; no second start on a duplicate connected event |
| `saved_credentials_need_no_tty_but_prompt_without_tty_exits_four` | connected with no auth prompt; separately `Auth(Password)` with `stdin_is_tty=false` | saved-credential path starts normally; prompt path closes and returns 4 without calling a prompt method |
| `overwrite_confirms_offset_zero_and_does_not_resume` | push conflict for file 0 with `is_upload=true` and `overwrite=true`; separately pull conflict with `is_upload=false` | one matching `ConfirmOverwrite { overwrite: true }` per case; no offset other than the protocol's block 0 and no resume action |
| `pull_success_requires_regular_metadata_and_complete_local_write` | one regular-file metadata response, completed job, transport close | destination verifier receives the exact expected size, stdout gets exactly the local destination, final status 0 |
| `incomplete_or_wrong_job_completion_is_protocol_status_five` | completion with wrong ID; separately `done=false`; separately `finished_size < total_size` | each case closes once, emits no stdout, and returns 5 after close |
| `job_error_maps_to_six_and_connection_loss_maps_to_five` | job failure; separately unexpected transport close while transferring | failure returns 6, interruption returns 5, neither emits another `StartJob` |
| `terminate_cancels_before_close_and_returns_143` | SIGTERM, transport close | ordered `CancelJob`, `CloseTransport`, then status 143 |
| `success_outputs_only_destination_and_failure_outputs_nothing` | complete pull success; separately a transfer failure | success stdout is exactly one destination entry, failure stdout is empty, all diagnostics are in recorded stderr |

The helper constructors used above must create complete real `RuntimeEvent` values; they must not bypass validation or call coordinator internals.

Define the exact action/backend contract in the test file before implementing coordinator behavior:

```rust
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TransferAction {
    StartJob {
        id: i32,
        source: String,
        destination: String,
        is_remote: bool,
    },
    ConfirmOverwrite {
        id: i32,
        file_num: i32,
        overwrite: bool,
        is_upload: bool,
    },
    ReadRemoteDir { path: String, include_hidden: bool },
    CancelJob { id: i32 },
    CloseTransport,
    RejectInsecureConnection,
}

pub(crate) trait TransferBackend {
    fn stdin_is_tty(&self) -> bool;
    fn action(&mut self, action: TransferAction);
    fn verify_push_source(&mut self) -> Result<(), HeadlessFileTransferError>;
    fn verify_pull_destination(&mut self, expected_size: u64) -> Result<(), HeadlessFileTransferError>;
    fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessFileTransferError>;
    fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessFileTransferError>;
    fn prompt_line(&mut self) -> Result<Option<String>, HeadlessFileTransferError>;
    fn login(&mut self, password: String, remember: bool);
    fn send_two_factor(&mut self, code: String);
    fn write_stdout(&mut self, destination: &str) -> Result<(), HeadlessFileTransferError>;
    fn write_stderr(&mut self, message: &str);
}
```

- [ ] **Step 2: Run state tests and verify RED**

```bash
cargo test --lib headless_file_transfer::state::tests -- --nocapture
```

Expected: FAIL because `TransferCoordinator` is not implemented.

- [ ] **Step 3: Implement phases and authentication**

Use these phases:

```rust
enum TransferPhase {
    Authenticating,
    Transferring,
    FinalizingPush,
    Closing,
    Closed,
    Failed,
}
```

Store the expected job ID, peer platform, expected size, maximum observed finished size, conflict flag, password-submission flag, and final local status. Require `PeerPlatform` before `Connected`; on `Connected`, send one `StartJob` using `is_remote = direction == Pull`.

For `AuthPrompt`, require stdin TTY only when a prompt actually arrives. Reject insecure transport with `RejectInsecureConnection`. Password EOF, save-confirmation EOF, and 2FA EOF close transport and return authentication status 4.

- [ ] **Step 4: Implement transfer, conflict, progress, and completion transitions**

On `Files` during transfer, require the expected job ID and exactly one regular entry, then store/compare its size. On conflict, send `ConfirmOverwrite` with the parsed `--overwrite`; set the conflict result when false.

On `Completed`:

- require expected ID, `done=true`, empty error, and `finished_size == total_size`;
- if conflict was selected, close with status 7;
- for pull, verify local destination size, write destination to stdout, and close with 0;
- for push, verify source metadata, split the remote destination using the stored peer platform, send `ReadRemoteDir { include_hidden: true }`, and enter `FinalizingPush`.

During `FinalizingPush`, accept only an id-0 directory response containing an entry with the exact destination name, `FileType::File`, and expected size. Then write the destination to stdout and close with 0.

Progress writes one bounded line:

```text
direction=push transferred=1048576 total=46964366 percent=2.23 speed_bps=1048576
```

Use the callback's existing one-second cadence; do not add a timer.

- [ ] **Step 5: Implement failure and cancellation transitions**

`JobFailed` maps to status 6. `ProtocolFailed`, unexpected IDs/events, and incomplete counts map to 5. Unexpected transport close maps to 5 and writes `transfer interrupted; a partial file may remain` on stderr. SIGINT/SIGTERM enqueue `CancelJob` then `CloseTransport`, returning 130/143 when transport closes.

- [ ] **Step 6: Run all state tests**

```bash
cargo test --lib headless_file_transfer::state::tests -- --nocapture
```

Expected: all tests PASS.

- [ ] **Step 7: Commit the state machine**

```bash
git add src/headless_file_transfer.rs src/headless_file_transfer/state.rs
git commit -m "feat: coordinate headless file transfers"
```

### Task 7: Wire the macOS runtime, signals, and native session

**Files:**
- Create: `src/headless_file_transfer/signals.rs`
- Create: `src/headless_file_transfer/runtime.rs`
- Modify: `src/headless_file_transfer.rs`

**Interfaces:**
- Consumes: `HeadlessFileTransferArgs`, typed handler events, and `TransferAction`.
- Produces: `run(args) -> i32`, `SystemTransferBackend`, SIGINT/SIGTERM events, and one `Session<HeadlessFileTransferHandler>` network thread.

- [ ] **Step 1: Write failing signal-forwarder tests**

Hide Tokio signal streams behind a factory so tests can inject events. Prove `Interrupt` and `Terminate` remain distinct and dropping the forwarder stops the thread without emitting another event.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransferSignal {
    Interrupt,
    Terminate,
}
```

- [ ] **Step 2: Run signal tests and verify RED**

```bash
cargo test --lib headless_file_transfer::signals::tests -- --nocapture
```

Expected: FAIL because the signal forwarder is absent.

- [ ] **Step 3: Implement SIGINT/SIGTERM forwarding**

Use the existing Tokio re-export and a current-thread runtime:

```rust
use tokio::signal::unix::{signal, SignalKind};
let mut interrupt = signal(SignalKind::interrupt())?;
let mut terminate = signal(SignalKind::terminate())?;
```

Emit one event, then stop. `Drop` sends an internal shutdown and joins the task-owned thread. Do not register process-global `ctrlc` handlers.

- [ ] **Step 4: Write failing system-backend mapping tests**

Test a small pure `map_action` adapter or fake session boundary so each action maps exactly:

- `StartJob` -> `send_files(id, JobType::Generic as i32, source, destination, 0, false, is_remote)`
- `ConfirmOverwrite` -> `set_confirm_override_file(id, file_num, overwrite, false, is_upload)`
- `ReadRemoteDir` -> `read_remote_dir(path, true)`
- `CancelJob` -> `cancel_job(id)`
- `CloseTransport` -> `close()`
- `RejectInsecureConnection` -> `continue_insecure_connection(false)`

- [ ] **Step 5: Implement system runtime wiring**

Preflight before starting the network thread:

```rust
let source_snapshot = match args.direction {
    TransferDirection::Push => Some(inspect_push_source(Path::new(&args.source))?),
    TransferDirection::Pull => {
        inspect_pull_destination(Path::new(&args.destination), args.overwrite)?;
        None
    }
};
```

Construct the session with `HeadlessFileTransferHandler`, enabled file-transfer permission, and `ConnType::FILE_TRANSFER`:

```rust
config.initialize(
    args.peer_id.clone(),
    ConnType::FILE_TRANSFER,
    None,
    args.force_relay,
    None,
    None,
    None,
);
```

Use `fs::get_next_job_id()` once. Spawn the existing `io_loop(session, round)` on one thread and send `TransportClosed` after return. Adapt handler and signal channels to `RuntimeEvent`. Do not create a nested Tokio runtime for networking; reuse the existing synchronous `io_loop` frontend pattern from headless terminal.

`SystemTransferBackend` uses `std::io::IsTerminal` only at an actual authentication prompt, shared `headless_auth` prompts, `verify_source_unchanged`, local destination metadata, and strict stdout/stderr writes. On success, `writeln!(stdout, "{}", args.destination)` is the only stdout write.

- [ ] **Step 6: Implement `run_cli` and error mapping**

`headless_file_transfer.rs` classifies arguments, prints usage errors to stderr with status 2, calls `runtime::run` on macOS, and returns 2 on unsupported platforms. Runtime preflight errors print once to stderr and return their typed status.

- [ ] **Step 7: Run complete headless file-transfer tests**

```bash
cargo test --lib headless_file_transfer -- --nocapture
cargo test --lib headless_terminal -- --nocapture
```

Expected: all new tests and all existing 54 terminal tests PASS.

- [ ] **Step 8: Commit runtime wiring**

```bash
git add src/headless_file_transfer.rs src/headless_file_transfer/runtime.rs src/headless_file_transfer/signals.rs
git commit -m "feat: run native headless file transfers"
```

### Task 8: Route the CLI before Flutter without changing GUI dispatch

**Files:**
- Modify: `src/core_main.rs:1-20, 110-175, 1010-1060`

**Interfaces:**
- Consumes: `headless_file_transfer::is_requested` and `run_cli`.
- Produces: correct dispatch ownership and process exit behavior.

- [ ] **Step 1: Write failing dispatch tests**

```rust
#[test]
fn headless_file_transfer_is_not_dispatched_to_flutter() {
    assert!(should_dispatch_flutter_connection(
        &args(&["--file-transfer", "175116438"]),
        true
    ));
    assert!(!should_dispatch_flutter_connection(
        &args(&[
            "--file-transfer", "--headless", "175116438", "push", "a", "b"
        ]),
        true
    ));
    assert!(should_dispatch_flutter_connection(
        &args(&["--terminal", "175116438"]),
        true
    ));
}
```

Add a source-level test that the file-transfer run hook appears after logging initialization and before the terminal hook / Flutter return.

- [ ] **Step 2: Run dispatch tests and verify RED**

```bash
cargo test --lib core_main::tests::headless_file_transfer_is_not_dispatched_to_flutter -- --nocapture
```

Expected: FAIL because Flutter dispatch excludes only headless terminal.

- [ ] **Step 3: Implement dispatch and run hook**

```rust
fn should_dispatch_flutter_connection(args: &[String], requested: bool) -> bool {
    requested
        && !crate::headless_terminal::is_requested(args)
        && !crate::headless_file_transfer::is_requested(args)
}
```

After `hbb_common::init_log`:

```rust
if crate::headless_file_transfer::is_requested(&args) {
    let exit_code = crate::headless_file_transfer::run_cli(&args);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    return None;
}
```

Keep the terminal hook unchanged immediately afterward.

- [ ] **Step 4: Run core, parser, and terminal regression tests**

```bash
cargo test --lib core_main::tests -- --nocapture
cargo test --lib headless_file_transfer -- --nocapture
cargo test --lib headless_terminal -- --nocapture
```

Expected: all tests PASS.

- [ ] **Step 5: Commit dispatch**

```bash
git add src/core_main.rs
git commit -m "feat: dispatch headless file transfer before Flutter"
```

### Task 9: Lock the RDH contract, CI test, and operational documentation

**Files:**
- Modify: `tests/test_herbin_branding.py`
- Modify: `.github/workflows/codex-macos-herbin.yml`
- Modify: `docs/rdh-upgrade-runbook.md`
- Modify: `implementation-notes.md`

**Interfaces:**
- Consumes: complete implementation and approved design.
- Produces: executable RDH static invariants, CI focused-test gate, and permanent upgrade/runtime documentation.

- [ ] **Step 1: Write failing RDH source-contract assertions**

Extend `tests/test_herbin_branding.py` to read every new module and require:

```python
assert 'mod headless_file_transfer;' in lib_rs
assert 'ConnType::FILE_TRANSFER' in file_runtime_rs
assert 'Data::SendFiles' not in file_runtime_rs  # runtime must use FileManager, not bypass it
assert 'file_transfer_job_completed' in ui_session_interface_rs
assert 'ReadRemoteDir' in file_state_rs
assert '--password' in file_args_rs
assert 'stdin_is_tty' in file_state_rs
assert 'cargo test --locked --lib headless_file_transfer' in macos_workflow
assert 'Headless file transfer CLI' in upgrade_runbook
```

Require absence of `base64`, automatic resume/reconnect markers, plaintext password flags, direct protobuf edits, and Flutter changes in the feature modules. Add mutation cases proving the contract rejects removal of the Flutter-dispatch exclusion, insecure-connection rejection, default overwrite refusal, and remote postflight.

- [ ] **Step 2: Run the static contract and verify RED**

```bash
python3 tests/test_herbin_branding.py
```

Expected: FAIL because CI and documentation do not yet contain the new contract.

- [ ] **Step 3: Add the CI focused-test gate**

Before the existing headless terminal test or in the same step, run both filters explicitly:

```yaml
      - name: Test RDH headless CLIs
        run: |
          MACOSX_DEPLOYMENT_TARGET=10.14 \
            cargo test --locked --lib headless_file_transfer \
              --features flutter,hwcodec,unix-file-copy-paste,screencapturekit \
              -- --nocapture
          MACOSX_DEPLOYMENT_TARGET=10.14 \
            cargo test --locked --lib headless_terminal \
              --features flutter,hwcodec,unix-file-copy-paste,screencapturekit \
              -- --nocapture
```

- [ ] **Step 4: Document the permanent CLI contract**

Add exact invocation, stdout/stderr rules, exit statuses, default overwrite refusal, no-resume behavior, saved-credential/non-TTY behavior, cancellation, and external SHA-256 acceptance boundary to `docs/rdh-upgrade-runbook.md`. Add a concise cumulative section to `implementation-notes.md` containing the approved decisions and `Open questions: none`; do not add task narration, PIDs, or transient diagnostics.

- [ ] **Step 5: Run contract and documentation checks**

```bash
python3 tests/test_herbin_branding.py
git diff --check
```

Expected: PASS with no output from the Python invariant and no whitespace errors.

- [ ] **Step 6: Commit contract and documentation**

```bash
git add tests/test_herbin_branding.py .github/workflows/codex-macos-herbin.yml docs/rdh-upgrade-runbook.md implementation-notes.md
git commit -m "docs: define headless file transfer contract"
```

### Task 10: Run aggregate source verification and build the exact CI candidate

**Files:**
- Verify only; no source edits unless a listed verification reveals an in-scope defect.

**Interfaces:**
- Consumes: complete feature branch.
- Produces: clean source evidence and a source-bound macOS candidate artifact.

- [ ] **Step 1: Run formatting and focused tests from a clean environment**

```bash
set -euo pipefail
source /Volumes/DevData/Development/RustDesk-Herbin/tools/devdata-env.zsh
cargo fmt --check
cargo test --locked --lib headless_file_transfer -- --nocapture
cargo test --locked --lib headless_terminal -- --nocapture
cargo test --locked --lib core_main::tests -- --nocapture
python3 tests/test_herbin_branding.py
git diff --check
```

Expected: every command exits 0; terminal baseline remains at least 54 passing tests.

- [ ] **Step 2: Verify exact scope and commit state**

```bash
git status --short --branch
git diff --stat cc5b57d..HEAD
git diff --name-status cc5b57d..HEAD
git log --oneline cc5b57d..HEAD
git submodule status --recursive
```

Require a clean worktree, only planned source/docs/workflow changes, and unchanged `libs/hbb_common` submodule commit.

- [ ] **Step 3: Push the task branch non-force to the personal fork**

```bash
git remote -v
git push -u fork feature/headless-file-transfer-cli
```

Require `fork` to resolve to `Herbin-s/rustdesk` and `origin` to official RustDesk. Do not push to `origin`.

- [ ] **Step 4: Dispatch and follow the CI candidate build**

Resolve the next unused RDH revision from all retained artifacts immediately before dispatch, then dispatch:

```bash
rdh_before_runs="$(mktemp /tmp/rdh-file-transfer-before-runs.XXXXXX)"
gh run list \
  --repo Herbin-s/rustdesk \
  --workflow codex-macos-herbin.yml \
  --event workflow_dispatch \
  --limit 100 \
  --json databaseId \
  --jq '.[].databaseId' | sort -n > "$rdh_before_runs"
rdh_revision="$(
  gh api --paginate 'repos/Herbin-s/rustdesk/actions/artifacts?per_page=100' \
    --jq '.artifacts[].name' |
    sed -nE 's/.*-rdh\.([0-9]+)-aarch64-adhoc$/\1/p' |
    sort -n |
    tail -1 |
    awk '{print $1 + 1}'
)"
test -n "$rdh_revision"
gh workflow run codex-macos-herbin.yml \
  --repo Herbin-s/rustdesk \
  --ref master \
  -f source_ref=feature/headless-file-transfer-cli \
  -f "rdh_revision=$rdh_revision"

rdh_run_id=
for rdh_attempt in {1..30}; do
  rdh_current_runs="$(mktemp /tmp/rdh-file-transfer-current-runs.XXXXXX)"
  gh run list \
    --repo Herbin-s/rustdesk \
    --workflow codex-macos-herbin.yml \
    --event workflow_dispatch \
    --limit 100 \
    --json databaseId \
    --jq '.[].databaseId' | sort -n > "$rdh_current_runs"
  rdh_new_runs="$(comm -13 "$rdh_before_runs" "$rdh_current_runs")"
  rm "$rdh_current_runs"
  rdh_new_run_count="$(printf '%s\n' "$rdh_new_runs" | sed '/^$/d' | wc -l | tr -d ' ')"
  if [ "$rdh_new_run_count" -eq 1 ]; then
    rdh_run_id="$rdh_new_runs"
    break
  fi
  test "$rdh_new_run_count" -eq 0
  sleep 2
done
rm "$rdh_before_runs"
test -n "$rdh_run_id"
gh run watch "$rdh_run_id" --repo Herbin-s/rustdesk --exit-status
```

This refuses to guess if more than one workflow dispatch appears in the capture window. At completion, the downloaded metadata must still bind `source_ref` indirectly through the exact feature-branch commit and bind the selected revision directly. If CI fails, fix only an in-scope defect, rerun focused verification, commit, push non-force, and dispatch a new revision.

- [ ] **Step 5: Verify the artifact offline**

Download and verify the exact successful run artifact:

```bash
rdh_candidate_dir="/Users/herbin/Library/Caches/RustDesk-Herbin/headless-file-transfer-$rdh_run_id"
test ! -e "$rdh_candidate_dir"
mkdir -p "$rdh_candidate_dir"
gh run download "$rdh_run_id" --repo Herbin-s/rustdesk --dir "$rdh_candidate_dir"

rdh_candidate_dmg="$(find "$rdh_candidate_dir" -type f -name 'rustdesk-herbin-*-aarch64.dmg' -print -quit)"
rdh_candidate_checksum="$rdh_candidate_dmg.sha256"
rdh_candidate_build_metadata="$(find "$rdh_candidate_dir" -type f -name rdh-build-metadata.txt -print -quit)"
test -f "$rdh_candidate_dmg"
test -f "$rdh_candidate_checksum"
test -f "$rdh_candidate_build_metadata"

(
  cd "$(dirname "$rdh_candidate_dmg")"
  shasum -a 256 -c "$(basename "$rdh_candidate_checksum")"
)
rdh_candidate_sha256="$(shasum -a 256 "$rdh_candidate_dmg" | awk '{print $1}')"
rdh_source_commit="$(awk -F= '$1 == "source_commit" { print $2 }' "$rdh_candidate_build_metadata")"
rdh_metadata_revision="$(awk -F= '$1 == "rdh_revision" { print $2 }' "$rdh_candidate_build_metadata")"
test "$rdh_source_commit" = "$(git rev-parse HEAD)"
test "$rdh_metadata_revision" = "$rdh_revision"
test "$(awk -F= '$1 == "signature" { print $2 }' "$rdh_candidate_build_metadata")" = ad-hoc
test "$(awk -F= '$1 == "notarized" { print $2 }' "$rdh_candidate_build_metadata")" = false

rdh_verify_mount="$(mktemp -d /tmp/rdh-file-transfer-verify.XXXXXX)"
hdiutil attach "$rdh_candidate_dmg" -readonly -nobrowse -mountpoint "$rdh_verify_mount"
rdh_verify_app="$rdh_verify_mount/RustDesk-Herbin.app"
file "$rdh_verify_app/Contents/MacOS/RustDesk-Herbin" | rg 'Mach-O 64-bit executable arm64'
test "$(defaults read "$rdh_verify_app/Contents/Info" CFBundleIdentifier)" = com.herbin.rustdesk
codesign --verify --deep --strict --verbose=4 "$rdh_verify_app"
codesign -dv --verbose=4 "$rdh_verify_app" 2>&1 | rg 'Signature=adhoc'
hdiutil detach "$rdh_verify_mount"
rmdir "$rdh_verify_mount"
```

Do not install the app or launch a server.

- [ ] **Step 6: Record candidate evidence outside Git**

Create `/Volumes/DevData/Development/RustDesk-Herbin/artifacts/headless-file-transfer-cli/candidate.json` with the exact successful run ID, artifact ID, source commit, RDH revision, downloaded DMG path, and verified SHA-256:

```bash
rdh_artifact_id="$(
  gh api "repos/Herbin-s/rustdesk/actions/runs/$rdh_run_id/artifacts" \
    --jq '.artifacts | if length == 1 then .[0].id else empty end'
)"
test -n "$rdh_artifact_id"
rdh_evidence_dir=/Volumes/DevData/Development/RustDesk-Herbin/artifacts/headless-file-transfer-cli
mkdir -p "$rdh_evidence_dir"
jq -n \
  --argjson run_id "$rdh_run_id" \
  --argjson artifact_id "$rdh_artifact_id" \
  --arg source_commit "$rdh_source_commit" \
  --arg rdh_revision "$rdh_revision" \
  --arg dmg_path "$rdh_candidate_dmg" \
  --arg sha256 "$rdh_candidate_sha256" \
  '{run_id: $run_id, artifact_id: $artifact_id, source_commit: $source_commit, rdh_revision: $rdh_revision, dmg_path: $dmg_path, sha256: $sha256}' \
  > "$rdh_evidence_dir/candidate.json"
jq -e . "$rdh_evidence_dir/candidate.json" >/dev/null
```

The file is task-owned evidence outside Git and contains no credentials. Keep mount cleanup and the same values in the task handoff; do not add transient IDs to the stable root contract or source history.

### Task 11: Run real CLI acceptance and publish a Draft PR

**Files:**
- Create temporary local probe files under a directory returned by `mktemp -d`.
- Create and remove one task-owned remote test directory under the work PC user's Downloads directory.
- Update outside Git after acceptance: `/Volumes/DevData/Development/RustDesk-Herbin/AGENTS.md` stable RDH patch category.

**Interfaces:**
- Consumes: verified mounted CI candidate, saved peer credentials, existing installed headless terminal, and work PC peer `175116438`.
- Produces: decisive push/pull/overwrite/cancel/relay evidence, stable project guidance, and a Draft PR.

- [ ] **Step 1: Freeze rescue and runtime baselines**

Confirm official RustDesk remains available, the installed RDH bundle is untouched, peer `175116438` is reachable, and no candidate server process is running. Read the candidate DMG path and SHA-256 from the exact task metadata, recheck the hash, mount it read-only at a new task-owned mount point, and set the executable path deterministically:

```bash
set -euo pipefail
rdh_candidate_metadata=/Volumes/DevData/Development/RustDesk-Herbin/artifacts/headless-file-transfer-cli/candidate.json
rdh_candidate_dmg="$(jq -er '.dmg_path' "$rdh_candidate_metadata")"
rdh_expected_sha256="$(jq -er '.sha256' "$rdh_candidate_metadata")"
test "$(shasum -a 256 "$rdh_candidate_dmg" | awk '{print $1}')" = "$rdh_expected_sha256"
rdh_mount_dir="$(mktemp -d /tmp/rdh-file-transfer-candidate.XXXXXX)"
hdiutil attach "$rdh_candidate_dmg" -readonly -nobrowse -mountpoint "$rdh_mount_dir"
rdh_candidate_binary="$rdh_mount_dir/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin"
test -x "$rdh_candidate_binary"
rdh_cleanup_candidate_mount() {
  hdiutil detach "$rdh_mount_dir" >/dev/null 2>&1 || true
  rmdir "$rdh_mount_dir" >/dev/null 2>&1 || true
}
trap rdh_cleanup_candidate_mount EXIT INT TERM
```

Execute only `$rdh_candidate_binary` for file-transfer commands; do not copy it into `/Applications`. The exit trap ejects only `$rdh_mount_dir` and removes that empty mount directory.

- [ ] **Step 2: Create deterministic local probes**

Create a small file containing NUL, invalid UTF-8 bytes, and deterministic content, plus a deterministic large file of at least 64 MiB. Use a Chinese filename for the small probe. Record local byte counts and SHA-256 without printing file content. Define every acceptance path once:

```bash
rdh_acceptance_stamp="$(date +%Y%m%d-%H%M%S)"
rdh_local_tmp="$(mktemp -d /tmp/rdh-file-transfer-acceptance.XXXXXX)"
rdh_local_probe="$rdh_local_tmp/中文-probe.bin"
rdh_local_pull="$rdh_local_tmp/中文-pulled.bin"
rdh_local_large="$rdh_local_tmp/large-probe.bin"
rdh_remote_dir="C:\\Users\\82520\\Downloads\\RDH-CLI-Acceptance-$rdh_acceptance_stamp"
rdh_remote_probe="$rdh_remote_dir\\中文-probe.bin"
```

Generate deterministic bytes and require exact sizes before connecting:

```bash
/usr/bin/printf '\x00\xff\xfeRDH-native-file-transfer\r\n' > "$rdh_local_probe"
dd if=/dev/zero of="$rdh_local_large" bs=1048576 count=64 status=none
test "$(wc -c < "$rdh_local_probe" | tr -d ' ')" -eq 29
test "$(wc -c < "$rdh_local_large" | tr -d ' ')" -eq 67108864
rdh_local_probe_sha256="$(shasum -a 256 "$rdh_local_probe" | awk '{print $1}')"
rdh_local_large_sha256="$(shasum -a 256 "$rdh_local_large" | awk '{print $1}')"
test -n "$rdh_local_probe_sha256"
test -n "$rdh_local_large_sha256"
```

- [ ] **Step 3: Push and independently verify the small probe**

Run the mounted candidate:

```bash
"$rdh_candidate_binary" \
  --file-transfer --headless 175116438 push \
  "$rdh_local_probe" \
  "$rdh_remote_probe"
```

Require exit 0, stdout exactly the remote destination plus newline, bounded stderr progress, and no Flutter window. Use the installed `rdh --terminal --headless 175116438` only to run sanitized `Get-Item` and `Get-FileHash` checks; require exact size and SHA-256.

- [ ] **Step 4: Pull the probe and compare all hashes**

Pull to a new non-existing local path. Require exit 0, stdout exactly that local destination, identical bytes, and identical SHA-256 across original local, remote, and pulled local files.

- [ ] **Step 5: Verify conflict and overwrite semantics**

Push changed content to the existing remote target without `--overwrite`; require exit 7 and unchanged remote SHA-256. Repeat with `--overwrite`; require exit 0 and the new remote SHA-256. Repeat equivalent local-destination checks for pull.

- [ ] **Step 6: Verify large-file native transfer and cancellation**

Push the 64 MiB probe and confirm progress contains byte counts but no Base64/raw blocks. Start a second large transfer, send SIGINT, require status 130, and inspect that normal cancellation removed its remote `.download` state. Do not claim cleanup if the transport was interrupted before cancel delivery.

- [ ] **Step 7: Verify error and relay paths**

Require status 6 for a missing remote source. If the peer exposes a safe task-owned unwritable location, verify permission failure there; otherwise report permission failure unverified rather than changing permissions. Run one successful small-file transfer with `--relay` and verify the exact result.

- [ ] **Step 8: Verify output, logs, and GUI absence**

Confirm no Flutter window appeared, stdout was empty on all failures, and logs/output contain no plaintext credential, file content, raw block, or Base64 payload. Separate external SHA-256 evidence from the CLI's normal success contract.

- [ ] **Step 9: Clean task-owned acceptance files**

Remove only the exact local temporary directory and exact timestamped remote acceptance directory after recording hashes and statuses. Confirm neither exists. Do not alter user files, CVR profiles, or unrelated Downloads content.

- [ ] **Step 10: Update the aggregate-root stable RDH contract**

Add one concise headless file-transfer category to `/Volumes/DevData/Development/RustDesk-Herbin/AGENTS.md`: native `FILE_TRANSFER`, push/pull one regular file, saved-credential script mode, default no-overwrite, no resume/reconnect, stdout/stderr separation, no protocol change, and real acceptance boundary. This file is outside Git; verify only the intended section changed.

- [ ] **Step 11: Finish the branch and open a Draft PR**

Invoke `superpowers:finishing-a-development-branch`. Re-run the aggregate verification, push any final non-force commit to `fork`, create or update a Draft PR targeting the user's fork branch appropriate for RDH integration, and stop. Do not merge, tag, release, install, or dispatch an independent review.

- [ ] **Step 12: Deliver the final handoff**

Report branch, commits, Draft PR, CI run and source-bound artifact identity, every verification layer separately, real push/pull acceptance, cancellation/relay/error coverage, remote cleanup, installed-state safety, and any explicitly unverified layer. State that no Git action remains for the user.
