# Tabby vs RustDesk — Feature Inventory

This document catalogs the **Tabby-specific features** layered on top of the upstream RustDesk iOS client. Everything below either replaces a RustDesk default UI surface or adds capability that does not exist in upstream RustDesk. The implementation lives almost entirely under `flutter/lib/custom/`.

For history and rebase notes, see `docs/tabby/upstream-diff.md`.

---

## Feature Categories

### 1. Connect / Session List Screen (`custom/screens/connect_screen.dart`)
Replaces RustDesk's stock mobile launcher.

- Custom tokenized dark theme (`custom/theme/`).
- Recent-peers list rendered as iOS-style cards (alias when present, otherwise ID; hostname subtitle).
- **Per-peer online indicator dot** — small badge over the computer icon. Driven by `bind.queryOnlines` whenever the recent-peers model reloads. RustDesk's mobile UI has no per-peer online dot.
- "Edit server ID" inline button that opens the existing RustDesk server-settings dialog (re-skinned entry).
- Hard cap of **5 simultaneous sessions** (`kMaxSessions`) with snackbar warning when exceeded.

### 2. Multi-Session Architecture (`custom/session/`)
- `SessionRegistry` singleton tracks up to 5 concurrent live FFI sessions, each with its own `FFI` instance.
- Per-session injection (RustDesk's mobile flow assumes one global FFI).
- `SessionSwitcherSheet` — bottom sheet that lists active sessions; tap to switch, swipe to disconnect.
- Active session id persists across foreground/background.

### 3. Custom Theme System (`custom/theme/`)
- `AppTokens` — colors, spacing, radii, typography. Single source of truth for visual style.
- Dark theme by default (`app_theme.dart`).
- Used by every Tabby-custom screen and overlay.

### 4. Remote Session Screen (`custom/screens/remote_session_screen.dart`)
Replaces RustDesk's mobile remote page wrapper.

Layered stack (z-order):
- **Layer 0** — remote canvas (auto-shrinks above the PowerStrip and keyboard, animated pan when keyboard appears).
- **Layer 1** — hidden 1×1 TextField (`TextFieldBridge`) that captures iOS keyboard input.
- **Layer 2** — PowerStrip (see §6).
- **Layer 3a/3b** — Terminal chat overlay (partial bar / max view).
- **Layer 4** — Cursor overlay, hoisted so the cursor can paint over the strip area without clipping.
- **Layer 4.5** — FloatingMacroBar (see §7).
- **Layer 5** — Full-screen dialog Overlay for password prompts etc.

Other behavior:
- **Per-peer display memory** — last-used monitor restored on reconnect (opt-in).
- **Per-peer zoom memory** — last canvas scale restored on reconnect (opt-in).
- **Zoom-to-fit** — single tap fits the remote display vertically to the canvas area.
- **Two-finger trackpad scroll** with sub-pixel delta accumulation (no rounding loss).
- **Mouse vs. touch mode** toggle with selection dialog explaining each.
- **Reconnect grace window** — modals suppressed for the first ~6 seconds after a reconnect.

### 5. Input Bridges (`custom/input/`)
- `input_bridge.dart` — high-level wrapper around RustDesk FFI for `tapKey(name, modifiers)` and `typeString(s)`. Used by every macro and PowerStrip key.
- `text_field_bridge.dart` — hidden TextField that channels iOS soft-keyboard input through to the remote host. Preserves trailing spaces, correct Return-key behavior with modifier flags, multi-line semantics, suppresses iOS autocorrect black border.

### 6. PowerStrip — Custom On-Screen Keyboard (`custom/strip/`)
A two-row, persistent overlay docked to the bottom of the session screen. Replaces RustDesk's small floating keyboard toggle entirely. Per-button reference: `powerstrip-buttons.md`.

- Sticky modifiers (`ModifierState`) — tap `⇧` then a letter to send Shift+letter; tap modifier again to release.
- `KeyDef` model with width factor + optional fixed height per key.
- Platform-aware labels — `⌘`/`⊞`/Super and `⌥`/Alt swap based on remote OS (`stripLayoutForPlatform`).
- Mirror layout supported (`StripLayout.mirrored()`) for left-handed users.
- Auto-pan: PowerStrip stays above the iOS soft keyboard when the hidden TextField is focused.

### 7. FloatingMacroBar — Vertical Quick-Macro Tab (`custom/overlay/floating_macro_bar.dart`)
A draggable lightning-bolt tab anchored to the right edge, just above the PowerStrip. Per-button reference: `floating-macro-bar.md`.

- Tap the tab to expand into a vertical column of macro buttons that grows upward.
- Height-capped + scrollable so it never clips off-screen on small devices.
- **Drag vertically** to reposition the handle; position persists (`settingsStore.macroBarTopOffset`).
- Collapsed/expanded state persists (`settingsStore.macroBarCollapsed`).
- Rectangle window-manager corner submenu — flyout to the left with ↖/↗/↙/↘ window placements (Rectangle macOS app).
- Haptic feedback on every tap.

### 8. Terminal Chat Overlay (`custom/chat/terminal_chat_overlay.dart`)
A chat-style input mode that types into the remote terminal.

- **Partial bar** — slim input row docked above PowerStrip; press Enter to send `text\n`.
- **Max view** — full-screen xterm.js view above the keyboard, mirrors the remote terminal buffer in real time. Send button replays through the InputBridge.
- Triggered by the 💬 button in the PowerStrip.

### 9. File Send (`custom/widgets/file_send_sheet.dart`)
Replaces RustDesk's barebones file-transfer flow.

- Bottom-sheet UX: pick destination chip (Home / Desktop / Downloads / Documents / /tmp) or type a custom path.
- Resolves `~` against **remote** home (not iOS sandbox) — primes `homePath` by issuing a directory listing as soon as the sheet opens.
- File picker invocation with explicit error dialogs (`file_picker` 10.3.10).
- Multi-file send (queue) with **per-file progress bar, transfer speed, and ETA**.
- Multi-file accordion view in "sending" state.
- "Send more files" loops back to destination picker.

### 10. Settings Store (`custom/settings/settings_store.dart`)
Thin `tabby:`-namespaced wrapper around `mainGet/SetLocalOption`. Persists:
- Custom ID server / relay server / public key.
- Left-handed layout.
- Scroll sensitivity + inversion.
- FloatingMacroBar position + collapsed state.
- Remember-last-display per peer.
- Remember-last-zoom per peer.

### 11. iOS Build / Branding Toolchain
- Custom Tabby app icons (all sizes replaced under `flutter/ios/Runner/Assets.xcassets`).
- `flutter/ios/exportOptions.plist` — Tabby bundle ID and signing profile.
- `rust-toolchain.toml` — pinned Rust 1.88.0 with iOS targets.
- `scripts/build-ios.sh` — single-command build helper.
- `docs/tabby/deploy-testflight.md` + `tabby-testflight` Claude skill — push-button TestFlight deploy.

---

## Button References

The per-button tables for the two on-screen control surfaces live in their own files:

- **PowerStrip** (the persistent two-row bottom keyboard) — see `powerstrip-buttons.md`.
- **FloatingMacroBar** (the draggable ⚡ tab in the bottom-right corner) — see `floating-macro-bar.md`.

---

## What Tabby Does NOT Change (vs. RustDesk)

- Wire protocol, peer discovery, rendezvous (`src/rendezvous_mediator.rs`).
- Screen capture (`libs/scrap/`), video codecs, audio pipeline.
- File-transfer protocol on the wire (UI only is replaced).
- Clipboard sync logic.
- Rust core security/auth — Tabby ships RustDesk's server binary and authentication unchanged.

All Tabby code is **additive** in `flutter/lib/custom/` plus targeted edits to `flutter/lib/main.dart`, `flutter/lib/mobile/pages/remote_page.dart`, `flutter/lib/common/widgets/remote_input.dart`, and `flutter/lib/common.dart` (see `docs/tabby/upstream-diff.md` for the exact conflict surface).
