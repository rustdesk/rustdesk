# PowerStrip Button Reference

The PowerStrip is Tabby's persistent on-screen keyboard, docked to the bottom of the remote session screen. It has two rows. The default layout is defined in `flutter/lib/custom/strip/layouts/default_strip.dart`. Modifier and disconnect keys are special; everything else types either a named key (via FFI `tapKey`) or a string.

For the broader feature context, see `features-vs-rustdesk.md`.

## Row 1 — Modifiers, common keys, layout controls

| Symbol | Type | Action / Key Sent | Description |
|--------|------|-------------------|-------------|
| ✕ | disconnect | — | Disconnect the current session and pop back to the session list. |
| ⇧ | modifier | `shift` (sticky) | Toggle Shift for the next non-modifier key. Stays pressed until you tap it again or send a key. |
| Ctrl | modifier | `control` (sticky) | Toggle Control. |
| ⏎ | regular | `return` | Press Return / Enter. |
| Esc | regular | `escape` | Press Escape. |
| ⌘ / ⊞ / Super | modifier | `meta` (sticky) | Toggle the platform meta key — `⌘` for macOS hosts, `⊞` for Windows, "Super" for Linux. |
| ⌥ / Alt | modifier | `alt` (sticky) | Toggle Option/Alt. `⌥` shown when remote is macOS, otherwise "Alt". |
| ▲▼ | stripToggle | — | Collapse / re-expand the PowerStrip itself (frees up canvas space). |
| Y | typeString | types `yes` then `⏎` | One-tap "yes\n" — confirms shell prompts (`apt`, `rm -i`, etc.) without opening the keyboard. |
| ⌨ | keyboardToggle | — | Show / hide the iOS soft keyboard (focuses or blurs the hidden TextField). |

## Row 2 — Editing keys, navigation, integrations

| Symbol | Type | Action / Key Sent | Description |
|--------|------|-------------------|-------------|
| 🖥 | displaySwitch | — | Opens the display picker sheet for hosts with multiple monitors. |
| ⌫ | regular | `backspace` | Backspace. |
| ⌦ | regular | `delete` | Forward Delete. |
| Tab | regular | `tab` | Tab. |
| ⎵ | regular | `space` | Space. |
| ⧉ | sessionSwitch | — | Opens the multi-session switcher sheet (tap to switch, swipe to disconnect, "+" to add). |
| 📎 | fileSend | — | Opens the File Send bottom sheet (destination picker → file picker → progress). |
| ⊞ | nextDisplay | — | Cycles to the **next** monitor on a multi-monitor host without opening the picker. |
| 💬 | chatToggle | — | Opens the terminal-chat overlay (partial bar; tap maximize for full xterm view). |
| ⇱ | regular | `home` | Home key. |
| ⇲ | regular | `end` | End key. |
| ⚡ | macroOpener | — | Opens the bottom-sheet **macros sheet** (10-action grid). The vertical FloatingMacroBar handle is separate; see `floating-macro-bar.md`. |
| ← | regular | `left` | Left arrow. |
| ↓ | regular | `down` | Down arrow. |
| ↑ | regular | `up` | Up arrow. |
| → | regular | `right` | Right arrow. |

## Notes

- Width factors and key heights are per-key (`KeyDef.widthFactor`, `KeyDef.height`). Row 2 keys are taller (48px) than Row 1.
- The whole layout can be **mirrored** for left-handed mode (`StripLayout.mirrored()`), swapping left/right groups.
- Modifier keys are **sticky** — they auto-release after sending one non-modifier key, unless toggled off manually first.
