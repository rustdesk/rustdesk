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

- [x] hbbs / hbbr reachable from device (`rustdesk.rbv1000.win`, single host for both)
- [x] vcpkg ports built for `arm64-ios` (libvpx, libyuv, opus, aom, ffmpeg, libjpeg-turbo) — built into project-local `vcpkg_installed/arm64-ios/`
- [x] Flutter SDK patched with `flutter_3.24.4_dropdown_menu_enableFilter.diff` (applied to fvm-managed `~/fvm/versions/3.24.5`)
- [x] App built and signed with Tabby team ID `GUW6BN8X57` + bundle ID `dev.ronenmars.tabby`
- [x] Deployed to device (UDID `00008150-00115DEA1A40401C`)
- [x] Connected to remote desktop end-to-end (video + input verified via Mac RustDesk → iPhone Tabby through `rustdesk.rbv1000.win`)

### Day 2 build invariants (must preserve into Phase 1 build script)

These are the actual steps that produce a working iOS device build from a clean clone.
The placeholders in `tabby-build-plan.md` were close but not exact — the real commands:

```bash
# 1. Clone with submodules (the manifest workspace breaks without libs/hbb_common)
git clone --recurse-submodules <fork-url> Tabby
# or after the fact:
git submodule update --init --recursive

# 2. Pin Flutter (committed via .fvmrc) and patch the Flutter SDK
fvm install 3.24.5
fvm use 3.24.5
( cd ~/fvm/versions/3.24.5 \
  && git apply <repo>/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff )

# 3. vcpkg in manifest mode (reads vcpkg.json) — installs to <repo>/vcpkg_installed/
~/vcpkg/vcpkg install --triplet=arm64-ios

# 4. Symlink manifest install dirs into vcpkg's global "installed/" so that
#    upstream crates (magnum-opus, hwcodec) which hardcode VCPKG_ROOT/installed/<triplet>/
#    can find headers/libs.
ln -s "$(pwd)/vcpkg_installed/arm64-ios" ~/vcpkg/installed/arm64-ios
ln -s "$(pwd)/vcpkg_installed/arm64-osx" ~/vcpkg/installed/arm64-osx

# 5. Generate flutter_rust_bridge bindings (Rust + Dart + iOS C header)
cargo install flutter_rust_bridge_codegen --version 1.80.1
PATH="$(pwd)/.fvm/flutter_sdk/bin:$PATH" \
  flutter_rust_bridge_codegen \
    --rust-input src/flutter_ffi.rs \
    --dart-output flutter/lib/generated_bridge.dart \
    --c-output flutter/ios/Runner/bridge_generated.h

# 6. Resolve Flutter deps
( cd flutter && fvm flutter pub get )

# 7. Build the Rust core for iOS device. `IPHONEOS_DEPLOYMENT_TARGET=13.0` is required:
#    cc-rs defaults to the Xcode SDK's max deployment (iOS 26.4 on Xcode 26), but rustc
#    targets iOS 10.0 by default for `aarch64-apple-ios`, and the linker fails on
#    `__chkstk_darwin` (introduced in iOS 13) due to the version mismatch.
IPHONEOS_DEPLOYMENT_TARGET=13.0 \
  VCPKG_ROOT="$HOME/vcpkg" \
  VCPKG_INSTALLED_ROOT="$(pwd)/vcpkg_installed" \
  cargo build --features flutter,hwcodec --release --target aarch64-apple-ios --lib
# → target/aarch64-apple-ios/release/liblibrustdesk.a (~140 MB)

# 8. Build the iOS app bundle (unsigned — Xcode signs)
( cd flutter && fvm flutter build ios --release --no-codesign )

# 9. Open Xcode, configure team + bundle ID, run
open flutter/ios/Runner.xcworkspace
# In Xcode:
#   - Runner target → Signing & Capabilities: team = GUW6BN8X57, bundle = dev.ronenmars.tabby
#   - Edit Scheme → Run → Build Configuration: Release  (Debug is unusably slow on device)
#   - Destination dropdown: pick the *physical* device (USB icon), not the simulator
#   - ▶
```

### Day 2 — observations / surprises

