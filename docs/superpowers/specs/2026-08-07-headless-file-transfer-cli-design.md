# RDH Headless File Transfer CLI Design

Date: 2026-08-07
Status: Approved by the user on 2026-08-07

## Objective

Add a macOS controller-side CLI mode to RustDesk-Herbin (RDH) that transfers one
regular file to or from a RustDesk peer without starting Flutter, opening a
window, or encoding the file through the remote terminal. The implementation
must reuse the existing RustDesk file-transfer connection, authentication,
encryption, relay, job, and block-transfer paths.

The initial user path is transferring files between the Mac and the saved work
PC peer that is already used by `rdh --terminal --headless`. The new path must
eliminate terminal Base64 chunking while preserving ordinary GUI file transfer
and the existing headless terminal CLI.

## Scope

The first version includes:

- `push` from one local regular file to one explicit remote file path;
- `pull` from one explicit remote regular file path to one local file path;
- saved-credential authentication, secure interactive retry, and 2FA;
- operation without a TTY when saved credentials are sufficient;
- optional forced relay transport;
- deterministic destination-conflict handling through `--overwrite`;
- bounded progress on stderr and one stable success value on stdout;
- graceful cancellation and deterministic exit statuses.

The first version does not include:

- directories, recursive transfer, multiple sources, or wildcard expansion;
- stdin/stdout as file-content streams;
- automatic retry, reconnect, or resume;
- remote shell execution or path expansion;
- list, create-directory, remove, rename, or general file-manager commands;
- plaintext passwords in argv or environment variables;
- a hidden Flutter window or dependency on a running RDH GUI process;
- a protobuf or controlled-side protocol change;
- an end-to-end SHA-256 response in the CLI success contract;
- installation or replacement of the currently running RDH application as part
  of source implementation.

## Command-Line Contract

The supported commands are:

```text
RustDesk-Herbin --file-transfer --headless [--relay] [--overwrite] <peer-id> push <local-file> <remote-file>
RustDesk-Herbin --file-transfer --headless [--relay] [--overwrite] <peer-id> pull <remote-file> <local-file>
```

`rdh` forwards this argument vector unchanged to the application binary.
`--relay` and `--overwrite` are optional flags before the peer ID. The four
positionals remain ordered as peer ID, operation, source, and destination.

`--file-transfer <peer-id>` without `--headless` retains the RDO Flutter file
manager behavior. `--terminal --headless` retains the existing RDH terminal CLI
behavior. The new classifier must claim only the combined
`--file-transfer --headless` form before the ordinary Flutter/IPC dispatch.

The command rejects missing or extra positionals, unknown flags, unsupported
operations, `--password`, directories, symlinks, and special local source
files. Paths containing spaces or shell metacharacters must be quoted by the
invoking shell. RDH treats remote paths as opaque protocol fields: it does not
execute a shell or expand `~`, environment variables, or wildcards.

## Architecture

### Module boundary

Add a focused `headless_file_transfer` module alongside, not inside,
`headless_terminal`:

1. `args.rs` contains pure command classification and validation.
2. `handler.rs` implements `InvokeUiSession` and maps connection,
   authentication, file metadata, progress, overwrite, completion, and failure
   callbacks into typed events.
3. `runtime.rs` owns one transfer state machine, the file preconditions, the
   `Session<HeadlessFileTransferHandler>`, signal handling, output, and exit
   mapping.

Authentication prompt and secret-reading behavior may be extracted into one
narrow headless helper when that removes concrete duplication. Do not refactor
the working terminal lifecycle state machine or create a broad frontend
framework for this feature.

### Existing RDO/RDH reuse

The runtime initializes the session with `ConnType::FILE_TRANSFER` and runs the
existing `client::io_loop`. It drives the existing `FileManager` /
`Data::SendFiles` boundary and lets `hbb_common::fs::TransferJob` retain
ownership of file enumeration, 128 KiB block reads, compression, overwrite
digests, temporary downloads, progress counters, and completion.

The feature does not reimplement rendezvous, transport security, password
hashing, relay negotiation, file blocks, or controlled-side writes. It does not
change `message.proto` or require the controlled peer to run a new server
implementation.

## Runtime State Machine

One process owns one job:

```text
Parsed -> Authenticating -> Ready -> Transferring -> Finalizing -> Done
                         \-> Failed <-----------------------------/
```

