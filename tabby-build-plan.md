# Tabby — Build Plan

> *Tabby gives your remote desktop extra paws.* 🐾
>
> A comprehensive, phased build plan for **Tabby**, a custom iOS client built on a fork of RustDesk. Tabby ships a redesigned UI, a minimal power-keys overlay (modifiers + arrows + Tab + macros) layered above the native iOS keyboard, 2-finger scroll on the remote view, and a curated macro system. This document is written to be consumed by Claude Code as the implementation guide.

---

## 0. Naming and Branding

**Project name:** **Tabby**
**Tagline:** *Tabby gives your remote desktop extra paws.* 🐾

The name Tabby leans into a playful cat metaphor that ships well alongside a power-user developer tool — fitting naturally beside terminal/dev-tool peers like Kitty, Catppuccin, and Ghostty. The tagline *"Tabby gives your remote desktop extra paws"* works as a deliberate double pun: **paws** as in extra digits (extra Tabs, extra arrows, extra modifiers — everything the iOS native keyboard alone can't reach) and **paws ≈ powers**, since the whole product is about giving you extra power-user capabilities — sticky modifiers, macros, layered shortcuts, gestures — that no stock remote-desktop client provides. Internally, the product vocabulary leans into the metaphor lightly, never heavily: each saved peer is a "machine," a held modifier "claws in," and the macro sheet is just "the macro tray." Branding-wise, Tabby is short, distinctive, instantly memorable, easy to say in any language Ronen works in (English, Hebrew, Russian), and has obvious icon potential (a stylized cat paw or whiskers mark). The bundle identifier should be `win.rbv1000.tabby` (matching the existing `rbv1000.win` infrastructure domain), the GitHub repo `tabby-ios`, and the TestFlight display name simply `Tabby`. **Throughout this document, "Tabby" refers to the iOS app being built; "RustDesk" refers exclusively to the upstream project being forked.**

---

---

## Table of Contents

0. [Naming and Branding](#0-naming-and-branding)
1. [Project Overview](#1-project-overview)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [Architecture](#3-architecture)
4. [Repository Layout (Sibling-Directory Pattern)](#4-repository-layout-sibling-directory-pattern)
5. [Phase 0 — Spike (1 week, go/no-go)](#5-phase-0--spike-1-week-gono-go)
6. [Phase 1 — Build Hardening (1–2 weeks)](#6-phase-1--build-hardening-12-weeks)
7. [Phase 2 — UI Shell (2–4 weeks)](#7-phase-2--ui-shell-24-weeks)
8. [Phase 3 — Power-Keys Strip + Input Bridge + Scroll (1–1.5 weeks)](#8-phase-3--power-keys-strip--input-bridge--scroll-115-weeks)
9. [Phase 3b — Macro System (0.5–1 week)](#9-phase-3b--macro-system-051-week)
10. [Phase 4 — Polish (0.5–1 week)](#10-phase-4--polish-051-week)
11. [Phase 5 — Distribution (1–2 weeks)](#11-phase-5--distribution-12-weeks)
12. [Reference: Data Models](#12-reference-data-models)
13. [Reference: Default Macro Library](#13-reference-default-macro-library)
14. [Risks and Mitigations](#14-risks-and-mitigations)
15. [Acceptance Criteria](#15-acceptance-criteria)
16. [Appendix A — Spike Notes Template](#16-appendix-a--spike-notes-template)
17. [Appendix B — Glossary](#17-appendix-b--glossary)

---

## 1. Project Overview

### 1.1 What we're building

A forked iOS RustDesk client with:

- A fully custom Flutter UI replacing the stock RustDesk UI
- A minimal power-keys overlay (8 keys: Esc, ⌃, ⌥, ⌘, Fn, Tab, ⌫, arrows) layered above the **native iOS keyboard**, which handles letters/numbers/space/return/shift/IME
- Native iOS keyboard providing all language input (incl. Hebrew RTL) via string injection
- 2-finger scroll gesture on the remote view
- A macro system (defaults + user-defined custom macros) accessed via a bottom sheet
- Self-hosted RustDesk server (`hbbs`/`hbbr`) as the relay

### 1.2 Why this approach

| Decision | Reason |
|---|---|
| Fork RustDesk vs build from scratch | Reuse battle-tested Rust core (protocol, codec, FFI) |
| Keep upstream Rust untouched | Minimize merge pain, focus changes on Flutter layer |
| Sibling-directory pattern | Reduce merge conflicts to a single line in `main.dart` |
| Native iOS keyboard for letters | Free RTL/IME/dictation/autocorrect; less code to maintain |
| Custom strip for power keys only | Address the only thing iOS doesn't natively do |
| TestFlight / Enterprise distribution | Public App Store rejects most remote-desktop apps |

### 1.3 Out of scope (v1)

- Android port (stays vanilla)
- Desktop clients (stay vanilla)
- Server modifications (use stock `hbbs`/`hbbr`)
- Voice / file transfer redesign (use stock UI for these)

---

## 2. Goals and Non-Goals

### 2.1 Goals

- **G1.** Connect to a self-hosted RustDesk server and control a remote desktop end-to-end
- **G2.** Custom-branded UI, fully replacing the stock connection/session screens
- **G3.** Power-keys strip with 8 keys, persistent above the iOS keyboard
- **G4.** 2-finger pan on the remote view → mouse wheel scroll on remote machine
- **G5.** Macro bottom sheet with curated defaults + user-defined custom macros
- **G6.** Hebrew input via string injection (no custom IME logic)
- **G7.** Distributable via TestFlight (and optionally Apple Enterprise)
- **G8.** Fork stays mergeable with upstream RustDesk security patches

### 2.2 Non-goals

- **NG1.** Public App Store submission
- **NG2.** Replacing or modifying the Rust core
- **NG3.** Building a custom on-screen letter keyboard
- **NG4.** Cross-platform (iOS only for v1)

---

## 3. Architecture

### 3.1 Layer diagram

```
┌────────────────────────────────────────────────────┐
│        Custom Flutter UI (lib/custom/)             │  ← All your code
│   Screens · Theme · Strip · Macros · Settings      │
├────────────────────────────────────────────────────┤
│        InputBridge (lib/custom/ffi/)               │  ← Single FFI seam
│   Wraps bind.* calls; isolates upstream churn      │
├────────────────────────────────────────────────────┤
│        flutter_rust_bridge generated bindings       │  ← Don't touch
├────────────────────────────────────────────────────┤
│        RustDesk Rust core (src/)                    │  ← Untouched
│   Protocol · Codec · Input handling · Networking    │
├────────────────────────────────────────────────────┤
│        Self-hosted hbbs/hbbr server                 │  ← Stock
└────────────────────────────────────────────────────┘
```

### 3.2 Input pipeline

```
User taps power-keys strip
  → KeyboardController.handleKey()
  → InputBridge.tapKey() / fireMacro()
  → bind.sessionInputKey()
  → Rust core
  → Network → remote machine

User types on iOS native keyboard
  → hidden TextField onChange (with sentinel)
  → InputBridge.typeString()
  → bind.sessionInputString()
  → Rust core
  → Network → remote machine

User does 2-finger pan on remote view
  → GestureDetector.onScaleUpdate (pointerCount==2, scale≈1.0)
  → ScrollAccumulator (throttled)
  → InputBridge.scroll(dx, dy)
  → bind.sessionSendMouse(type: "wheel", ...)
  → Rust core
  → Network → remote machine
```

### 3.3 Final keyboard layout

```
┌─────────────────────────────────────────┐
│         Remote Desktop View             │
│                                         │
├─────────────────────────────────────────┤
│  [Esc] [⌃] [⌥] [⌘] [Fn]       [Tab]     │  Power-keys row 1
│                                         │
│  [Macros]                [←][↓][↑][→]   │  Power-keys row 2
├─────────────────────────────────────────┤
│      iOS NATIVE KEYBOARD                │  System-rendered
│   (letters, numbers, space, return,     │
│    shift, emoji, dictation, languages)  │
└─────────────────────────────────────────┘
```

8 power keys total. Modifiers + utility on the left, navigation cluster on the right. Strip height ~100pt.

---

## 4. Repository Layout (Sibling-Directory Pattern)

**Critical principle:** All custom code lives inside `flutter/lib/custom/`. Upstream Flutter code is touched in exactly one place: a single feature flag in `main.dart`. This keeps the merge surface tiny.

```
flutter/lib/
├── (vanilla RustDesk Flutter code — DO NOT MODIFY)
├── main.dart                         ← edit ONE line: feature flag
└── custom/                           ← all custom code lives here
    ├── app_root.dart                 ← top-level router/theme switcher
    ├── theme/
    │   ├── tokens.dart               ← color, spacing, radius, typography tokens
    │   ├── app_theme.dart            ← ThemeData light/dark
    │   └── components.dart           ← shared styled widgets
    ├── screens/
    │   ├── connect_screen.dart       ← entry: enter peer ID, password, connect
    │   ├── session_list_screen.dart  ← saved peers, recents, favorites
    │   ├── remote_session_screen.dart ← active session: remote view + strip
    │   └── settings_screen.dart      ← server config, handedness, scroll sens.
    ├── strip/
    │   ├── models/
    │   │   ├── key_def.dart          ← KeyDef class
    │   │   ├── strip_layout.dart     ← StripLayout, StripRow
    │   │   └── modifier_state.dart   ← ModifierMode, ModifierController
    │   ├── controller/
    │   │   └── keyboard_controller.dart ← orchestrates keys + modifiers
    │   ├── widgets/
    │   │   ├── power_strip.dart      ← root strip widget
    │   │   ├── key_cell.dart         ← individual key visual
    │   │   ├── arrow_cluster.dart    ← right-side arrow group
    │   │   └── modifier_indicator.dart ← visual state for held modifiers
    │   └── layouts/
    │       └── default_strip.dart    ← canonical 8-key layout
    ├── input/
    │   ├── input_bridge.dart         ← THE single FFI seam
    │   ├── text_field_bridge.dart    ← hidden TextField + sentinel logic
    │   ├── scroll_gesture.dart       ← 2-finger scroll detector
    │   └── scroll_accumulator.dart   ← throttle + emit
    ├── macros/
    │   ├── models/
    │   │   ├── macro.dart            ← Macro class
    │   │   ├── macro_step.dart       ← sealed: KeyChord, String, Delay, KeyTap
    │   │   └── macro_category.dart
    │   ├── engine/
    │   │   └── macro_engine.dart     ← execution, timing, haptics
    │   ├── widgets/
    │   │   ├── macro_sheet.dart      ← bottom sheet picker
    │   │   ├── macro_row.dart        ← single macro line item
    │   │   └── macro_builder.dart    ← create/edit custom macro (v1.1)
    │   └── data/
    │       ├── default_macros.dart   ← curated library
    │       └── macro_storage.dart    ← persist via SharedPreferences
    ├── settings/
    │   └── settings_store.dart       ← Zustand-style local state
    └── util/
        ├── haptics.dart
        └── logging.dart
```

### 4.1 The single upstream touch point

```dart
// flutter/lib/main.dart (UPSTREAM FILE — minimal edit)

// ADD: import at top
import 'package:flutter_hbb/custom/app_root.dart' as custom;

// ADD: feature flag
const _useCustomUI = bool.fromEnvironment('CUSTOM_UI', defaultValue: true);

// MODIFY: in runApp(), branch on flag
void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initEnv(/* existing args */);
  if (_useCustomUI) {
    runApp(const custom.AppRoot());
  } else {
    runApp(const App()); // original
  }
}
```

That's it. Build with `--dart-define=CUSTOM_UI=true` (default) or `false` to fall back to vanilla for diffing/debugging.

---

## 5. Phase 0 — Spike (1 week, go/no-go)

**Objective:** Prove the project is feasible before committing 8+ weeks. End the week with a vanilla RustDesk iOS build running on a real device, plus one custom button that fires a real key event end-to-end.

### 5.1 Deliverables

- [ ] Reproducible local build of vanilla RustDesk for iOS device
- [ ] Self-hosted `hbbs`/`hbbr` reachable from the iOS device
- [ ] `SPIKE_NOTES.md` documenting actual FFI signatures, file paths, gotchas
- [ ] One custom widget in a sibling directory that fires `Esc` to a real remote machine
- [ ] Go/no-go decision documented

### 5.2 Day-by-day

#### Day 1 — Environment

- Install Rust + iOS targets:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  ```
- Install Flutter at the version pinned in RustDesk's `flutter/pubspec.yaml`
- Install vcpkg, bootstrap dependencies for iOS (libvpx, libyuv, opus, aom)
- Clone `https://github.com/rustdesk/rustdesk` and check out the latest **stable tag** (not `master`)
- Read end-to-end: `flutter/build_ios.sh`, `build.py`, the iOS section of the README
- Provision an Apple Developer account if not already done
- Set up an iOS device for development (UDID registered, dev provisioning profile)

#### Day 2 — Vanilla iOS build

- Spin up self-hosted `hbbs`/`hbbr` (Docker compose recommended)
- Build the vanilla RustDesk iOS app following the official build instructions
- Sign and deploy to the physical device
- Run a full session: connect from iOS to a desktop, control it, verify input + video
- **Hard gate: if the app is not running on a device by EOD, escalate or scope-cut**

#### Day 3 — Reconnaissance (the most important day)

Run these greps and record the **actual** function signatures in `SPIKE_NOTES.md`. Do not trust assumptions.

```bash
# Find the actual FFI key input function name and signature
grep -rn "sessionInputKey\|sessionSendKey\|inputKey" flutter/lib/ rust/

# Find the canonical key name enumeration (in Rust)
grep -rn "enum.*ControlKey\|rdev::Key\|enum Key" rust/src/

# Find string injection / IME path
grep -rn "sessionInputString\|inputString\|composition\|ime" flutter/lib/ rust/

# Find mouse / scroll input
grep -rn "sessionSendMouse\|sendMouse\|MouseEvent" flutter/lib/ rust/

# Find existing keyboard overlay widget
grep -rn "GestureHelp\|KeyHelpTools\|keyboard_overlay" flutter/lib/

# Find the remote page (you'll inject your overlay here)
find flutter/lib -name "remote_page*.dart"
```

Document in `SPIKE_NOTES.md`:
- Exact function names and parameter signatures
- Valid key name strings (the enumerated set the Rust core accepts)
- Path to the remote page widget where the strip will mount
- Any version-specific oddities

#### Day 4 — Sibling-directory scaffold

- Create `flutter/lib/custom/` with a stub `app_root.dart`
- Add the feature flag to `main.dart` (the single upstream touch)
- Build with `--dart-define=CUSTOM_UI=true` and confirm the stub renders
- Build with `--dart-define=CUSTOM_UI=false` and confirm vanilla still works

#### Day 5 — Keyboard POC (moment of truth)

- Add a single floating button in your custom UI
- On tap, invoke the actual FFI function (signatures from Day 3)
- Verify on the remote machine that an `Esc` keystroke arrived
- This proves the full pipeline: custom UI → FFI → Rust → remote

#### Days 6–7 — Buffer + decision

- Finalize `SPIKE_NOTES.md`
- Write the go/no-go memo (see template below)
- Capture screenshots / a short video of the working POC

### 5.3 Go/no-go criteria

**Green-light only if all of these are true:**

- [ ] Vanilla iOS build completes in under 30 minutes from clean
- [ ] Custom widget fired a real FFI key event end-to-end on a real remote
- [ ] Sibling-directory builds cleanly with one upstream touch
- [ ] FFI signatures and key name enumeration are documented
- [ ] Build pipeline ergonomics are tolerable (not flaky)

**Otherwise:** drop scope to a keyboard-only fork (skip Phase 2, ship vanilla UI + custom strip), or revisit alternatives entirely.

---

## 6. Phase 1 — Build Hardening (1–2 weeks)

**Objective:** Make the build pipeline reproducible, scripted, and CI-friendly. Set up signing for TestFlight.

### 6.1 Tasks

- Pin Rust toolchain version in `rust-toolchain.toml`
- Pin Flutter version in tooling (asdf / fvm)
- Pin `flutter_rust_bridge` version (do not upgrade)
- Document vcpkg dependency versions
- Create `scripts/build-ios.sh` that takes clean → IPA in one command
- Set up GitHub Actions workflow for iOS builds (optional but recommended)
- Provision Apple Developer signing:
  - App ID with custom bundle identifier (e.g., `com.yourorg.rustdesk-custom`)
  - Distribution provisioning profile
  - TestFlight-ready certificate
- Tag the fork's first stable build commit
- Add `UPGRADE.md` documenting the upstream merge process

### 6.2 Exit criteria

- [ ] `./scripts/build-ios.sh` runs from clean to signed IPA
- [ ] First build uploaded to TestFlight successfully (even if vanilla)
- [ ] Documented procedure for pulling upstream RustDesk changes

---

## 7. Phase 2 — UI Shell (2–4 weeks)

**Objective:** Replace the stock RustDesk UI with a custom-branded shell. Connection, session list, settings, and the remote session frame all routed through `custom/`.

### 7.1 Screens to build

| Screen | Purpose | Notes |
|---|---|---|
| Connect screen | Peer ID input, password, recent peers list | Pre-fill from saved server config |
| Session list | Saved peers, recents, favorites, search | Promote favorites to top |
| Remote session | The active control session: remote view + power strip | The biggest screen |
| Settings | Server config, handedness, scroll sensitivity, theme | Local-only persistence |

### 7.2 Theming

Define design tokens in `theme/tokens.dart`:

```dart
class AppTokens {
  // Colors (light theme)
  static const colorPrimary = Color(0xFF2563EB);
  static const colorBgBase = Color(0xFF0F172A);
  static const colorBgSurface = Color(0xFF1E293B);
  static const colorTextHigh = Color(0xFFE2E8F0);
  static const colorTextMid = Color(0xFF94A3B8);

  // Spacing scale (4pt grid)
  static const spaceXs = 4.0;
  static const spaceSm = 8.0;
  static const spaceMd = 12.0;
  static const spaceLg = 16.0;
  static const spaceXl = 24.0;

  // Radii
  static const radiusKey = 8.0;
  static const radiusCard = 12.0;
  static const radiusSheet = 20.0;

  // Typography
  static const fontKey = TextStyle(fontSize: 16, fontWeight: FontWeight.w600);
  static const fontKeySmall = TextStyle(fontSize: 12, fontWeight: FontWeight.w500);
  static const fontBody = TextStyle(fontSize: 15, fontWeight: FontWeight.w400);
  static const fontTitle = TextStyle(fontSize: 22, fontWeight: FontWeight.w700);
}
```

### 7.3 Routing

Use `go_router` (or whatever upstream uses, to minimize transitive dep changes). All routes branch from `custom/app_root.dart`.

### 7.4 Exit criteria

- [ ] All four screens render and navigate
- [ ] Peer ID + password + custom server connects to a real remote
- [ ] Saved peers persist across app restarts
- [ ] Theme tokens used consistently (no hardcoded colors in screens)

---

## 8. Phase 3 — Power-Keys Strip + Input Bridge + Scroll (1–1.5 weeks)

**Objective:** The headline feature. Build the 8-key strip above the iOS keyboard, the hidden-TextField bridge for native input, and the 2-finger scroll gesture.

### 8.1 The InputBridge (single FFI seam)

```dart
// custom/input/input_bridge.dart

class InputBridge {
  final SessionID sessionId;
  InputBridge(this.sessionId);

  // ⚠️ Verify exact bind.* signatures from SPIKE_NOTES.md before implementing

  /// Press a single key (down + brief hold + up)
  Future<void> tapKey(String name) async {
    await _keyDown(name);
    await Future.delayed(const Duration(milliseconds: 8));
    await _keyUp(name);
  }

  /// Hold a key down (modifier behavior)
  Future<void> _keyDown(String name) => bind.sessionInputKey(
    sessionId: sessionId,
    name: name,
    down: true,
    press: false,
    alt: false, ctrl: false, shift: false, command: false,
  );

  Future<void> _keyUp(String name) => bind.sessionInputKey(
    sessionId: sessionId,
    name: name,
    down: false,
    press: false,
    alt: false, ctrl: false, shift: false, command: false,
  );

  /// String injection (handles Hebrew, emoji, accents, IME composition)
  Future<void> typeString(String s) => bind.sessionInputString(
    sessionId: sessionId,
    value: s,
  );

  /// Tap a key while modifiers are held
  Future<void> tapKeyWithModifiers(String key, Set<String> modifiers) async {
    for (final m in modifiers) {
      await _keyDown(m);
      await Future.delayed(const Duration(milliseconds: 12));
    }
    await tapKey(key);
    for (final m in modifiers) {
      await _keyUp(m);
      await Future.delayed(const Duration(milliseconds: 8));
    }
  }

  /// Mouse wheel scroll
  Future<void> scroll(int dx, int dy) => bind.sessionSendMouse(
    sessionId: sessionId,
    msg: jsonEncode({
      'type': 'wheel',
      'x': dx,
      'y': dy,
    }),
  );
}
```

**All FFI churn is contained here.** If upstream renames `sessionInputKey`, you fix one file.

### 8.2 The hidden TextField bridge (sentinel-based)

```dart
// custom/input/text_field_bridge.dart

const _sentinel = '\u200B';

class TextFieldBridge extends StatefulWidget {
  final InputBridge inputBridge;
  final ModifierController modifierController;
  const TextFieldBridge({
    required this.inputBridge,
    required this.modifierController,
  });

  @override
  State<TextFieldBridge> createState() => _TextFieldBridgeState();
}

class _TextFieldBridgeState extends State<TextFieldBridge> {
  final _controller = TextEditingController(text: _sentinel);
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _controller.selection = const TextSelection.collapsed(offset: 1);
    _focus.requestFocus();
    _controller.addListener(_onChange);
  }

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _onChange() {
    final text = _controller.text;

    // Initial / re-set state
    if (text == _sentinel) return;

    // User backspaced past the sentinel → empty field
    if (text.isEmpty) {
      widget.inputBridge.tapKey('backspace');
      _resetSentinel();
      return;
    }

    // Strip sentinel; remainder is what was typed
    final typed = text.replaceFirst(_sentinel, '');
    if (typed.isEmpty) return;

    final mods = widget.modifierController.heldModifiers;
    if (mods.isNotEmpty && typed.length == 1) {
      // Modifier + single char → key event with modifiers
      widget.inputBridge.tapKeyWithModifiers(typed.toLowerCase(), mods);
      widget.modifierController.releaseOneShot();
    } else {
      // Plain text → string injection (Hebrew, emoji, etc.)
      widget.inputBridge.typeString(typed);
    }

    _resetSentinel();
  }

  void _resetSentinel() {
    _controller.value = const TextEditingValue(
      text: _sentinel,
      selection: TextSelection.collapsed(offset: 1),
    );
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 1,
      height: 1,
      child: Opacity(
        opacity: 0,
        child: TextField(
          controller: _controller,
          focusNode: _focus,
          autofocus: true,
          enableInteractiveSelection: false,
          autocorrect: false,
          enableSuggestions: false,
          textInputAction: TextInputAction.send,
          onSubmitted: (_) {
            widget.inputBridge.tapKey('return');
            _focus.requestFocus(); // re-grab focus after submit
          },
          decoration: const InputDecoration(border: InputBorder.none),
        ),
      ),
    );
  }
}
```

### 8.3 The strip widget

```dart
// custom/strip/widgets/power_strip.dart

class PowerStrip extends StatelessWidget {
  final InputBridge inputBridge;
  final ModifierController modifierController;
  final VoidCallback onMacrosTap;
  final bool leftHanded;

  const PowerStrip({
    required this.inputBridge,
    required this.modifierController,
    required this.onMacrosTap,
    this.leftHanded = false,
  });

  @override
  Widget build(BuildContext context) {
    final layout = leftHanded
        ? defaultStripLayout.mirrored()
        : defaultStripLayout;

    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceSm,
        vertical: AppTokens.spaceXs,
      ),
      decoration: BoxDecoration(
        color: AppTokens.colorBgSurface,
        boxShadow: const [
          BoxShadow(blurRadius: 8, color: Colors.black26, offset: Offset(0, -2)),
        ],
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: layout.rows.map((row) {
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                ...row.left.map((k) => _wrap(k)),
                const Spacer(),
                ...row.right.map((k) => _wrap(k)),
              ],
            ),
          );
        }).toList(),
      ),
    );
  }

  Widget _wrap(KeyDef k) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: 2),
    child: KeyCell(
      keyDef: k,
      modifierController: modifierController,
      onTap: () => _handle(k),
    ),
  );

  void _handle(KeyDef k) {
    HapticFeedback.lightImpact();
    switch (k.type) {
      case KeyType.modifier:
        modifierController.toggle(k.keyName, ModifierMode.oneShot);
        break;
      case KeyType.macroOpener:
        onMacrosTap();
        break;
      case KeyType.regular:
        inputBridge.tapKey(k.keyName);
        break;
      case KeyType.layer:
        // Fn layer not implemented in v1 strip (use macros instead)
        break;
    }
  }
}
```

### 8.4 The default layout

```dart
// custom/strip/layouts/default_strip.dart

const defaultStripLayout = StripLayout(
  rows: [
    // Row 1: utilities + modifiers (left)  |  Tab (right)
    StripRow(
      left: [
        KeyDef(label: 'Esc', keyName: 'escape', type: KeyType.regular),
        KeyDef(label: '⌃', keyName: 'control', type: KeyType.modifier),
        KeyDef(label: '⌥', keyName: 'alt', type: KeyType.modifier),
        KeyDef(label: '⌘', keyName: 'meta', type: KeyType.modifier),
        KeyDef(label: 'Fn', keyName: '', type: KeyType.layer),
      ],
      right: [
        KeyDef(label: 'Tab', keyName: 'tab', type: KeyType.regular, widthFactor: 1.2),
      ],
    ),
    // Row 2: macros (left)  |  arrow cluster (right)
    StripRow(
      left: [
        KeyDef(label: '⚡ Macros', keyName: '', type: KeyType.macroOpener, widthFactor: 1.6),
      ],
      right: [
        KeyDef(label: '←', keyName: 'left', type: KeyType.regular),
        KeyDef(label: '↓', keyName: 'down', type: KeyType.regular),
        KeyDef(label: '↑', keyName: 'up', type: KeyType.regular),
        KeyDef(label: '→', keyName: 'right', type: KeyType.regular),
      ],
    ),
  ],
);
```

### 8.5 The 2-finger scroll detector

```dart
// custom/input/scroll_gesture.dart

class TwoFingerScrollDetector extends StatefulWidget {
  final InputBridge inputBridge;
  final Widget child;
  final double sensitivity;
  final bool inverted;

  const TwoFingerScrollDetector({
    required this.inputBridge,
    required this.child,
    this.sensitivity = 1.0,
    this.inverted = false,
  });

  @override
  State<TwoFingerScrollDetector> createState() => _TwoFingerScrollDetectorState();
}

class _TwoFingerScrollDetectorState extends State<TwoFingerScrollDetector> {
  late final ScrollAccumulator _acc;

  @override
  void initState() {
    super.initState();
    _acc = ScrollAccumulator(widget.inputBridge);
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onScaleUpdate: (details) {
        if (details.pointerCount != 2) return;
        // Reject if user is pinching (zoom)
        if ((details.scale - 1.0).abs() > 0.05) return;

        final dx = details.focalPointDelta.dx * widget.sensitivity;
        final dy = details.focalPointDelta.dy * widget.sensitivity * (widget.inverted ? -1 : 1);
        _acc.add(dx, dy);
      },
      child: widget.child,
    );
  }
}

class ScrollAccumulator {
  final InputBridge bridge;
  double _x = 0, _y = 0;
  DateTime _last = DateTime.now();

  ScrollAccumulator(this.bridge);

  void add(double dx, double dy) {
    _x += dx;
    _y += dy;
    final now = DateTime.now();
    if (now.difference(_last).inMilliseconds > 16 &&
        (_x.abs() > 2 || _y.abs() > 2)) {
      bridge.scroll(_x.round(), _y.round());
      _x = 0; _y = 0;
      _last = now;
    }
  }
}
```

### 8.6 Modifier state machine

Three modes per modifier:

| Gesture | Mode | Behavior |
|---|---|---|
| Single tap | `oneShot` | Held until next key fires, then auto-released |
| Double tap | `sticky` | Held until tapped again |
| Long-press | `held` | Held while finger is down, released on lift |

```dart
// custom/strip/models/modifier_state.dart

enum ModifierMode { off, oneShot, sticky, held }

class ModifierController extends ChangeNotifier {
  final InputBridge bridge;
  final Map<String, ModifierMode> _state = {};

  ModifierController(this.bridge);

  Set<String> get heldModifiers => _state.entries
      .where((e) => e.value != ModifierMode.off)
      .map((e) => e.key)
      .toSet();

  bool hasHeldModifiers() => heldModifiers.isNotEmpty;

  void toggle(String name, ModifierMode requestedMode) {
    final current = _state[name] ?? ModifierMode.off;
    if (current == ModifierMode.off) {
      _state[name] = requestedMode;
      bridge._keyDown(name);
    } else {
      _state[name] = ModifierMode.off;
      bridge._keyUp(name);
    }
    notifyListeners();
  }

  /// Called after a oneShot modifier has been used; release auto-releasing ones
  void releaseOneShot() {
    final toRelease = _state.entries
        .where((e) => e.value == ModifierMode.oneShot)
        .map((e) => e.key)
        .toList();
    for (final k in toRelease) {
      _state[k] = ModifierMode.off;
      bridge._keyUp(k);
    }
    notifyListeners();
  }
}
```

### 8.7 Mounting in the remote session screen

```dart
// custom/screens/remote_session_screen.dart

@override
Widget build(BuildContext context) {
  return Scaffold(
    body: Stack(
      children: [
        // Layer 0: remote view + 2-finger scroll
        Positioned.fill(
          child: TwoFingerScrollDetector(
            inputBridge: _bridge,
            child: RemoteCanvas(sessionId: widget.sessionId),
          ),
        ),

        // Layer 1: hidden TextField (must exist in tree to capture keyboard)
        Positioned(
          left: 0, top: 0,
          child: TextFieldBridge(
            inputBridge: _bridge,
            modifierController: _modCtl,
          ),
        ),

        // Layer 2: power strip floats above iOS keyboard
        Positioned(
          left: 0, right: 0,
          bottom: MediaQuery.of(context).viewInsets.bottom,
          child: PowerStrip(
            inputBridge: _bridge,
            modifierController: _modCtl,
            onMacrosTap: _openMacroSheet,
            leftHanded: _settings.leftHanded,
          ),
        ),
      ],
    ),
  );
}
```

### 8.8 Exit criteria

- [ ] All 8 keys fire correctly on the remote
- [ ] Modifier+letter combos work (e.g., ⌘+C copies on remote)
- [ ] Hebrew typing works via string injection
- [ ] Backspace works via sentinel detection
- [ ] Return key works via `onSubmitted`
- [ ] 2-finger scroll moves the remote scroll wheel
- [ ] Pinch is correctly distinguished from scroll
- [ ] Strip moves with iOS keyboard show/hide

---

## 9. Phase 3b — Macro System (0.5–1 week)

**Objective:** Curated macro library + bottom sheet picker. Custom macro builder deferred to v1.1.

### 9.1 Data model

```dart
// custom/macros/models/macro.dart

enum TargetOS { any, windows, macos, linux }
enum MacroCategory { system, editing, browser, ide, terminal, window, custom }

class Macro {
  final String id;
  final String name;
  final String? icon;          // emoji or icon name
  final List<MacroStep> steps;
  final MacroCategory category;
  final TargetOS targetOS;
  final bool isFavorite;
  final int useCount;
}

// custom/macros/models/macro_step.dart

sealed class MacroStep {}

class KeyChordStep extends MacroStep {
  final List<String> keys;     // ["ctrl", "shift", "t"]
  final int holdMs;
}

class StringStep extends MacroStep {
  final String text;
}

class DelayStep extends MacroStep {
  final int ms;
}

class KeyTapStep extends MacroStep {
  final String key;
  final int repeats;
}
```

### 9.2 Engine

```dart
// custom/macros/engine/macro_engine.dart

class MacroEngine {
  final InputBridge bridge;
  MacroEngine(this.bridge);

  Future<void> execute(Macro macro) async {
    HapticFeedback.mediumImpact();
    for (final step in macro.steps) {
      await switch (step) {
        KeyChordStep s => _runChord(s),
        StringStep s   => bridge.typeString(s.text),
        DelayStep s    => Future.delayed(Duration(milliseconds: s.ms)),
        KeyTapStep s   => _runTaps(s),
      };
    }
    // bump useCount, persist via macro_storage
  }

  Future<void> _runChord(KeyChordStep step) async {
    for (final k in step.keys) {
      await bridge._keyDown(k);
      await Future.delayed(const Duration(milliseconds: 12));
    }
    if (step.holdMs > 0) {
      await Future.delayed(Duration(milliseconds: step.holdMs));
    }
    for (final k in step.keys.reversed) {
      await bridge._keyUp(k);
      await Future.delayed(const Duration(milliseconds: 8));
    }
  }

  Future<void> _runTaps(KeyTapStep step) async {
    for (var i = 0; i < step.repeats; i++) {
      await bridge.tapKey(step.key);
      if (i < step.repeats - 1) {
        await Future.delayed(const Duration(milliseconds: 30));
      }
    }
  }
}
```

### 9.3 Bottom sheet UI

- Sticky search bar at top (fuzzy match name + key sequence)
- Sections: Favorites, Recent, Categories (collapsible)
- Tap to fire (and dismiss sheet)
- Long-press to toggle favorite
- "Create new macro" CTA at bottom (deferred to v1.1)

### 9.4 Persistence

JSON via `SharedPreferences`:

```json
{
  "macros": [
    {
      "id": "uuid-...",
      "name": "Lock screen",
      "icon": "🔒",
      "category": "system",
      "targetOS": "windows",
      "isFavorite": true,
      "useCount": 12,
      "steps": [
        { "type": "chord", "keys": ["meta", "l"], "holdMs": 0 }
      ]
    }
  ]
}
```

### 9.5 Exit criteria

- [ ] Bottom sheet opens from `[⚡ Macros]` button
- [ ] All defaults from §13 fire correctly
- [ ] Favorites persist
- [ ] Use count tracked
- [ ] Search works (fuzzy)
- [ ] Sheet dismisses after macro fires

---

## 10. Phase 4 — Polish (0.5–1 week)

### 10.1 Tasks

- [ ] Haptic feedback on all key presses (`HapticFeedback.lightImpact`)
- [ ] Visual press states for all keys (idle / pressed / held / sticky)
- [ ] Modifier indicators showing held state (color tint or underline)
- [ ] One-handed / handedness toggle in settings
- [ ] Scroll sensitivity slider in settings
- [ ] Inverted scroll toggle in settings
- [ ] Theme: light/dark/auto
- [ ] Connection error handling and reconnect UX
- [ ] Empty states for session list
- [ ] Loading states for connection
- [ ] Onboarding screen (first launch): explain the strip, scroll gesture
- [ ] App icon, launch screen
- [ ] Accessibility: minimum touch targets 44pt, VoiceOver labels on keys

---

## 11. Phase 5 — Distribution (1–2 weeks)

### 11.1 TestFlight

- [ ] Bundle ID + App ID configured
- [ ] App Store Connect record created (Name, SKU, primary language)
- [ ] Distribution certificate + provisioning profile installed
- [ ] First IPA uploaded via `xcrun altool` or Transporter
- [ ] Internal testing group created
- [ ] External testing group (up to 10k testers, requires beta review for first build)
- [ ] Build expiration handling: 90-day rolling window — plan for periodic re-uploads

### 11.2 Apple Enterprise (optional, for internal-only deployment)

- Requires DUNS number, registered company, $299/year
- Allows internal distribution outside the App Store
- Cannot be used for public/general distribution
- Apple is strict about misuse — read the agreement carefully

### 11.3 Crash reporting

- Integrate Sentry or Firebase Crashlytics for crash + error tracking
- Add a privacy notice in app describing what's collected

### 11.4 Exit criteria

- [ ] Build live on TestFlight, installable by invited testers
- [ ] Crash reports flowing to dashboard
- [ ] Documented release process: tag → build → upload → release notes

---

## 12. Reference: Data Models

Already specified inline above. Single source of truth files in repo:

- `custom/strip/models/key_def.dart`
- `custom/strip/models/strip_layout.dart`
- `custom/strip/models/modifier_state.dart`
- `custom/macros/models/macro.dart`
- `custom/macros/models/macro_step.dart`

---

## 13. Reference: Default Macro Library

This is the curated set that ships in v1. Stored in `custom/macros/data/default_macros.dart`.

### 13.1 System

| Name | Icon | Sequence | Target |
|---|---|---|---|
| Lock screen | 🔒 | `meta+l` | Windows |
| Lock screen | 🔒 | `control+meta+q` | macOS |
| Sign out (Win) | ⏏ | `control+alt+delete` | Windows |
| Show desktop | 🖥️ | `meta+d` | Windows |
| Mission Control | 🖥️ | `control+up` | macOS |
| Window switcher | 🪟 | `alt+tab` | Windows |
| App switcher | 🪟 | `meta+tab` | macOS |
| Spotlight | 🔍 | `meta+space` | macOS |
| Search | 🔍 | `meta+s` | Windows |
| Clipboard history | 📋 | `meta+v` | Windows |
| Screenshot region | 🖼️ | `meta+shift+s` | Windows |
| Screenshot region | 🖼️ | `meta+shift+4` | macOS |
| File explorer | 📂 | `meta+e` | Windows |
| Settings | ⚙️ | `meta+i` | Windows |
| Force quit | 🚫 | `alt+meta+escape` | macOS |
| Close window | ✕ | `alt+f4` | Windows |
| Close window | ✕ | `meta+w` | macOS |

### 13.2 Editing

| Name | Sequence (Win) | Sequence (macOS) |
|---|---|---|
| Copy | `control+c` | `meta+c` |
| Cut | `control+x` | `meta+x` |
| Paste | `control+v` | `meta+v` |
| Paste plain | `control+shift+v` | `alt+shift+meta+v` |
| Undo | `control+z` | `meta+z` |
| Redo | `control+shift+z` | `shift+meta+z` |
| Select all | `control+a` | `meta+a` |
| Find | `control+f` | `meta+f` |
| Find & replace | `control+h` | `alt+meta+f` |
| Save | `control+s` | `meta+s` |
| Save as | `control+shift+s` | `shift+meta+s` |

### 13.3 Browser

| Name | Sequence (Win) | Sequence (macOS) |
|---|---|---|
| New tab | `control+t` | `meta+t` |
| Reopen tab | `control+shift+t` | `shift+meta+t` |
| Close tab | `control+w` | `meta+w` |
| Next tab | `control+tab` | `control+tab` |
| Prev tab | `control+shift+tab` | `control+shift+tab` |
| Address bar | `control+l` | `meta+l` |
| Refresh | `control+r` | `meta+r` |
| Hard refresh | `control+shift+r` | `shift+meta+r` |
| Dev tools | `f12` | `alt+meta+i` |

### 13.4 IDE (JetBrains / VSCode)

| Name | JetBrains | VSCode |
|---|---|---|
| Go to file | `shift+meta+o` | `meta+p` |
| Go to symbol | `alt+meta+o` | `shift+meta+o` |
| Find usages | `alt+f7` | `shift+f12` |
| Refactor | `control+t` | `shift+meta+r` |
| Run | `control+r` | `f5` |
| Debug | `control+d` | `f5` |
| Toggle terminal | `alt+f12` | `control+grave` |
| Comment line | `meta+slash` | `meta+slash` |
| Multi-cursor down | `alt+meta+down` | (sequence) |
| Reformat | `alt+meta+l` | `alt+shift+f` |

### 13.5 Terminal / shell

| Name | Sequence |
|---|---|
| Cancel | `control+c` |
| Clear screen | `control+l` |
| End of line | `control+e` |
| Start of line | `control+a` |
| Search history | `control+r` |
| Send EOF | `control+d` |
| Suspend | `control+z` |
| Kill word | `control+w` |
| Tmux: new pane (vertical) | `control+b` → `percent` *(sequence)* |
| Tmux: new pane (horizontal) | `control+b` → `quote` *(sequence)* |
| Tmux: zoom pane | `control+b` → `z` *(sequence)* |

### 13.6 Window management

| Name | macOS (Rectangle/Magnet) | Windows |
|---|---|---|
| Snap left | `control+alt+left` | `meta+left` |
| Snap right | `control+alt+right` | `meta+right` |
| Snap top | `control+alt+up` | `meta+up` |
| Maximize | `control+alt+return` | `meta+up` |
| Next display | `control+alt+meta+right` | `meta+shift+right` |

### 13.7 User-specific (preset for first launch)

These are tailored to Ronen's workflow. Group them as a "Dev workflow" category. Easy to remove in settings if not desired.

| Name | Steps |
|---|---|
| Briya: open repo | `meta+shift+p` → delay 200ms → type "Open Folder" → `return` |
| Linear quick add | `meta+k` → delay 100ms → type "Add issue" → `return` |
| Tmux: new pane | chord `control+b` → delay 50ms → tap `percent` |
| Tmux: zoom | chord `control+b` → delay 50ms → tap `z` |
| Open Spotlight: ccr-pc | chord `meta+space` → delay 150ms → type "ccr-pc" → `return` |
| Claude Code session | type "cl" → tap `return` |

---

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| iOS build pipeline is fragile | High | High | Phase 0 spike must prove buildability before commitment |
| FFI signatures change upstream | Medium | Medium | Single seam in `InputBridge`, easy to fix |
| Apple rejects from App Store | High | Low | Don't aim for App Store; use TestFlight + Enterprise |
| Sentinel TextField bridge has edge cases | Medium | Medium | Extensive Phase 3 testing; especially Hebrew + autocorrect |
| Upstream merge conflicts grow | Medium | Medium | Sibling-directory pattern keeps surface area near-zero |
| Key name strings differ from assumed | High | Low | Phase 0 Day 3 reconnaissance documents the canonical set |
| 2-finger scroll conflicts with pinch-zoom | Medium | Low | Scale threshold (`abs(scale - 1) > 0.05` = zoom) |
| iOS keyboard doesn't appear / loses focus | Medium | High | Re-focus on every interaction; sentinel resilience |
| Macro timing drops events on remote | Medium | Low | Configurable delays; default 12ms between keydowns |
| TestFlight 90-day rolling expiration | Certain | Low | Document re-upload cadence in release process |

---

## 15. Acceptance Criteria

The project is considered "complete" when **all** of these are demonstrably true on a real iOS device against a self-hosted RustDesk server:

### Connection
- [ ] User can enter peer ID + password and connect via the custom UI
- [ ] Saved peers persist across launches
- [ ] Custom server config (hbbs/hbbr) is honored
- [ ] Connection errors surface clear, actionable messages

### Input — power strip
- [ ] All 8 strip keys (Esc, ⌃, ⌥, ⌘, Fn-or-placeholder, Tab, ⌫-or-iOS-native, arrows ←↓↑→, Macros) fire correctly
- [ ] Modifier sticky/oneshot/held modes work via tap/double-tap/long-press
- [ ] Modifier + letter combos work (e.g., ⌘+C)
- [ ] Visual press / held / sticky states render correctly

### Input — native keyboard
- [ ] English typing arrives on remote in real time
- [ ] Hebrew typing arrives correctly via string injection
- [ ] iOS native backspace deletes one char on remote (sentinel works)
- [ ] iOS native return key fires `return` on remote
- [ ] iOS shift / autocorrect / dictation all forward through

### Input — scroll
- [ ] 2-finger pan scrolls the remote desktop
- [ ] Pinch is correctly distinguished and does not scroll
- [ ] Sensitivity slider in settings affects scroll speed
- [ ] Inverted toggle reverses direction

### Macros
- [ ] Bottom sheet opens from strip
- [ ] At least the 13.1–13.6 default sets fire correctly
- [ ] Favorites persist
- [ ] Search filters macro list
- [ ] Sheet dismisses on selection

### UI / Polish
- [ ] Theme switches (light / dark / auto)
- [ ] Handedness toggle mirrors strip
- [ ] Onboarding screen explains the strip on first launch
- [ ] All keys have VoiceOver labels
- [ ] Minimum 44pt touch targets

### Distribution
- [ ] App is installable via TestFlight by an invited tester
- [ ] Crash reports flow to a dashboard
- [ ] Build pipeline reproducible via single script

### Maintainability
- [ ] All custom code lives in `flutter/lib/custom/`
- [ ] Upstream RustDesk Flutter code is touched in only one place (`main.dart` feature flag)
- [ ] `UPGRADE.md` documents the upstream merge process
- [ ] FFI calls are routed exclusively through `InputBridge`

---

## 16. Appendix A — Spike Notes Template

Drop this into the fork as `SPIKE_NOTES.md` on Day 1.

```markdown
# Spike Notes — RustDesk iOS Custom Fork

## Environment

- Date started: ____
- Rust version: ____ (from rust-toolchain.toml)
- Flutter version: ____ (from pubspec.yaml)
- flutter_rust_bridge version: ____ (from pubspec.lock)
- RustDesk commit / tag: ____
- Xcode version: ____

## Day 1 — Environment

- [ ] Rust + iOS targets installed
- [ ] Flutter pinned + builds vanilla
- [ ] vcpkg deps bootstrapped
- [ ] hbbs / hbbr running at: ____
- [ ] Apple Developer account ready
- Notes / blockers: ____

## Day 2 — Vanilla iOS build

- [ ] App built and signed
- [ ] Deployed to device (UDID: ____)
- [ ] Connected to remote desktop end-to-end
- [ ] Build time clean → IPA: ____ minutes
- Notes / blockers: ____

## Day 3 — Reconnaissance

### FFI: key input
- Function name: ____
- File path: ____
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
- Names:
  - ____
  - ____
  - ____ (full list)

### Existing keyboard overlay widget
- File: ____
- Notes: ____

### Remote page widget (mounting point)
- File: ____
- Notes: ____

## Day 4 — Sibling directory

- [ ] `flutter/lib/custom/` created
- [ ] `app_root.dart` stub renders
- [ ] Feature flag in `main.dart` works both ways
- Notes: ____

## Day 5 — Keyboard POC

- [ ] Custom button fired Esc on remote machine
- Path: button → InputBridge → bind.____ → remote
- Notes / surprises: ____

## Decision

- [ ] GREEN — proceed to Phase 1
- [ ] YELLOW — proceed with reduced scope: ____
- [ ] RED — abandon / revisit alternatives

Reason: ____
```

---

## 17. Appendix B — Glossary

| Term | Meaning |
|---|---|
| **hbbs / hbbr** | RustDesk relay servers (rendezvous + relay) |
| **FFI** | Foreign Function Interface (Dart ↔ Rust bridge) |
| **flutter_rust_bridge** | Codegen tool that generates Dart bindings for Rust functions |
| **Sentinel** | Zero-width space (`\u200B`) kept in the hidden TextField to detect backspace events |
| **Sibling-directory pattern** | All custom code lives in a sibling folder (`custom/`) to upstream code, minimizing merge conflicts |
| **InputBridge** | Single Dart class that wraps all FFI calls; the seam where upstream changes are absorbed |
| **One-shot modifier** | Modifier held only until the next key fires, then auto-released |
| **Sticky modifier** | Modifier held until tapped again (Caps-Lock-style) |
| **Power strip** | Custom 8-key overlay above the iOS native keyboard |
| **TestFlight** | Apple's beta distribution platform; up to 10k testers, 90-day rolling expiration |
| **Apple Enterprise** | $299/yr program for distributing internal-only iOS apps; not for public release |

---

**End of plan.**

> This document is the working spec for Claude Code. It is the source of truth for scope, structure, signatures, and acceptance criteria. Update it as the spike surfaces real-world constraints (Day 3 especially), and treat the file paths and function signatures inside as **best-effort placeholders** until the spike confirms them against the actual fork.
