# RustDesk-Herbin upgrade runbook

This runbook upgrades RDH to an official stable RustDesk tag while preserving the
small RDH patch set. Builds happen in GitHub Actions. The procedure never replaces
the running application until a candidate artifact and a recovery connection both
exist.

## Patch contract

RDH keeps only these deviations from upstream:

1. `RustDesk-Herbin` branding, bundle ID, URL scheme, config, and launchd isolation.
2. A configurable macOS controlled-side click preprocessor. Ordered rules choose
   `skip`, `forward_only`, or `activate`; only `activate` may order the selected
   regular application or its public-Accessibility window before the original
   remote left-button-down event.
3. A dedicated ad-hoc-signed macOS CI build until Developer ID signing is available.
4. A launchd-gated macOS `--server` memory watchdog that checks once daily at
   06:00 and restarts an over-limit server within the unattended window.

Dock and interactive transient UI, including non-zero-layer menus and popovers,
must not be blanket-filtered. Passive Notification Center overlay recognition
remains an explicit built-in `skip` rule. `mode = "passthrough"` is the A/B
baseline for upstream mouse behavior and performs no targeting lookup.

The supported management operations are `--window-targeting status`,
`--window-targeting validate`, and `--window-targeting reload`. Future upstream
merges must preserve unconditional delivery of the original mouse-down after
preprocessing and must not add file watching or private window APIs.

Custom keyboard mapping and high-volume mouse diagnostics must remain absent.

## Headless terminal CLI

On macOS, run an interactive remote terminal without opening Flutter or creating
a window:

```bash
/Applications/RustDesk-Herbin.app/Contents/MacOS/RustDesk-Herbin \
  --terminal --headless 175116438
```

The peer ID is required. Add `--relay` to force relay transport. Add
`--persistent` to detach without closing the remote terminal service, so a later
headless connection can reuse it; the CLI does not reconnect automatically.
Press `Ctrl+]` to detach. Without `--persistent`, detaching also closes the
remote terminal. A normal remote `exit` always closes it.

Both stdin and stdout must be interactive TTYs; Terminal.app and Codex PTY
sessions satisfy this requirement. Pipe mode is not supported. Remote terminal
bytes are written only to stdout. Local prompts, connection diagnostics, and
usage errors are written only to stderr.

The command uses the existing saved peer credentials. When no usable stored
credential is available, it prompts securely with echo disabled and asks whether
to save the successfully derived peer credential. Supplying a password on the
command line is forbidden: `--password` is rejected as a usage error. Ordinary
`--terminal <peer-id>` without `--headless` keeps the upstream behavior and opens
the Flutter terminal window.

The exit statuses are:

- `0`: clean remote shell exit or intentional local detach;
- `1`: an unrepresentable or otherwise non-zero remote exit status outside the
  pass-through range;
- `2`: command-line usage error, including `--password`;
- `3`: missing stdin/stdout TTY prerequisite;
- `4`: authentication cancellation or unrecoverable authentication failure;
- `5`: connection, transport, or terminal protocol failure.

A representable remote exit status from `1` through `125` is returned unchanged;
therefore a remote command may also produce a numeric status in that range.

## Built-in memory recovery

RDH does not need a Codex or cron automation to recover its leaking user server.
The macOS `--server` process monitors its own physical footprint only when
`XPC_SERVICE_NAME` proves that the RDH launchd agent is supervising it. The value
is public `proc_pid_rusage(RUSAGE_INFO_V0).ri_phys_footprint`.

- The default threshold is 1024 MiB.
- Memory is checked once daily at 06:00 local time.
- 06:00 is inside the configured unattended window of 00:00 through 06:59. Active
  connections are intentionally not checked in this window, so an over-limit
  server restarts even if a remote session happens to be connected.
- If sleep or clock movement delays the 06:00 wake beyond 07:00, that day's check
  is skipped instead of restarting outside the unattended window.
- A `proc_pid_rusage` read error skips that day's decision and never falls back to
  RSS.
- Recovery exits only the user `--server` with a nonzero status. The existing
  launchd `KeepAlive` policy relaunches it; the root service is not restarted and
  no administrator prompt is involved.
- Setting the integrated option `rdh-memory-restart-threshold-mib` to `0` disables
  the watchdog. An invalid value fails closed by disabling it and logging the error.

This is containment, not a claim that the upstream leak is fixed. Keep connection-
level memory profiling as a separate diagnostic effort.

## 1. Check the target

Run `RDH Upstream Upgrade Check` manually, or wait for its weekly run. It resolves
the latest official stable release, rehearses the merge in an ephemeral runner,
and runs the RDH invariant test. It never pushes or installs anything.

To inspect the release locally without changing the checkout:

```bash
gh api repos/rustdesk/rustdesk/releases/latest \
  --jq '{tag: .tag_name, published_at: .published_at, url: .html_url}'
```

Read the official release notes and prioritize security, macOS input/permission,
clipboard, connection, and protocol changes. A routine release can wait several
days for upstream regressions to surface.

## 2. Prepare an isolated candidate

The existing checkout may contain valuable untracked files. Do not clean, reset,
restore, or switch it. Start only when `git status --porcelain` has been reviewed.

```bash
REPO="$HOME/Develop/my-rustdesk-win"
TARGET_TAG="1.4.10"
CANDIDATE_BRANCH="rdh/candidate-${TARGET_TAG}"
WORKTREE="$HOME/.config/superpowers/worktrees/rustdesk/rdh-${TARGET_TAG}"

git -C "$REPO" fetch origin "+refs/tags/${TARGET_TAG}:refs/tags/${TARGET_TAG}"
git -C "$REPO" fetch fork master
git -C "$REPO" worktree add -b "$CANDIDATE_BRANCH" "$WORKTREE" fork/master
git -C "$WORKTREE" merge --no-ff "$TARGET_TAG"
```