- vcpkg's manifest mode installs to `<repo>/vcpkg_installed/<triplet>/`, but `magnum-opus` and `hwcodec` (both rustdesk-org git deps) hardcode `~/vcpkg/installed/<triplet>/` (only respect `VCPKG_ROOT`, not `VCPKG_INSTALLED_ROOT`). Symlink is the cleanest workaround. Phase 1 build script must do this automatically.
- `IPHONEOS_DEPLOYMENT_TARGET=13.0` is mandatory at cargo build time. Without it the link fails on `__chkstk_darwin`.
- Xcode default destination after Flutter build is the iOS *Simulator* with the same model name as a connected device. Easy to miss. Linker error gives it away (`Building for 'iOS-simulator'` while the static lib was built for `'iOS'`).
- **Debug** Run configuration on physical device is unusably slow for a heavy Flutter app like RustDesk (white screen for 3+ minutes — Dart kernel interpreter). **Release** is required for any actual testing on device.
- Clone-without-submodules makes `flutter_rust_bridge_codegen` fail with a confusing cargo workspace error (`failed to read libs/hbb_common/Cargo.toml`). Must `git submodule update --init --recursive` before any cargo or codegen step.
- The Xcode build emits two warnings about `Info.plist` missing `NSBonjourServices` / `NSLocalNetworkUsageDescription`. These didn't actually block our smoke test (peer-to-peer over the relay does *not* need local-network privacy), but if/when we want LAN discovery to work, those keys must be added to `flutter/ios/Runner/Info.plist`. Deferred.
- Upstream `Info.plist` already has `Photo Library`, `Camera`, and `Wi-Fi Information` privacy keys — those came along for free.

### Day 2 — clean build time on this machine (M-series Mac, cold)

| Stage | Time |
|---|---|
| vcpkg arm64-ios manifest install | 6.9 min |
| `flutter pub get` | < 30 s |
| `flutter_rust_bridge_codegen` | ~30 s |
| `cargo build --release --target aarch64-apple-ios` (cold) | 2 min 19 s |
| `flutter build ios --release --no-codesign` | 7 min 47 s (mostly Xcode build = 6 min 47 s) |
| Xcode Release rebuild + sign + install on device | ~3 min |
| **Total clean → installed** | **~20 min** |

## Day 3 — Reconnaissance

> Verified against `1.4.6`. The placeholders in `tabby-build-plan.md` were
> close but had quirks — these are the actual signatures, paths, and names
> that the production code uses today.

### FFI: key input

