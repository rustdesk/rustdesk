# RDH Headless Terminal CLI Design

Date: 2026-08-05
Status: Approved in conversation; awaiting written-spec review

## Objective

Add a macOS controller-side CLI mode to RustDesk-Herbin (RDH) that opens an
interactive remote terminal without starting Flutter or creating a window.
The initial acceptance target is the saved RDH peer `175116438`, whose remote
host is Windows and whose terminal backend is the existing RustDesk
TerminalService/ConPTY implementation.

The primary invocation is:

```bash
RustDesk-Herbin --terminal --headless 175116438
```

The result should feel like an SSH interactive terminal while continuing to
use RustDesk authentication, rendezvous, encryption, saved peer configuration,
and the existing TerminalAction/TerminalResponse protocol.

## Scope

The first version includes:

- interactive terminal input and output on a local macOS TTY;
- both `--terminal --headless <peer-id>` and
  `--terminal <peer-id> --headless` argument orderings;
- existing saved-password and local address-book credential resolution;
- secure no-echo password retry and optional password persistence;
- interactive 2FA entry when the peer requires it;
- local terminal raw mode, remote control-byte forwarding, and size updates;
- explicit persistent-session detach with `--persistent`;
- existing `--relay` behavior;
- deterministic cleanup, diagnostics, and exit statuses.

The first version does not include:

- one-shot remote command execution;
- a non-interactive stdin/stdout pipe mode;
- passwords supplied through argv or environment variables;
- a hidden Flutter window;
- a dependency on an already-running RDH GUI process;
- headless `--terminal-admin` support;
- changes to the remote terminal protobuf, Windows helper, ConPTY service, or
  Flutter terminal UI;
- installation or replacement of the currently running RDH as part of source
  implementation.

## Command-Line Contract

The parser accepts one peer ID and the following flags in any order after the
executable name:

```text
--terminal --headless [--relay] [--persistent] <peer-id>
```

These commands are equivalent:

```bash
RustDesk-Herbin --terminal --headless 175116438
RustDesk-Herbin --terminal 175116438 --headless
```

`--terminal <peer-id>` without `--headless` retains its current behavior and
opens the Flutter terminal window. The headless combination is recognized
before the existing Flutter URL/IPC dispatch so `--headless` can never be
mistaken for the peer ID.

The command rejects missing IDs, multiple positional IDs, unknown flags,
`--password`, and `--terminal-admin`. Usage failures print a concise usage line
to stderr and exit with status 2.

Interactive mode requires both stdin and stdout to be TTYs. A missing TTY is a
local precondition failure, reported on stderr with status 3. This constraint
keeps the first version's byte stream and terminal-size behavior deterministic;
Codex PTY sessions and normal Terminal.app sessions satisfy it.

## Architecture

### CLI dispatch

`core_main` gains a narrow headless-terminal argument check before
`core_main_invoke_new_connection`. A matching invocation initializes a CLI log
scope, runs the headless terminal runtime, maps its result to a process status,
and returns without starting Flutter.

The argument parser is a pure unit-testable component. It must not modify the
existing behavior of other connection flags.

### Headless terminal module

A new module, expected at `src/headless_terminal.rs`, contains four focused
components:

1. `HeadlessTerminalArgs` parses and validates the command contract.
2. `HeadlessTerminalRuntime` owns lifecycle state and coordinates the network
   session, authentication prompts, stdin, resize events, stdout, and shutdown.
3. `HeadlessTerminalHandler` implements `InvokeUiSession` for
   `Session<HeadlessTerminalHandler>`. Only connection, prompt, and terminal
   callbacks perform work; unrelated video, mouse, file-transfer, and Flutter
   callbacks are explicit no-ops.
4. `LocalTtyGuard` captures the original local terminal attributes, enters raw
   mode only after `TerminalOpened(success=true)`, and restores the attributes
   in `Drop` before any user-visible exit.

The module reuses `Session`, `LoginConfigHandler`, `client::io_loop`, and
`ConnType::TERMINAL`. It does not reimplement rendezvous, transport security,
password hashing, or the terminal wire protocol.

### Runtime data flow

1. Parse arguments and verify the local TTY.
2. Initialize a terminal `Session` for the peer and load existing peer config.
3. Resolve credentials using RDH's existing ordering. If no usable local
   credential is available, prompt for the RustDesk password with echo disabled
   and ask `Save for this peer? [y/N]`.
4. Start the existing RustDesk client I/O loop with `ConnType::TERMINAL`.
5. On authenticated peer info, send `OpenTerminal` for terminal ID 1 using the
   current local rows and columns.
6. On successful `TerminalOpened`, enter raw mode and begin forwarding stdin
   bytes as `TerminalData`.
7. Write remote `TerminalData` bytes directly to stdout without UTF-8
   conversion or log decoration.
8. Translate local terminal size changes into `ResizeTerminal`.
9. On remote `TerminalClosed`, connection failure, local detach, or process
   termination, restore the local TTY before printing diagnostics and exiting.

`TerminalOpened.service_id` continues to be stored through the existing peer
option path. A later `--persistent` connection therefore reuses the existing
server-side service and reconnection buffer without a new persistence format.

## Authentication and Security

The headless session uses the existing RustDesk credential sources and hashing
flow. It never prints, logs, or places a plaintext password in argv, environment
variables, or process titles.

If no saved credential is available, the runtime prompts before raw mode. A
positive save answer sets the existing `remember` behavior so the successfully
derived peer hash is stored by `LoginConfigHandler::handle_peer_info`; the
plaintext is never persisted.

