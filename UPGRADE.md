# Upgrading Tabby to a newer upstream RustDesk release

Tabby is a true GitHub fork of `rustdesk/rustdesk`. Upstream cuts stable
tags (e.g. `1.4.6`, `1.4.7`) on `master`. This document is the runbook for
pulling a new upstream stable tag into Tabby with minimal merge pain.

## Branching model

- **`main`** — Tabby's main. Contains all Tabby commits (build plan,
  scripts, custom UI, signing config) on top of an older upstream tag.
  It does **not** track `upstream/master`. Never merge `upstream/master`
  directly into `main`; always go through the upgrade branch (step 3
  below).
- **`upstream/master`** — read-only reference to upstream rustdesk
  development. Fetch it to find new tags, but don't merge it.
- **`tabby/<feature>`** — feature branches off `main`, merged back to
  `main` via PR.
- **`tabby/upgrade-<tag>`** — short-lived branches dedicated to absorbing
  a new upstream stable tag, also merged back to `main` via PR after the
  build is verified on device.

## Why merge surface stays small

The Phase 0 spike confirmed that all custom Tabby code lives under
`flutter/lib/custom/` and exactly **one** upstream Dart file is touched:
the feature-flag line in `flutter/lib/main.dart` (see plan §4.1). The
only other upstream-side touches are iOS Xcode-project files where
Tabby's team / bundle ID diverge from Carriez's. Everything else in the
tree is upstream-as-is. As a consequence, an upstream merge should
produce conflicts only in `main.dart` (and only if upstream changed that
file's `runApp` block in the same area), plus possibly in
`flutter/ios/Runner.xcodeproj/project.pbxproj` if signing settings moved.

If you ever find yourself resolving conflicts in many files, **stop** —
something in our process drifted from the sibling-directory pattern and
needs to be reverted before continuing.

## Runbook

### 1. Sync remotes

```bash
git fetch upstream --tags
git fetch origin
```

### 2. Identify the target stable tag

```bash
git tag -l --sort=-v:refname | head -10
# pick the latest non-RC, non-beta tag, e.g. 1.4.7
```

Avoid `nightly` and `fdroid-version` — those are not stable releases.

### 3. Create the upgrade branch off the target tag

`main` is Tabby's main — do not merge upstream into it directly. Cut a
fresh branch from the upstream tag:

```bash
git checkout -b tabby/upgrade-<new-tag> refs/tags/<new-tag>
```

### 4. Replay Tabby's commits onto the new tag

The set of commits to replay = everything on `main` that's not in the
old base tag we were last on (i.e. our Tabby-specific work). Find that
old tag in the previous upgrade PR's title, or look at the merge-base:

```bash
# Replay range
git log --reverse --oneline <old-tag>..main

# Cherry-pick the range onto the upgrade branch
git cherry-pick <old-tag>..main
```

For `flutter/lib/main.dart`, expect a conflict if upstream touched the
surrounding `runApp` block. Resolve by re-applying the conditional
`runApp(_rootWidget())` swap and the `tabby` import. Confirm the
resolved file matches the pattern in plan §4.1.

For `flutter/ios/Runner.xcodeproj/project.pbxproj`, expect conflicts on
`DEVELOPMENT_TEAM` and `PRODUCT_BUNDLE_IDENTIFIER` if upstream renamed
or restructured signing settings — always resolve to Tabby's values
(`GUW6BN8X57` and `dev.ronenmars.tabby`).

### 5. Re-pin Flutter / Rust if upstream changed them

Check `.github/workflows/flutter-build.yml` for `FLUTTER_VERSION` and
`rust-toolchain.toml` for the channel. If either changed:

```bash
# Flutter
fvm install <new-version>
fvm use <new-version>
# also update .fvmrc

# Rust — edit rust-toolchain.toml channel
```

### 6. Re-generate flutter_rust_bridge bindings

The Rust FFI surface may have shifted. Regenerate:

```bash
( cd flutter && fvm flutter pub get )
PATH="$(pwd)/.fvm/flutter_sdk/bin:$PATH" \
  flutter_rust_bridge_codegen \
    --rust-input src/flutter_ffi.rs \
    --dart-output flutter/lib/generated_bridge.dart \
    --c-output flutter/ios/Runner/bridge_generated.h
```

If signatures of `session_input_key`, `session_input_string`, or
`session_send_mouse` changed, **stop and update `custom/input/input_bridge.dart`
to match before continuing**. Those are the load-bearing FFI calls.

### 7. Verify the build

```bash
./scripts/build-ios.sh
```

Then open `flutter/ios/Runner.xcworkspace` and run on a registered device.
Smoke-test against the relay (see `SPIKE_NOTES.md` for credentials in
`.env.local`).

### 8. Push and PR

```bash
git push -u origin tabby/upgrade-<new-tag>
gh pr create --base main --head tabby/upgrade-<new-tag> \
  --title "tabby/upgrade: rustdesk <new-tag>" \
  --body "$(cat <<EOF
## Upgrade summary
- Bumped from <old-tag> to <new-tag>
- Flutter: <X> → <Y> (or unchanged)
- Rust: <X> → <Y> (or unchanged)
- flutter_rust_bridge: <X> → <Y> (or unchanged)
- FFI signature changes: <list — or "none">

## Verification
- [ ] scripts/build-ios.sh runs clean
- [ ] App installs on device
- [ ] Connects to relay, video + input verified
EOF
)"
```

## When to skip an upstream release

Tabby is not obligated to track every upstream release. Skip a release if:

- The upstream changelog covers only platforms we don't ship (Android,
  Linux, web, server) and no security fixes
- The release is a non-stable (RC / beta / nightly) tag
- FFI churn is large and we have no Phase 1+ build hardening to validate it

Track the security-relevant upstream commits in
`SPIKE_NOTES.md` ("Postponed upstream changes") so they aren't lost.
