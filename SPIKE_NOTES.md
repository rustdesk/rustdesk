# Spike Notes — Tabby (RustDesk iOS Custom Fork)

> Working notebook for Phase 0 (§5 of `tabby-build-plan.md`). Source of truth
> for real-world FFI signatures, file paths, and build pipeline gotchas as
> uncovered during the spike. Update daily. Mark the go/no-go decision at the
> bottom on Day 7.

## Environment

- Date started: 2026-04-26
- Upstream: `rustdesk/rustdesk`
- Fork: `RonenMars/Tabby` (true GitHub fork)
- Branch: `tabby/phase-0-spike`
- RustDesk tag / commit: `1.4.6` / `1abc897c4`
- Flutter version: `3.24.5` (per `.github/workflows/flutter-build.yml` → `FLUTTER_VERSION`)
- Dart SDK constraint: `^3.1.0` (per `flutter/pubspec.yaml`)
- `flutter_rust_bridge`: `1.80.1` (per `flutter/pubspec.yaml` — DO NOT upgrade)
- Rust toolchain: `rustc 1.88.0` (no `rust-toolchain.toml` in repo; pin in Phase 1)
- Xcode: `26.4.1` (Build 17E202)
- CocoaPods: `1.16.2`
- vcpkg baseline: `120deac3062162151622ca4860575a33844ba10b` (per `vcpkg.json`)
- Flutter version manager: `fvm 4.0.5` (Homebrew)
- Apple Developer Team ID: `GUW6BN8X57`
- Tabby bundle ID (proposed): `dev.ronenmars.tabby`
- Upstream bundle ID (Carriez, to replace): `com.carriez.flutterHbb`
- Upstream Team ID (Carriez, to replace): `HZF9JMC8YN`

## Day 1 — Environment

- [x] Rust + iOS targets installed (`aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`)
- [x] Flutter 3.24.5 install via `fvm` (in progress / completed)
- [x] vcpkg cloned + bootstrapped at `~/vcpkg` (port builds deferred to Day 2)
- [x] Apple Developer account: enrolled, Team ID `GUW6BN8X57`
- [x] Fork created: `RonenMars/Tabby` from `rustdesk/rustdesk`
- [x] Default branch renamed from upstream's `master` to `main` (origin/master deleted)
- [x] Branch `tabby/phase-0-spike` checked out at tag `1.4.6`
- [x] Upstream remote added: `upstream → https://github.com/rustdesk/rustdesk.git`
- [x] hbbs / hbbr server: `rustdesk.rbv1000.win` (single host for both ID + relay). Public key + values held in 1Password (`RemoteAccess` vault → `RustDesk Server (PC - Ronen)`) and mirrored locally to `.env.local` (gitignored). Pull with `op item get 'RustDesk Server (PC - Ronen)' --vault RemoteAccess --format=json`.
- [x] iOS device UDID recorded: `00008150-00115DEA1A40401C` (primary dev iPhone)
- [ ] iOS device registered in Apple Developer portal
- [ ] Distribution / development provisioning profile generated for `dev.ronenmars.tabby`

### Reading log (Day 1)

- `flutter/build_ios.sh` — minimal: applies a Flutter SDK patch then `flutter build ipa --release`. The patch is `.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff` — applied to the Flutter SDK install itself, not to the project. **Important: this means our local Flutter 3.24.5 SDK will need this patch applied before the first build.**
- `build.py` — Python build harness; iOS-specific paths are limited (`grep -nE "ios|iOS|iPhone|xcode" build.py` returns minimal hits at lines 406 and 589). Most iOS work flows through `flutter/build_ios.sh` and Xcode directly.
- `README.md` — has **no dedicated iOS section**. iOS docs are essentially absent upstream. We'll need to write our own under `UPGRADE.md` / build runbook in Phase 1.
- `flutter/ios/Podfile` — declares `platform :ios, '13.0'` but post-install forces `IPHONEOS_DEPLOYMENT_TARGET = '11.0'`. Effective minimum: iOS 11. Worth aligning to a single value in Phase 1 (likely iOS 13 to match modern SwiftUI / native APIs).
- `flutter/ios/exportOptions.plist` — currently hardcoded to Carriez identifiers (`HZF9JMC8YN` / `com.carriez.flutterHbb`). To swap in Phase 1.
- `vcpkg.json` — pins iOS-relevant ports: `aom`, `libjpeg-turbo`, `opus`, `libvpx`, `libyuv`, plus `ffmpeg` for `(android | ios | (linux & arm32)) & static`.
- Upstream branches of interest:
  - `upstream/master` — main dev
  - `upstream/ios` — **iOS-specific work in progress upstream**. Must inspect on Day 3 to see if it diverges from `1.4.6` in ways that affect our FFI assumptions.

### Notes / blockers (Day 1)

- The `git clone --clone` flow with `gh repo fork` was not used; we forked headlessly (`--clone=false`) and `git clone` separately. Result: clean, two remotes (`origin` = our fork, `upstream` = rustdesk).
- The cwd `/Users/.../Tabby` was briefly removed during the wipe-and-refork dance; recreated and restored without issue. No data lost (backups were in `/tmp/tabby-backup`).
- vcpkg port builds for iOS triplet (`arm64-ios`) are deferred — they only need to be present by Day 2 build time. Triggering them now would tie up minutes for no gain.

## Day 2 — Vanilla iOS build

- [ ] hbbs / hbbr reachable from device (URL / TLS verified)
- [ ] vcpkg ports built for `arm64-ios` (libvpx, libyuv, opus, aom, ffmpeg, libjpeg-turbo)
- [ ] Flutter SDK patched with `flutter_3.24.4_dropdown_menu_enableFilter.diff`
- [ ] App built and signed with Tabby team ID + bundle ID
- [ ] Deployed to device (UDID: ____)
- [ ] Connected to remote desktop end-to-end (video + input)
- [ ] Build time clean → IPA: ____ minutes
- Notes / blockers: ____

## Day 3 — Reconnaissance

> The most important day. Replace every "____" below with the **actual** signature
> read from the cloned repo, not the placeholder snippets in `tabby-build-plan.md`.

### FFI: key input

- Function name: ____
- File path (Dart side): ____
- File path (Rust side): ____
- Signature:
  ```dart
  // paste here
  ```

### FFI: string input

- Function name: ____
- Signature: ____

### FFI: mouse / scroll

- Function name: ____
- Signature: ____

### Canonical key names

- Source enum / file: ____
- Names (full enumerated set):
  - ____

### Existing keyboard overlay widget

- File: ____
- Notes: ____

### Remote page widget (the mounting point for `PowerStrip`)

- File: ____
- Notes: ____

### Cross-check upstream `ios` branch

- Diff `upstream/ios` vs `1.4.6` for any FFI / Flutter changes:
- Findings: ____

## Day 4 — Sibling directory

- [ ] `flutter/lib/custom/` created (stub)
- [ ] `app_root.dart` stub renders
- [ ] Feature flag in `main.dart` works both ways (`CUSTOM_UI=true` / `false`)
- Notes: ____

## Day 5 — Keyboard POC

- [ ] Custom button fired Esc on a real remote machine
- Path: button → InputBridge → `bind.____` → remote
- Notes / surprises: ____

## Decision (Days 6–7)

- [ ] **GREEN** — proceed to Phase 1
- [ ] **YELLOW** — proceed with reduced scope: ____
- [ ] **RED** — abandon / revisit alternatives

Reason: ____