Resolve conflicts by preserving current upstream behavior first, then reapplying
the patch contract above. Never recover an entire conflicted input or keyboard file
from the old RDH branch.

For each conflict, compare all three versions:

```bash
git -C "$WORKTREE" diff --base --ours --theirs -- path/to/file
git -C "$WORKTREE" diff "${TARGET_TAG}..HEAD" -- path/to/file
```

## 3. Verify the source boundary

```bash
cd "$WORKTREE"
python3 tests/test_herbin_branding.py
git diff --check
git diff --stat "$TARGET_TAG..HEAD"
git diff --name-status "$TARGET_TAG..HEAD"
```

Review the patch for:

- no `herbin-keymap`, shortcut-remap, or old handoff implementation;
- no `macos-input-trace`, `macos-focus-trace`, delayed click thread, or per-click
  info logging;
- `skip`, `forward_only`, and `activate` remain the only targeting decisions, and
  `mode = "passthrough"` bypasses candidate collection and execution;
- targeting preprocessing still occurs before `en.mouse_down(MouseButton::Left)`,
  while the original mouse-down remains unconditional;
- candidate collection remains bounded and does not blanket-filter non-zero layers
  or Dock-owned windows;
- built-in rules still distinguish Dock UI, interactive transient UI, and the
  passive Notification Center overlay;
- only `activate` uses public Accessibility/AppKit ordering APIs, validates the AX
  window PID against the selected CoreGraphics owner PID, and avoids re-raising an
  already focused AX window;
- `_AXUIElementGetWindow`, `CGSOrderWindow`, and SkyLight remain absent;
- file watching remains absent; configuration changes occur only through explicit
  `status`, `validate`, and `reload` management operations;
- bundle ID and service namespace stay separate from official RustDesk;
- the custom client still skips the official update checker.
- the memory watchdog remains launchd-gated, scheduled once daily at 06:00, and
  scoped to the user `--server` process;
- the macOS user LaunchAgent keeps `--server` alive after both clean and
  unsuccessful exits; stopping the service still unloads the LaunchAgent;

Commit and push the candidate branch only after these checks pass:

```bash
git add --all
git commit -m "Upgrade RustDesk-Herbin to ${TARGET_TAG}"
git push -u fork "$CANDIDATE_BRANCH"
```

## 4. Build the candidate in CI

Do not install local build dependencies. Dispatch the candidate workflow:

```bash
gh workflow run codex-macos-herbin.yml \
  --repo Herbin-s/rustdesk \
  --ref master \
  -f source_ref="$CANDIDATE_BRANCH" \
  -f rdh_revision=1
```

Use `gh run list` and `gh run view --json status,conclusion,url` to inspect the
result. The artifact contains the DMG, SHA-256 file, and build metadata. While RDH
uses ad-hoc signing, `codesign --verify --deep --strict` must pass but Gatekeeper
acceptance is not expected.

## 5. Protect the active remote session

Never replace RDH while it is the only working route into the Mac.

Before installation:

1. Start official RustDesk and prove that a separate controller can reconnect to it.
2. Keep the previous known-good RDH DMG and checksum available outside `/Applications`.
3. Verify the candidate checksum and signature.
4. Record the current RDH ID and confirm that its config directory remains
   `$HOME/Library/Preferences/com.herbin.RustDesk-Herbin`.

Only then install the candidate. Do not delete the previous release artifact.

## 6. Runtime acceptance

Test from an official Windows controller and from iPhone/iPad mouse mode:

- click between at least three applications and confirm menu-bar app name and window
  order change;
- click between two windows of the same application and confirm the clicked window
  becomes frontmost without changing the menu-bar application;
- open a Finder menu and select an item; the menu must remain interactive without
  pre-activating the window below it;
- open a Finder toolbar popover and interact with it without dismissing it through
  unwanted pre-activation;
- open a Dock contextual menu and select an item without activating an application
  underneath the menu before the click;
- expose the passive Notification Center overlay and confirm its built-in `skip`
  rule reaches and activates the intended regular window below it;
- use `--window-targeting validate`, switch to `mode = "passthrough"`, reload, and
  repeat the click matrix as the upstream mouse-behavior baseline;
- restore `mode = "rules"`, validate and reload again, then repeat the same matrix
  and compare it with the passthrough A/B baseline;
- click Dock icons, other popovers, and desktop without unwanted activation;
- drag, select text, right-click, scroll, and double-click;
- test multiple displays and a fullscreen or separate-Space window;
- repeat with AltTab and DockDoor running;
- disconnect and reconnect once;
- confirm Screen Recording, Accessibility, and Input Monitoring remain effective.

## 7. Promote or roll back

After acceptance, fast-forward the fork's `master` branch to the candidate and tag
the deployed artifact as `rdh-<upstream>.1`:

```bash
git push fork "${CANDIDATE_BRANCH}:master"
git tag -a "rdh-${TARGET_TAG}.1" -m "RustDesk-Herbin ${TARGET_TAG}.1"
git push fork "rdh-${TARGET_TAG}.1"
```

The first push intentionally fails if promotion would not be a fast-forward. Keep
the candidate DMG and its checksum in a GitHub Release rather than as duplicate
applications in `/Applications`.

If the candidate fails, reconnect through official RustDesk, reinstall the previous
known-good RDH DMG, verify its signature, and launch its isolated server/service.
Do not remove the official rescue route until the old RDH connection works again.
