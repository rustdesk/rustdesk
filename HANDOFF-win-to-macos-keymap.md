# Windows-to-macOS Shortcut Remapping Handoff

## Context

This local fork exists to explore a RustDesk-native keyboard remapping layer for the user's main workflow:

- Control side: Windows RustDesk client.
- Controlled side: macOS RustDesk server.
- Existing desired behavior to preserve:
  - `Ctrl+C` and `Ctrl+V` already behave correctly for copy/paste on the remote Mac.
  - `Ctrl+Tab` currently reaches RustDesk and can trigger the user's current application-switching workflow.
  - `Alt+Tab` on the Windows RustDesk client is captured by RustDesk and does not trigger the local Windows app switcher in the user's current workflow.
- Problem with external remappers:
  - macOS Karabiner-EventViewer does not see RustDesk-injected keyboard events.
  - Therefore Mac-side Karabiner rules are not a reliable fix for RustDesk remote input.

The preferred direction is to implement a small, peer-aware mapping layer inside the Windows RustDesk client after RustDesk captures keyboard input and before it sends `KeyEvent` messages to the remote peer.

## Current Source Baseline

- Repository: `https://github.com/rustdesk/rustdesk.git`
- Local branch: `my-win-keymap-handoff`
- Baseline commit: `456817b4f42114be5025263a7241dddb7470d479`
- Submodule:
  - `libs/hbb_common` at `387603f47cbb15c0d3dc3d67ae3396d3eb707daf`
- Fork product identity:
  - App name: `RustDesk-Herbin`
  - Namespace keyword: `herbin`
  - Bundle identifier: `com.herbin.rustdesk`
  - Windows installer/artifact prefix: `rustdesk-herbin`
  - Config namespace is intentionally separate from upstream RustDesk because config paths are derived from `APP_NAME`.

## Relevant Files

- `flutter/lib/common/widgets/remote_input.dart`
  - Flutter desktop focus layer routes keyboard events to `InputModel.handleKeyEvent`.
- `flutter/lib/models/input_model.dart`
  - `handleKeyEvent` chooses map/legacy routing.
  - `newKeyboardMode` calls `sessionHandleFlutterKeyEvent`.
  - `inputKey` calls `sessionInputKey` for legacy-compatible paths.
- `src/flutter_ffi.rs`
  - FFI bridge from Flutter to Rust session methods.
- `src/ui_session_interface.rs`
  - Converts Flutter key data into `rdev::Event`.
  - Calls `keyboard::client::process_event_with_session`.
- `src/keyboard.rs`
  - Main control-side keyboard conversion.
  - `event_to_key_events` dispatches by `KeyboardMode`.
  - `map_keyboard_mode`, `translate_keyboard_mode`, and `legacy_keyboard_mode` produce protocol `KeyEvent`s.
- `src/server/input_service.rs`
  - Controlled-side key injection.
  - Useful for understanding how remote macOS receives `ControlKey::Meta`, `ControlKey::Tab`, and modifiers.
- `libs/enigo/src/macos/macos_impl.rs`
  - macOS CoreGraphics injection via `CGEvent`.
  - Confirms RustDesk injection is not equivalent to a physical HID keyboard device.

## Existing Keyboard Pipeline

1. Flutter receives a key event in the remote desktop view.
2. `InputModel.handleKeyEvent` forwards the event to Rust through FFI.
3. `Session::handle_flutter_key_event` or `Session::input_key` builds or forwards key data.
4. `keyboard::client::event_to_key_events` converts the local event into one or more protocol `KeyEvent`s.
5. The remote macOS server receives `KeyEvent` and injects it through RustDesk's input service.

RustDesk supports three keyboard modes:

- `map`: physical-position style mapping.
- `translate`: layout-aware character-oriented behavior.
- `legacy`: compatibility mode used by older RustDesk behavior.

The shortcut remapping layer should not change the meaning of those modes globally. It should only intercept explicit shortcut patterns for a Windows client controlling a macOS peer.

## Goal

Add a minimal Windows-control-side remapping hook so selected Windows shortcuts become the same protocol-level events as the user's existing RustDesk shortcut workflow before RustDesk sends them to the peer.

First target:

- `Alt+Tab` on Windows control side -> current `Ctrl+Tab` effect on macOS controlled side.

Optional later target:

- `Alt+Shift+Tab` -> current `Ctrl+Shift+Tab` effect on macOS controlled side, if reverse cycling is desired and works reliably.

## Non-Goals

- Do not change `Ctrl+C`, `Ctrl+V`, or other copy/paste behavior.
- Do not change macOS Karabiner configuration.
- Do not add AutoHotkey or external Windows tools.
- Do not implement a broad keyboard-layout compatibility layer.
- Do not modify server-side macOS injection unless the control-side approach cannot meet the acceptance criteria.
- Do not change default behavior for Windows-to-Windows, Windows-to-Linux, macOS-to-macOS, mobile, or web sessions.
- Do not share the upstream RustDesk install identity or user configuration namespace in the installable fork.

## Recommended Design

Introduce a small control-side shortcut remapping function near the existing keyboard conversion boundary in `src/keyboard.rs`.

Suggested shape:

```rust
fn remap_shortcut_for_peer(
    peer: &str,
    keyboard_mode: KeyboardMode,
    event: &Event,
    key_events: Vec<KeyEvent>,
) -> Vec<KeyEvent> {
    // Only Windows client builds should handle this first version.
    // Only remap when peer == OS_LOWER_MACOS.
    // Only remap explicit Tab press/release events with Alt held.
    // Return the original key_events unchanged for every other case.
}
```

Implementation should be conservative:

