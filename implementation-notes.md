# Implementation Notes

## Deprecated Windows-to-macOS shortcut remap

- The earlier Windows-control-side `Alt+Tab` remap has been removed from `src/keyboard.rs`.
- Current direction: keep the Windows RustDesk client official/unmodified and perform the shortcut rewrite only on the macOS controlled side.
- The old handoff document remains as historical context only; do not use it as the active implementation source.

## macOS server-side Alt+Tab -> Command+Tab remap

- Scope: macOS peer (controlled side) only, legacy keyboard mode. Non-macOS behavior is unchanged; no `KEY_MAP`/translate changes, no copy/paste changes, no broad refactor.
- In `src/server/input_service.rs`, `legacy_keyboard_mode` now calls `try_remap_mac_alt_tab` before `sync_modifiers`. When it handles the event it returns early, skipping the normal `add_flags_to_enigo` + `process_control_key` Alt+Tab path.
- The helper rewrites only `ControlKey::Tab` events whose `modifiers` contain `Alt`/`RAlt`, and only when `Control`/`RControl`/`Meta`/`RWin` are NOT already present. `Shift`/`RShift` are preserved as flags so `Cmd+Shift+Tab` cycles backwards through the switcher.
- Decision (from the standalone POC): inject a `Command+Tab` shape via enigo rather than forwarding `Alt+Tab`. This drives the native macOS Cmd+Tab application switcher and, critically, auto-dismisses its UI.
- Exact injected event sequence, split across the Tab press and release events:
  - press (`down == true`): `reset_flag(); key_up(Alt/RAlt); add_flag(Meta)[; add_flag(Shift)]; key_down(Tab)`
  - release (`down == false`): `key_up(Tab); reset_flag(); key_up(Meta); key_up(Alt/RAlt)`
- Cleanup requirement: the trailing `key_up(Meta)` on the Tab release path is mandatory. It is what dismisses the macOS Cmd+Tab switcher UI; without it the switcher stays on screen after the remapped Tab. The `reset_flag()` before it drops the `Meta` (and any `Shift`) flag set during the press.
- Alt cleanup requirement: the preceding remote `Alt` key down may already have been injected before the Tab event reaches this remapper. The remap explicitly releases `Alt`/`RAlt` before sending the Command+Tab-shaped Tab down, and repeats that cleanup on release; the original later Alt-up event is harmless if it still arrives.
- Runtime testing caveat: exercise this path with an upstream/unmodified Windows client controlling the macOS fork, so the macOS peer receives the original remote `Alt+Tab`.
- The remapped path intentionally does not call `record_pressed_key` for Tab, so the `fix_key_down_timeout` cleanup loop will not track it. This is acceptable for the POC because each Tab press is paired with its own explicit `key_up(Tab)` on the matching release event; the only residual risk is a stuck switcher if a Tab release event never arrives (e.g. connection drop mid-keystroke).
- No unit tests were added: `src/server/input_service.rs` has no existing test module, and the remap path depends on the live macOS enigo/CGEvent virtual-input state that cannot be exercised in isolation. Verification is deferred to runtime testing on a macOS peer (not claimed here).

## RustDesk-Herbin branding and namespace

- This fork now uses `RustDesk-Herbin` as its default app name so it can be installed beside upstream RustDesk instead of replacing it.
- The macOS bundle identifier is `com.herbin.rustdesk`, the app bundle is `RustDesk-Herbin.app`, and the URL scheme is `rustdesk-herbin://`.
- macOS config and launchd namespace now use `com.herbin` plus `RustDesk-Herbin`, so the fork does not share the upstream `com.carriez.RustDesk*` service names or config directories.
- Mac DMG/build scripts and macOS GitHub Actions package paths publish `rustdesk-herbin-*` artifacts around `RustDesk-Herbin.app`.

## macOS JSON keymap configuration

- Current direction: keep the macOS controlled-side shortcut remapper in `src/server/input_service.rs`, but make the mapping rules adjustable without rebuilding.
- Runtime keymap path: `$HOME/.config/RustDesk-Herbin/herbin-keymap.json`, which is `/Users/herbin/.config/RustDesk-Herbin/herbin-keymap.json` for the target user.
- The app does not auto-create the JSON file. If the file is absent, unreadable, or invalid JSON, RustDesk-Herbin falls back to the existing `herbin-macos-keymap` option and then to the built-in `alt+tab=ctrl+tab` default.
- Current JSON scope is intentionally narrow: `tab`, `a-z0-9`, and `$same` with `alt`/`shift`/`ctrl`/`cmd` modifiers. The recommended experiment is a one-way Alt layer (`Alt+Tab` to `Ctrl+Tab`, `Alt+a-z0-9` to `Ctrl+same`) while leaving existing `Ctrl+...` shortcuts untouched.
- Supported JSON fields: top-level `enabled`, `rules`; per-rule `from`, `to`, `modes`; endpoint `key`, `modifiers`, `hold_until`. `hold_until` currently accepts `source_modifiers_released`, matching the intended Alt-held app-switcher behavior.
