# FloatingMacroBar Button Reference

The FloatingMacroBar is the lightning-bolt tab in the bottom-right corner of the remote session screen. Tap it to expand a vertical column that grows **upward**. All buttons send keystrokes through the InputBridge to the remote host. Implementation: `flutter/lib/custom/overlay/floating_macro_bar.dart`.

For the broader feature context, see `features-vs-rustdesk.md`.

## Buttons

| Label | Sends | Description |
|-------|-------|-------------|
| ⚡ / ✕ | — | The handle itself. Tap to expand/collapse; long-drag vertically to reposition. |
| `git\ncmt` | types `git commit\n` | Quick `git commit` — types the command and presses Return. |
| ⌃V | Ctrl+V | Paste on Linux/Windows hosts. |
| ⌘V | Cmd+V | Paste on macOS hosts. |
| ⌘⇧V | Cmd+Shift+V | "Paste and match style" on macOS. |
| ⌘⇧[ | Cmd+Shift+[ | 1Password quick-fill shortcut. |
| ⌘⎵ | Cmd+Space | Spotlight. |
| Tg | Cmd+Space → types `telegram` → Return | Multi-step macro: opens Spotlight, types "telegram", launches it. |
| ⌘⇥ | Cmd+Tab | macOS App Switcher. |
| ⌘N | Cmd+N | New window in the focused app. |
| ⇞ | Page Up | Page Up. |
| ⇟ | Page Down | Page Down. |
| ⌥↵ | Option+Return | Used by some shells (Warp, iTerm) for newline-without-submit. |
| F12 | F12 | Function key F12. |
| ⤢↑ | Ctrl+Alt+Cmd+↑ | **Rectangle: Maximize** window. |
| ⤢← | Cmd+Alt+← | **Rectangle: Left Half**. |
| ⤢→ | Cmd+Alt+→ | **Rectangle: Right Half**. |
| ▭ | — | Opens the Rectangle **corners submenu** (flyout to the left with ↖/↗/↙/↘). |
| ⌘⇧2 | Cmd+Shift+2 | Screenshot (macOS). |
| ⤢ | local action | **Zoom to fit** the remote display vertically; collapses the bar. |
| 📋→ | local action | **Paste iPhone clipboard** — reads iOS clipboard via `Clipboard.getData` and types it remotely; collapses the bar. |
| 🖱 / 👆 | local action | **Toggle mouse/touch mode**. Label is live — shows 🖱 in mouse mode, 👆 in touch mode. |

## Rectangle Corners Submenu (flyout)

Opened by tapping the ▭ button above. Flies out to the left of the main bar.

| Label | Sends | Description |
|-------|-------|-------------|
| ↖ | Ctrl+Alt+U | Rectangle: Top-Left quarter. |
| ↗ | Ctrl+Alt+I | Rectangle: Top-Right quarter. |
| ↙ | Ctrl+Alt+J | Rectangle: Bottom-Left quarter. |
| ↘ | Ctrl+Alt+K | Rectangle: Bottom-Right quarter. |

## Behavior Notes

- **Drag the ⚡ handle vertically** to reposition. Position persists across sessions (`settingsStore.macroBarTopOffset`).
- **Collapsed/expanded state persists** (`settingsStore.macroBarCollapsed`).
- The button column is **height-capped + scrollable** so it never clips off-screen on small devices.
- Every tap fires a **light haptic** (`HapticFeedback.lightImpact`).
- The Rectangle macros assume the [Rectangle](https://rectangleapp.com/) macOS app's default shortcuts.