- Compile the hook only for `target_os = "windows"`.
- Apply it only when `peer == OS_LOWER_MACOS`.
- Apply it only when the physical/logical key is `Tab`.
- Detect Alt using the existing modifier tracking helpers rather than querying unrelated global state ad hoc.
- Rewrite the generated `KeyEvent` for the Tab event into a legacy `ControlKey::Tab` event whose modifiers include `ControlKey::Control` instead of `ControlKey::Alt`.
- Preserve `Shift` when present, so `Alt+Shift+Tab` can map to the current `Ctrl+Shift+Tab` effect.
- Preserve key down/up semantics exactly.

The safest insertion point is after the current mode-specific conversion inside `event_to_key_events`, before lock-mode modifiers are added:

```rust
let mut key_events = match keyboard_mode {
    KeyboardMode::Map => map_keyboard_mode(peer.as_str(), event, key_event),
    KeyboardMode::Translate => translate_keyboard_mode(peer.as_str(), event, key_event),
    _ => legacy_keyboard_mode(event, key_event),
};

key_events = remap_shortcut_for_peer(peer.as_str(), keyboard_mode, event, key_events);
```

If tests show that legacy mode produces a cleaner representation for this shortcut, keep the remapper mode-aware and handle only the modes that are actually used by the Windows desktop client for the user's connection.

## Rule Semantics

Initial mapping table:

| Control side shortcut | Peer platform | Output protocol event |
| --- | --- | --- |
| `Alt+Tab` | macOS | legacy `ControlKey::Tab` with `ControlKey::Control` modifier |
| `Alt+Shift+Tab` | macOS | legacy `ControlKey::Tab` with `ControlKey::Control` and `ControlKey::Shift` modifiers |

Deliberately unchanged:

| Control side shortcut | Reason |
| --- | --- |
| `Ctrl+C` | Existing copy behavior must be preserved. |
| `Ctrl+V` | Existing paste behavior must be preserved. |
| `Ctrl+A/X/Z` | Not requested; changing them would broaden behavior risk. |
| Windows-to-Windows `Alt+Tab` | Non-macOS peers must keep existing behavior. |

## Suggested Implementation Tasks

### Task 1: Add Characterization Tests

Find or create focused tests around `src/keyboard.rs` conversion behavior. The test should prove:

- A Windows-style Tab event with Alt held and macOS peer produces a `KeyEvent` equivalent to the current `Ctrl+Tab` behavior.
- `Ctrl+C` and `Ctrl+V` remain unchanged.
- Non-macOS peers remain unchanged.
- Tab without Alt remains unchanged.

If `keyboard.rs` does not currently have unit-test scaffolding for these helpers, prefer extracting a pure helper that accepts:

- `peer: &str`
- `key: rdev::Key`
- `modifiers: ShortcutModifierState`
- `key_events: Vec<KeyEvent>`

and returns a transformed `Vec<KeyEvent>`.

Keep the extraction narrow. Do not refactor the whole keyboard pipeline.

### Task 2: Implement the Remapper

Add a small helper in `src/keyboard.rs` near `event_to_key_events`.

The helper should:

- Return original events unless all conditions match.
- Replace Alt modifier with Control for the target Tab shortcut.
- Keep Shift if it was present.
- Avoid duplicate modifiers.
- Avoid changing key release ordering.

Use existing `ControlKey` enum values and existing modifier helper patterns.

### Task 3: Wire the Helper

Call the helper from `event_to_key_events` after mode-specific conversion and before lock-state modifier injection.

Reason:

- The event has already been normalized into RustDesk protocol-level `KeyEvent`s.
- The hook remains close to the boundary where platform-specific shortcut semantics belong.
- Lock-state handling stays outside the shortcut rule.

### Task 4: Add a Feature Gate or Config Gate

Do not hard-enable the behavior globally in the long term.

Preferred first local fork option:

- Add a local-only boolean constant with a clear name such as `ENABLE_WINDOWS_TO_MACOS_SHORTCUT_REMAP`.
- Default it to `true` in this fork only.

Preferred productized option:

- Add a peer/session option in RustDesk config, but defer this until the local fork behavior is validated.

### Task 5: Validate Manually on Windows

Manual test matrix:

- Windows RustDesk fork controlling macOS peer:
  - `Ctrl+C` copies.
  - `Ctrl+V` pastes.
  - `Alt+Tab` has the same effect as the current `Ctrl+Tab` workflow.
  - `Alt+Shift+Tab` behavior is recorded.
  - Numpad digits still work after Mouse Keys is disabled on macOS.
- Windows RustDesk fork controlling Windows peer:
  - `Alt+Tab` is unchanged.
  - `Ctrl+C/V` unchanged.
- Windows RustDesk fork controlling Linux peer:
  - `Alt+Tab` is unchanged.

## Validation Commands

Before implementing:

```bash
git status --short
git submodule status
```

After implementing Rust-only changes:

```bash
cargo fmt --check
cargo test -p rustdesk --lib keyboard
```

If the exact package/test selector differs, inspect the workspace package names first:

```bash
cargo metadata --no-deps --format-version 1
```

Windows build validation will likely need a Windows environment. Do not claim the shortcut is fixed until it is tested from a real Windows RustDesk client against the macOS peer.

## Acceptance Criteria

- The fork contains a narrow, peer-aware shortcut remapping layer.
- `Alt+Tab` from Windows to macOS produces the same behavior as the current `Ctrl+Tab` workflow on the remote Mac.
- Existing `Ctrl+C` and `Ctrl+V` behavior remains unchanged.
- Non-macOS peers are unaffected.
- Tests cover the pure remapping behavior.
- Manual Windows-to-macOS verification is recorded before considering the change complete.

## Open Questions

- Should the mapping be hardcoded for this fork, or exposed as a per-peer advanced setting after the first validation pass?
