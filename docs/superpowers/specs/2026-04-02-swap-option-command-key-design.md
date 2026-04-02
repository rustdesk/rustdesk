# Swap Option-Command Key Design

## Summary

Add a new session-level toggle for RustDesk remote desktop sessions that swaps `Option` and `Command` key behavior when the local client is macOS and the remote peer is not macOS.

This new toggle must coexist with the existing `Swap control-command key` feature, but the two toggles are mutually exclusive. Enabling one must automatically disable the other.

## Context

RustDesk already supports a session toggle named `allow_swap_key`, exposed as `Swap control-command key`, for cross-platform keyboard adaptation involving macOS `Command` and non-macOS `Control`.

The current code paths already support:

- Session-scoped toggle persistence through `LoginConfigHandler` and `PeerConfig`
- Flutter toolbar toggles in `flutter/lib/common/widgets/toolbar.dart`
- Sciter toolbar toggles in `src/ui/header.tis`
- Input rewriting for keyboard and mouse modifiers in `src/ui_session_interface.rs`

The new feature should follow the same architecture and remain session-scoped.

## Goals

- Add a separate user-configurable toggle for swapping `Option` and `Command`
- Only show and enable the new toggle when the local client is macOS and the remote peer is not macOS
- Keep the existing `Swap control-command key` behavior unchanged unless the new toggle is explicitly enabled
- Enforce mutual exclusivity between the two swap toggles
- Apply the swap consistently to keyboard events and mouse modifier state

## Non-Goals

- No changes to system keyboard settings on the local machine
- No changes to remote host input processing outside the normal RustDesk input pipeline
- No change to the existing `Swap control-command key` label, storage key, or semantics
- No generalized remapping UI for arbitrary modifier combinations

## User Experience

### Visibility Rules

Show `Swap option-command key` only when all of the following are true:

- The local client is macOS
- The remote peer platform is not macOS
- Keyboard input is enabled for the current session

This differs from the existing `Swap control-command key` toggle, which remains governed by its current visibility rules.

### Interaction Rules

- The new toggle is session-scoped, like the existing swap toggle
- Turning on `Swap option-command key` automatically turns off `Swap control-command key`
- Turning on `Swap control-command key` automatically turns off `Swap option-command key`
- Turning either toggle off leaves the other off unless the user explicitly enables it

## Data Model

Add a new boolean session config field:

- Internal key: `allow_swap_option_command_key`

This field should be stored and queried using the same `PeerConfig` and `LoginConfigHandler` mechanisms currently used by `allow_swap_key`.

## Input Mapping Behavior

### Keyboard Events

When `allow_swap_option_command_key` is enabled:

- `Alt` maps to `Meta`
- `Meta` maps to `Alt`
- `RAlt` maps to `Meta`
- `RWin` maps to `Alt`

The mapping must be applied in all keyboard event forms currently handled by `swap_modifier_key`:

- `control_key` values
- `modifiers` arrays
- Platform-specific `chr` or scan/key code values derived from `rdev`

The existing `allow_swap_key` path continues to swap `Control` and `Meta`.

### Mouse Events

When `allow_swap_option_command_key` is enabled, mouse modifier arrays must also swap:

- `Alt` with `Meta`
- `RAlt` with `Meta`
- `RWin` with `Alt`

This keeps click, drag, and wheel gestures consistent with keyboard state.

## Architecture

### UI

Add a new toggle entry in both UI stacks:

- Flutter: `flutter/lib/common/widgets/toolbar.dart`
- Sciter: `src/ui/header.tis`

Use a new translation string:

- `Swap option-command key`

### Session Config

Extend `LoginConfigHandler` so the new key participates in:

- Toggle writes
- Toggle reads
- Session persistence
- Mutual exclusion enforcement

Mutual exclusion must be enforced in Rust, not only in the UI, so persisted state remains coherent even if toggles are changed through other call sites.

### Input Rewrite Layer

Refactor the current swap logic in `src/ui_session_interface.rs` into explicit helper paths so the two behaviors are clear and isolated:

- Existing `Control <-> Command` swap path
- New `Option <-> Command` swap path

The caller should choose at most one active swap behavior per event based on the mutually exclusive session state.

## Error Handling

- If both toggles are somehow true in stored config, toggling either option should normalize state so only the chosen option remains enabled
- If neither toggle is enabled, input behavior remains unchanged
- Unsupported platforms must never see the new toggle in the UI

## Testing Strategy

### Rust Logic

Add targeted tests for:

- `toggle_option()` mutual exclusion behavior
- `get_toggle_option()` reads for the new field
- Modifier swap helper behavior for `Alt/Meta` and right-side variants

### UI

Validate that:

- Flutter shows the new toggle only for local macOS and remote non-macOS
- Sciter shows the new toggle under the same condition
- The existing `Swap control-command key` toggle remains available under its current condition

### Regression Checks

Verify that:

- Existing `allow_swap_key` behavior still swaps `Control` and `Command`
- Existing sessions without the new field continue to load without migration issues
- Keyboard and mouse input both respect the selected swap mode

## Risks

- Left/right modifier normalization is asymmetric in the existing code paths, so the new mapping must follow current conventions instead of inventing a more invasive left/right model
- UI-only mutual exclusion would be fragile, so Rust-side enforcement is required
- `chr` remapping must remain platform-aware to avoid breaking shortcut delivery on Windows, Linux, or macOS peers

## Implementation Outline

1. Add the new config/toggle key and Rust-side mutual exclusion logic
2. Add the new toolbar toggle in Flutter and Sciter with the macOS-local visibility condition
3. Split modifier swap logic into separate `Control-Command` and `Option-Command` helper paths
4. Apply the new swap mode to keyboard and mouse events
5. Add tests for toggle state and swap behavior