- `Parsed` has a valid command and passed all available local preconditions.
- `Authenticating` owns connection and credential prompts.
- `Ready` means the file-transfer session is authenticated and accepts exactly
  one job start.
- `Transferring` accepts metadata, overwrite, progress, cancellation, and job
  failure events for the expected job ID.
- `Finalizing` requires native completion and verifies the local byte-count and
  source metadata invariants.
- `Done` or `Failed` closes the transport and returns exactly once.

Events for an unexpected job ID, duplicate job start, completion before the
job is active, or completion with an incomplete byte count are protocol
failures. The runtime never converts them into retries.

## Authentication and Transport Security

The command uses the existing RustDesk credential ordering and hashing flow.
Saved credentials allow fully non-interactive operation; neither stdin nor
stdout needs to be a TTY in that case.

When a password retry or 2FA code is required, the runtime prompts only if
stdin is a TTY. Password input has echo disabled. A successful manually entered
password may be saved through the existing peer credential flow after the same
explicit confirmation used by the terminal CLI. If a prompt is required
without a TTY, the command exits with authentication status 4.

The parser rejects `--password`. Plaintext credentials must never appear in
argv, environment variables, process titles, stdout, stderr, logs, branch names,
commit messages, or PR text. An insecure or unverifiable transport is rejected;
the first version has no bypass flag.

## Push Data Flow

1. Validate that the local source exists and is one regular, non-symlink file.
2. Snapshot its size and modification metadata for completion checks.
3. Establish and authenticate a `FILE_TRANSFER` session.
4. Start one existing local-read/remote-write job with `is_remote = false` and
   the explicit remote destination path.
5. Let the existing transfer job send native binary blocks and compression
   metadata.
6. On a destination digest/conflict:
   - without `--overwrite`, send the existing skip confirmation, retain a
     destination-conflict result, and exit 7 after job cleanup;
   - with `--overwrite`, confirm offset block 0 and continue from the beginning.
7. After the expected native done event, complete source byte count, and
   unchanged local source metadata, issue the existing `ReadDir` request for
   the destination parent.
8. Accept success only when that directory response contains the exact target
   name as one regular file with the expected size.

The receiver retains its existing temporary-download and finalization behavior.
The post-transfer `ReadDir` is verification only. The CLI introduces no remote
directory creation or other directory-management operation; any parent handling
remains the current RDO receiver behavior.

## Pull Data Flow

1. Reject an existing local destination unless `--overwrite` is present.
2. Require the local destination parent to exist and be a directory; the CLI
   does not create it.
3. Establish and authenticate a `FILE_TRANSFER` session.
4. Ask the remote side to send the exact source path through one existing
   remote-read/local-write job with `is_remote = true`.
5. Reject remote metadata that describes a directory, link, or more than one
   file.
6. Apply the same destination-conflict rule: default failure, or offset block 0
   only with `--overwrite`.
7. Accept success only after the native done event, complete received byte
   count, and the existing temporary-download finalization to the requested
   local destination.

Remote file-not-found, permission, one-way-transfer, and invalid-file-type
responses remain transfer-job failures. They are not retried through a terminal
or another protocol.

## Overwrite, Partial Files, and Cancellation

Destination existence is never reported as success, even if the peer reports
that the files may be identical. Without `--overwrite`, the command exits 7.
With `--overwrite`, it always starts at offset block 0. There is no implicit
resume path in the first version.

`Ctrl+C` sends the existing `FileTransferCancel`, removes the local job through
the current cancellation path, closes the transport, and exits 130. `SIGTERM`
uses the same orderly path and exits 143. A normally delivered cancellation
uses existing receiver cleanup for `.download` state.

An abrupt connection loss exits 5 without reconnecting. Because the remote
cancel may not have arrived, diagnostics must state that a peer-side partial may
remain; the command must not claim cleanup it did not observe. A later explicit
`--overwrite` may replace stale partial state from offset 0.

## Output Contract

stdout and stderr have stable, separate responsibilities:

- stdout is empty during transfer and on every failure;
- on success, stdout contains only the exact destination path followed by one
  newline;
- stderr contains secure prompts, connection diagnostics, bounded progress, and
  failure messages;
- progress is emitted no more often than the existing one-second job-status
  cadence and contains direction, completed bytes, total bytes, percentage, and
  speed, but never file content or raw blocks.

