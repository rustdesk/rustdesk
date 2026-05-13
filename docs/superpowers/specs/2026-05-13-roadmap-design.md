# Tabby iOS Roadmap — Design Spec

**Date:** 2026-05-13
**Audience:** Power users, personal hobbyists controlling macOS machines

---

## Context

Tabby differentiates from Jump Desktop, RustDesk, and AnyDesk through a custom PowerStrip keyboard, macro system, and multi-session support. The roadmap is depth-first: make existing pillars (input, macros) significantly better rather than chasing feature parity with enterprise tools like TeamViewer.

Primary target: **macOS remote machines**, **power users and hobbyists**.

---

## Milestone 1 — Daily Driver

**Goal:** Input experience good enough that a power user switches to Tabby as their daily driver.

| Feature | Description |
|---|---|
| **Trackpad mode** | Toggle in PowerStrip row 2. Switches from "tap teleports cursor" to relative trackpad mode. Drag = relative cursor move, left tap = click, two-finger tap = right click, two-finger drag = scroll. Sensitivity setting in Settings. |
| **Right-click ergonomics** | Dedicated right-click PowerStrip button (or one-tap toggle: next tap is right-click). Removes the long-press requirement. |
| **macOS quick-actions macro set** | Pre-loaded macro page with: Mission Control, App Exposé, Spaces left/right, App Switcher, Spotlight, Force Quit. One tap from FloatingMacroBar. |
| **Haptic feedback** | Vibration on key presses and PowerStrip taps. Toggle in Settings (on by default). |
| **Connection quality indicator** | Live latency (ms) + FPS overlay, corner of screen, dismissible. |

---

## Milestone 2 — Macro Power

**Goal:** Macros become a real personal automation layer, not just presets.

| Feature | Description |
|---|---|
| **Visual macro editor** | In-app editor: create, reorder, rename, delete macros. Action types: key combo, type-string, delay, open URL, remote shell command. Replaces hardcoded Dart definitions. |
| **Per-connection macro sets** | Different macro pages per connected machine (e.g. "Work Mac" vs "Home Mac"). |
| **iOS Shortcuts integration** | Expose macros as Shortcuts actions. Trigger from home screen widget, Lock Screen, or Siri. |
| **Macro import/export** | Export sets as JSON (AirDrop/Files). Import community macro sets. |

---

## Milestone 3 — Deep Mac Integration

**Goal:** Features only possible because the target is macOS.

| Feature | Description |
|---|---|
| **Reverse file transfer** | Download files from remote Mac to iPhone. File browser on remote → select → save to iOS Files app. Completes the file transfer story. |
| **SSH terminal tab** | Open SSH terminal alongside or instead of a remote session. Useful for headless Macs and Linux servers. |
| **Remote clipboard history** | Show last N clipboard entries from remote. Tap to paste any previous item. |
| **Spotlight launch shortcut** | PowerStrip key fires Cmd+Space. With type-ahead: tap → keyboard opens → type app name → Enter. App launches in 2 taps. |

---

## Prioritization Notes

- M1 ships first. Trackpad mode alone is a headline feature.
- M2 and M3 can overlap. SSH terminal (M3) may outrank macro editor (M2) for developers.
- iOS Shortcuts integration has high App Store discoverability value.
- Reverse file transfer (M3) completes the story started with file send in v1.4.6.
