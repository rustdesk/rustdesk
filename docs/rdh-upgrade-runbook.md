# RustDesk-Herbin upgrade runbook

This runbook upgrades RDH to an official stable RustDesk tag while preserving the
small RDH patch set. Builds happen in GitHub Actions. The procedure never replaces
the running application until a candidate artifact and a recovery connection both
exist.

## Patch contract

RDH keeps only these deviations from upstream:

1. `RustDesk-Herbin` branding, bundle ID, URL scheme, config, and launchd isolation.
2. A macOS controlled-side workaround that activates the regular application under
   the cursor immediately before a remote left-button-down event.
3. A dedicated ad-hoc-signed macOS CI build until Developer ID signing is available.

Custom keyboard mapping and high-volume mouse diagnostics must remain absent.

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
the three-item patch contract. Never recover an entire conflicted input or keyboard
file from the old RDH branch.

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
- activation still occurs before `en.mouse_down(MouseButton::Left)`;
- Dock and non-regular overlay applications remain excluded;
- bundle ID and service namespace stay separate from official RustDesk;
- the custom client still skips the official update checker.

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
  --ref "$CANDIDATE_BRANCH" \
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
- click Dock icons, menus, popovers, and desktop without unwanted activation;
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