The success path does not claim a cryptographic end-to-end digest. RustDesk
transport security, the native done event, complete byte count, and destination
finalization form the version-one command contract. External SHA-256 comparison
is required during real acceptance but is not a normal command prerequisite.

## Exit Status

- `0`: native transfer completion and all local completion invariants passed;
- `1`: unclassified internal failure;
- `2`: command-line usage or parser failure;
- `3`: local file or path precondition failure;
- `4`: authentication cancellation or failure;
- `5`: connection, transport-security, transport, or protocol-state failure;
- `6`: file-not-found, filesystem, permission, one-way-policy, or transfer-job
  failure;
- `7`: destination exists without `--overwrite`;
- `130`: local `SIGINT` / `Ctrl+C` cancellation;
- `143`: local `SIGTERM` cancellation.

Every failure includes actionable context on stderr without credentials or file
content. stdout remains empty.

## Testing

### Parser and state-machine tests

Focused unit tests cover:

- push, pull, relay, overwrite, and the supported positional order;
- missing, duplicate, malformed, and unsupported arguments;
- rejection of `--password` and ordinary GUI/terminal command non-ownership;
- saved-credential success, interactive authentication, 2FA, and no-TTY prompt
  failure;
- single job start, expected progress, conflict handling, native completion,
  and every exit status;
- wrong job IDs, duplicate or out-of-order events, incomplete byte counts,
  connection loss, SIGINT, and SIGTERM;
- stdout/stderr separation and success-path destination output.

### Filesystem and in-memory integration tests

Use temporary directories, fake filesystem boundaries, and channel-driven
session events to cover:

- regular-file acceptance and directory, symlink, and special-file rejection;
- existing destination failure and explicit overwrite from offset 0;
- missing destination parent on pull;
- metadata change during push;
- remote directory/link/multiple-file metadata rejection during pull;
- graceful partial cleanup and abrupt-disconnect limitation reporting;
- push and pull event ordering without opening a real connection.

Tests must not change the runner's real credentials, TTY, signals, or peer
configuration.

### Project and CI verification

Run the focused Rust tests under the project environment, `cargo fmt --check`,
`tests/test_herbin_branding.py`, and `git diff --check`. Extend the RDH macOS CI
workflow so the headless file-transfer tests run before the release build.

Regression coverage must prove that ordinary `--file-transfer <peer-id>` still
reaches Flutter and that all existing `headless_terminal` tests remain green.
The CI candidate retains the existing source revision, RDH identity, arm64,
ad-hoc-signature, DMG checksum, and metadata checks.

### Real acceptance

Installation remains a separate authorization boundary. After a verified CI
candidate exists and the official RustDesk rescue route plus rollback asset are
ready, test against the work PC peer:

1. Run push without a Flutter window using a saved credential.
2. Transfer a probe containing NUL bytes, non-UTF-8 bytes, and a Chinese
   filename.
3. Independently calculate the remote SHA-256, pull the file to a new local
   path, and compare both local SHA-256 values.
4. Transfer a large file and prove that no Base64 or terminal chunking is used.
5. Verify default destination conflict exits 7 without changing the target.
6. Verify `--overwrite` replaces the target from the beginning.
7. Verify missing-file, permission, and one-way-policy failures.
8. Cancel a large transfer and inspect partial cleanup.
9. Repeat one successful transfer with `--relay`.
10. Confirm no Flutter window, plaintext credential, file content, or raw block
    appears in output or logs.

The external SHA-256 checks are acceptance evidence only; they do not silently
expand the version-one protocol contract.

## Repository and Delivery Boundaries

Implementation targets an isolated worktree based on RDH commit `cc5b57d`,
which contains the approved headless terminal CLI. The existing dirty
`rdh/1.4.9` checkout and its `implementation-notes.md` remain untouched.

After an implementation plan is separately approved, the agent owns routine
worktree, branch, commit, personal-fork push, Draft PR, and CI operations under
the project `AGENTS.md`. Installation, replacement of the running RDH bundle,
release tags, protected-branch promotion, and live work-PC mutation remain their
separate explicit authorization boundaries.

## Open Questions

None. Product direction, CLI shape, transfer directions, file scope, overwrite
behavior, authentication mode, output contract, integrity level, architecture,
error handling, and acceptance criteria were confirmed by the user before this
document was written.