If a saved credential is rejected, the handler requests one secure retry at a
time through the existing `Data::Login` path. Repeated failures remain
interactive until the user cancels with the local escape key or EOF at the
credential prompt. A required 2FA code is prompted separately and sent through
the existing `Auth2FA` message. Trusting the device is not enabled implicitly.

An insecure transport prompt fails closed by default. The first version does
not silently continue an unencrypted or unverifiable connection.

## TTY and Interaction Semantics

Raw mode starts only after the remote terminal has opened. In raw mode:

- input bytes are forwarded unchanged, including escape sequences, arrow keys,
  Backspace, `Ctrl+C` (`0x03`), and `Ctrl+D` (`0x04`);
- local signal generation for those control bytes is disabled so they reach
  PowerShell or the active remote TUI;
- `Ctrl+]` (`0x1d`) is reserved as the local detach escape and is not forwarded;
- remote output is flushed to stdout as bytes;
- local size changes send the latest rows and columns to the remote PTY.

Size notifications should use an event-driven `SIGWINCH` watcher on macOS. A
small direct `signal-hook` dependency is acceptable if needed; the runtime must
not add a permanent polling loop outside an active headless session.

On `Ctrl+]` without `--persistent`, the client sends `CloseTerminal` and then
closes the transport. With `--persistent`, it closes only the transport so the
remote TerminalService retains the PowerShell/ConPTY session. A normal remote
`exit` always ends the corresponding remote terminal and then the local CLI.

The first version performs no automatic reconnect. This prevents a network
failure during a command from being disguised as a new shell session.

## Output and Exit Status

The streams are separated strictly:

- stdout contains only remote terminal bytes;
- stderr contains local prompts, connection diagnostics, and usage errors;
- detailed debug information remains in RDH's normal log files.

Exit statuses are:

- 0 for a clean remote shell exit or an intentional local detach;
- the remote exit code when it is representable in the local range 1 through
  125;
- 1 for any other non-zero or unrepresentable remote exit code;
- 2 for command-line usage errors;
- 3 for missing TTY prerequisites;
- 4 for authentication cancellation or unrecoverable authentication failure;
- 5 for connection, transport, or terminal protocol failure.

Before any return path after raw mode begins, `LocalTtyGuard` must be dropped
and stdout flushed. `SIGTERM` and `SIGHUP` request orderly shutdown through the
runtime; `SIGKILL` cannot be intercepted and is the only signal for which TTY
restoration cannot be guaranteed by the process.

## Error Handling

Boundary errors retain actionable context without secrets. Examples include:

- `headless terminal requires an interactive TTY`;
- `authentication failed for peer 175116438`;
- `peer does not permit terminal connections`;
- `remote terminal failed to open: <remote message>`;
- `connection closed before terminal opened`.

The runtime owns a single terminal state machine:

```text
Parsed -> Authenticating -> Opening -> Active -> Closing -> Closed
                     \-> Failed <--------------------/
```

Only `Active` accepts stdin or resize forwarding. Duplicate opened/closed
responses, terminal data before opened, and responses for a different terminal
ID are protocol failures rather than implicit recovery opportunities.

## Testing

### Unit tests

Focused tests cover:

- both supported argument orderings;
- `--relay` and `--persistent` combinations;
- missing, duplicate, and malformed peer IDs;
- rejection of `--password`, `--terminal-admin`, and unknown flags;
- ordinary `--terminal <id>` remaining on the Flutter path;
- state-machine transitions and rejection of out-of-order responses;
- stdout/stderr separation and byte preservation;
- local detach behavior with and without persistence;
- exit-status mapping;
- terminal attribute restoration on success and injected failures.

TTY syscalls and process signals are hidden behind narrow adapters so lifecycle
tests can use fakes rather than changing the test runner's real terminal.

### In-memory integration tests

Channel-driven tests simulate:

- authentication success and retry;
- 2FA prompt forwarding;
- opened, binary data, resize, and closed responses;
- a connection drop while opening and while active;
- remote shell exit with zero and non-zero status;
- persistent detach without a `CloseTerminal` message.

### Build verification

The implementation runs focused Rust tests and formatting checks locally when
they do not require new downloads. The release candidate is built by the
existing RDH macOS GitHub Actions workflow, which must verify source revision,
RDH identity, arm64 output, ad-hoc signature, DMG checksum, and metadata.

### Real acceptance

After CI produces a verified candidate and before installation, confirm the
official RustDesk rescue connection and preserve a validated rollback copy.
Installation remains a separate, explicitly approved operation.

Using the installed candidate against peer `175116438`, verify:

1. the command creates no Flutter window and does not require the RDH GUI;
2. the saved password authenticates without exposing a secret;
3. PowerShell prompt, typing, paste, arrow keys, Backspace, ANSI output, and
   `Ctrl+C` work;
4. resizing the local terminal updates the remote PTY and redraws a TUI;
5. `exit` restores the local terminal and returns the expected status;
6. default `Ctrl+]` closes the remote terminal;
7. `--persistent` plus `Ctrl+]` detaches, and a later invocation reconnects to
   the same server-side terminal session and buffered output;
8. the existing Flutter terminal path still opens and works normally.

## Repository and Delivery Boundaries

Implementation targets the existing `rdh/1.4.9` worktree and follows its
CI-first release process. The current unrelated `implementation-notes.md`
modification and the older checkout's untracked `poc/` directory are excluded
from headless-terminal commits.

The design does not authorize replacing the installed application, restarting
RDH, pushing a release candidate, or triggering installation. Those operations
remain separate delivery steps after implementation, tests, review, and user
approval.