- **Function name (Rust)**: `session_input_key`
- **Function name (Dart, generated)**: `sessionInputKey`
- **Rust file**: `src/flutter_ffi.rs:628`
- **Dart abstract / impl**: `flutter/lib/generated_bridge.dart:366` (abstract) and `:3059` (impl)
- **Existing Dart wrapper** (do **not** reinvent — but Tabby's `InputBridge` will still call `bind.*` directly per plan §3.1, to keep the FFI seam single): `InputModel.inputKey()` at `flutter/lib/models/input_model.dart:870` calls `bind.sessionInputKey(...)`.
- **Signature** — matches the build plan placeholder exactly:
  ```rust
  // src/flutter_ffi.rs:628
  pub fn session_input_key(
      session_id: SessionID,
      name: String,
      down: bool,
      press: bool,
      alt: bool,
      ctrl: bool,
      shift: bool,
      command: bool,
  )
  ```
  ```dart
  // flutter/lib/generated_bridge.dart
  Future<void> sessionInputKey({
    required UuidValue sessionId,
    required String name,
    required bool down,
    required bool press,
    required bool alt,
    required bool ctrl,
    required bool shift,
    required bool command,
    dynamic hint,
  });
  ```

### FFI: string input

- **Function name (Rust)**: `session_input_string`
- **Function name (Dart)**: `sessionInputString`
- **Rust file**: `src/flutter_ffi.rs:644`
- **Signature**:
  ```rust
  pub fn session_input_string(session_id: SessionID, value: String)
  ```
  ```dart
  Future<void> sessionInputString({
    required UuidValue sessionId,
    required String value,
    dynamic hint,
  });
  ```
- **Used for**: bulk string injection — Hebrew / IME composition / pasted text. No modifier handling at this layer.

### FFI: mouse / scroll

- **Function name (Rust)**: `session_send_mouse`
- **Function name (Dart)**: `sessionSendMouse`
- **Rust file**: `src/flutter_ffi.rs:1843`
- **Signature**:
  ```rust
  pub fn session_send_mouse(session_id: SessionID, msg: String)
  ```
  ```dart
  Future<void> sessionSendMouse({
    required UuidValue sessionId,
    required String msg,
    dynamic hint,
  });
  ```
- **Wire format**: `msg` is a JSON-encoded `HashMap<String, String>`. Recognised keys include `type` (`"wheel"`, `"trackpad"`, `"down"`, `"up"`, `"move"`), `x`, `y`, `buttons`, `relative_mouse_mode`, etc. Plan's `{type:"wheel", x, y}` payload is correct for our 2-finger scroll. Note: there's a *relative-mouse-mode marker* at `:1849` — Flutter-only filtering, irrelevant for our scroll flow but worth knowing exists.

### Canonical key names

- **Source**: `libs/hbb_common/protos/message.proto:209` — the `ControlKey` protobuf enum is the authoritative set of named (non-character) keys.
- **All current values** (extract — full list in the proto):
  ```
  Unknown(0)  Alt(1)  Backspace(2)  CapsLock(3)  Control(4)  Delete(5)
  DownArrow(6)  End(7)  Escape(8)  F1..F12(9-20)  Home(21)  LeftArrow(22)
  Meta(23)  Option(24, deprecated→Alt)  PageDown(25)  PageUp(26)  Return(27)
  RightArrow(28)  Shift(29)  Space(30)  Tab(31)  UpArrow(32)
  Numpad0..9(33-42)  Cancel(43)  Clear(44)  Menu(45, deprecated→Alt)
  Pause(46)  Kana/Hangul/Junja/Final/Hanja/Kanji/Convert(47-53)
  Select(54)  Print(55)  Execute(56)  Snapshot(57)  Insert(58)
  Help(59)  Sleep(60)  Separator(61)  Scroll(62)  NumLock(63)
  RWin(64)  Apps(65)  Multiply/Add/Subtract/Decimal/Divide(66-70)
  Equals(71)  NumpadEnter(72)  RShift/RControl/RAlt(73-75)
  VolumeMute/Up/Down(76-78)  Power(79)
  CtrlAltDel(100)  LockScreen(101)
  ```
- **Name string form passed through `session_input_key`**: not raw enum names. Two forms observed in upstream call-sites:
  1. **Lowercase descriptive** for our use case: e.g. `"escape"`, `"tab"`, `"control"`, `"alt"`, `"meta"`, `"shift"`, `"backspace"`, `"return"`, `"left"`, `"right"`, `"up"`, `"down"`. The matching is in `src/keyboard.rs` (`name_to_key`-style logic).
  2. **`VK_*` prefixed form** for some keys: `flutter/lib/mobile/pages/remote_page.dart:279` uses `inputKey('VK_BACK')` for backspace from the IME; `:978` uses `inputKey('VK_$name')` from gesture-help. These are Windows VK_ codes the Rust core also resolves.
- **Tabby strip mapping** — for our 8 power keys, the lowercase descriptive form (option 1) is the safe pick:
  - `escape`, `tab`, `control`, `alt`, `meta`, `backspace`, `left`, `down`, `up`, `right`.
- **TODO before Phase 3 implementation**: read `name_to_key` in `src/keyboard.rs` (or wherever the name string is decoded into `rdev::Key` / `ControlKey`) and confirm the exact accepted strings, especially for `meta` vs `command` vs `super`.

### Existing keyboard overlay widget(s)

- **`flutter/lib/mobile/widgets/gesture_help.dart`** — a *gesture tutorial* overlay (taps explained, two-finger tap, etc.), not a power-keys strip. Conditional render via `_showGestureHelp` flag in `remote_page.dart`. Tabby's strip will replace this slot or live alongside it.
- **`flutter/lib/mobile/widgets/floating_mouse.dart` + `floating_mouse_widgets.dart`** — a floating on-screen mouse cursor for touch input. Distinct from the keyboard strip; we leave this alone for v1 (it's how mouse interaction currently works on iOS).
- **`flutter/lib/common/widgets/toolbar.dart`** — *desktop* toolbar (window controls etc.). Not relevant on iOS.
- **There is no current "power-keys strip" widget** in upstream. Tabby's `PowerStrip` is the first.

### Remote page widget (mount point for `PowerStrip`)

- **File**: `flutter/lib/mobile/pages/remote_page.dart`
- **Layout**: builds a `Scaffold` (line 386) with:
  - `body` = the remote canvas (image stream + gesture detector)
  - `bottomNavigationBar` (line 416) = `Obx(() => Stack(...))` showing the bottom UI based on observable state
  - `floatingActionButton` (line ~389) = the show/hide toggle
- **Existing bottom widget logic** at `_bottomWidget()` (line 369):
  ```dart
  Widget _bottomWidget() => _showGestureHelp
      ? getGestureHelp()
      : (_showBar && gFFI.ffiModel.pi.displays.isNotEmpty
          ? getBottomAppBar()
          : Offstage());
  ```
- **Tabby integration plan**: The cleanest sibling-directory mount is to **not** edit `remote_page.dart` directly. Instead, the `app_root.dart` feature flag (per plan §4.1) routes the entire app to `custom/screens/remote_session_screen.dart` when `CUSTOM_UI=true`, which builds its own `Scaffold` and embeds upstream's `RemoteCanvas` (or equivalent) underneath the Tabby `PowerStrip` overlay. This keeps `flutter/lib/mobile/pages/remote_page.dart` untouched.
- **Caveat**: extracting the canvas/render layer from `remote_page.dart` may require importing helpers (e.g. `_ImagePaint`, `RemoteMenubarState` etc.). If those are private (`_`-prefixed) we either (a) live with re-implementing the wrapping, or (b) accept a single targeted upstream edit to expose them. Decide in Phase 2.

### Cross-check upstream `ios` branch

- **Tip commit**: `a92d2301d9eb55e203fe5a90351d277430be4987` (vs `1.4.6` = `1abc897c4`)
- **Diff scope** (`git diff --stat 1.4.6 upstream/ios -- flutter/lib/ flutter/ios/ src/flutter_ffi.rs libs/hbb_common/`):
  - 73 files changed, **+1,500 / −9,672** — i.e. the `ios` branch is a *deletion* fork. It strips desktop-only paths.
  - Notable removals from `flutter/lib/`:
    - `models/relative_mouse_model.dart` — entire file deleted (1,061 lines)
    - `models/relative_mouse_accumulator.dart` — entire file deleted
    - `models/model.dart` shrinks by ~863 lines
    - `models/input_model.dart` shrinks by ~557 lines
  - `src/flutter_ffi.rs` shrinks by ~425 lines (FFI surface narrowed)
  - `flutter/lib/web/bridge.dart` rewritten substantially
- **Tabby decision**: **stay on `1.4.6`, do NOT base on `upstream/ios`**.
  - The `ios` branch is too divergent (and continues to diverge) to be a stable base. Merging future stable tags forward becomes painful.
  - We may *cherry-pick* specific iOS-targeted simplifications later if they prove useful, but we won't rebase.
  - The `ios` branch is a useful *reference* for "what could be simplified for iOS-only" — keep it as a research tab open during Phase 2.

## Day 4 — Sibling directory  *(landed in Phase 1, verified on device)*

- [x] `flutter/lib/custom/` created with `app_root.dart` + `input/input_bridge.dart`
- [x] `app_root.dart` stub renders (MaterialApp → Scaffold → centered placeholder + paw icon + Esc POC FAB)
- [x] Feature flag in `main.dart` works both ways (`CUSTOM_UI=true` ⇒ `tabby.AppRoot`, `CUSTOM_UI=false` ⇒ upstream `App()`)
- [x] **Verified on real iPhone (Ronen Mars's iPhone 17 Pro, iOS 26.2):** Tabby scaffold renders as designed — AppBar, paw icon, mount-confirmation copy, Send Esc FAB.

**Upstream touch in `main.dart`** — kept minimal: 1 import, 1 const, 1 helper
function, 2 `runApp(App())` → `runApp(_rootWidget())` swaps. `flutter analyze`
on `lib/custom/` and `lib/main.dart` reports no errors from Tabby code (the
remaining `info`-level diagnostics are pre-existing upstream Flutter SDK
deprecations).

## Day 5 — Keyboard POC  *(mechanically green on device; remote-arrival verify in Phase 2)*

- [x] **`InputBridge.tapKey('escape')`** wraps `bind.sessionInputKey` with the verified Day 3 signature; compiles + links cleanly. Path: scaffold FAB → `InputBridge.poc().tapKey('escape')` → `bind.sessionInputKey(...)` → Rust core.
- [x] **Verified on real iPhone:** tap of "Send Esc" FAB invokes the FFI without crashing. App stays alive; snackbar surfaces ("FFI invoked. No active session — wire one up in Phase 2.") confirming the await on `tapKey()` resolves cleanly. The Rust core treats the placeholder zero-UUID as "no matching session" and returns silently — exactly the design.
- [ ] **End-to-end Esc keystroke arrives on a real remote** — *deferred to Phase 2.* Requires an active session, which requires a connection flow in the custom UI. That flow is Phase 2 scope. The mechanical seam and the FFI invocation are now proven on hardware; the session wiring is the next phase.

## Decision (Days 6–7)

- [x] **GREEN** — proceed to Phase 1
- [ ] **YELLOW** — proceed with reduced scope
- [ ] **RED** — abandon / revisit alternatives

**Reason:** Day 2 hard gate cleared on the first end-to-end attempt. Vanilla
RustDesk (`1.4.6`) builds cleanly from a clone, signs against the Tabby
Apple Developer team, installs on a registered iPhone, and connects through
the user's self-hosted `rustdesk.rbv1000.win` relay with both video and
input working. The build pipeline is non-trivial (the eight invariants above
are the price of admission) but every step is deterministic and scriptable;
nothing in the chain depends on undocumented behavior or upstream changes
we don't control. The sibling-directory pattern from §4 of the plan is
viable as written. Day 3 reconnaissance (FFI signature audit) is the
remaining Phase 0 work and unblocks Phases 1 and beyond.
