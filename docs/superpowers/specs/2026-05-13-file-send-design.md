# File Send Feature — Design Spec

**Date:** 2026-05-13  
**Status:** Approved

---

## Overview

Add a "send file to remote" button to the PowerStrip row 2 in Tabby's iOS remote session screen. Tapping it opens a bottom sheet where the user picks a destination on the remote machine, selects one or more files from the iOS file picker, and monitors transfer progress — all without leaving the session.

Primary direction: **iPhone → remote machine** (local → remote).

---

## Entry Point

### PowerStrip Row 2 — 📎 button

- **Location:** First button in `right` array of row 2, immediately right of the `⧉` session switcher (which lives in `middle`)
- **Icon:** 📎 (paperclip emoji)
- **Label:** `📎`
- **KeyType:** New enum value `fileSend` added to `KeyType` in `key_def.dart`
- **Size:** `widthFactor: 0.7`, `height: 48` — matches adjacent buttons
- **Definition in `default_strip.dart`:**

```dart
KeyDef(label: '📎', keyName: '', type: KeyType.fileSend, widthFactor: 0.7, height: 48),
```

Placed as the first item in the `right:` list of row 2.

---

## Bottom Sheet Flow

Tapping 📎 opens a `showModalBottomSheet` from `RemoteSessionScreen`.

### State 1 — Destination Picker

The sheet presents two ways to choose the remote destination:

**Common Destinations** — a horizontal chip row of preset paths:
- 🏠 Home (`~`) — selected by default
- 🖥 Desktop (`~/Desktop`)
- ⬇️ Downloads (`~/Downloads`)
- 📄 Documents (`~/Documents`)
- 🗂 /tmp

**Custom Path** — a text field pre-filled with `~`. The user can type any absolute or `~`-relative path. A 📁 browse button next to the field opens the existing `FileManagerPage` (mobile) filtered to the remote side, allowing folder navigation; selecting a folder populates the text field.

A **Destination** summary bar below shows the currently active path (highlighted in blue). It updates live as the user taps chips or edits the text field. Chips are mutually exclusive with the text field — tapping a chip overwrites the text field and vice versa.

**CTA:** "Choose File & Send" — opens the iOS native document picker (`file_picker` package, already a project dependency) with `allowMultiple: true`. Picking files transitions the sheet to the progress state.

### State 2 — Progress (single file)

- File name + size icon row
- Single progress bar (blue, animated)
- Bytes transferred / total + percentage (right-aligned, blue)
- Speed + estimated time remaining (small, grey)
- **Cancel** button at the bottom (red text, grey background)

### State 2 — Progress (multiple files)

- **Accordion row** (tappable, fills the sheet body):
  - Header: "N files · X MB total" + combined percentage + ▶/▼ chevron
  - Combined progress bar
  - Bytes transferred / total + speed
  - **Collapsed by default.** Tapping expands to reveal per-file rows.
- **Per-file rows** (inside accordion, scrollable if many):
  - File icon + truncated name + status badge
  - Thin 4px progress bar
  - States: ✓ Done (green), `N%` in progress (blue bar), Queued (grey, 0%)
  - Files transfer sequentially — only one bar animates at a time
- **Cancel** button at the bottom

### State 3 — Done

- Green checkmark in a rounded square
- "N files sent successfully" + total size (single file: just the filename)
- **"Send More Files"** button (grey, secondary) — resets sheet to destination picker
- **"Done"** button (green, primary) — pinned to the very bottom, dismisses sheet

---

## Architecture

### New files

None required — all changes are additive to existing files.

### Modified files

| File | Change |
|---|---|
| `flutter/lib/custom/strip/models/key_def.dart` | Add `fileSend` to `KeyType` enum |
| `flutter/lib/custom/strip/layouts/default_strip.dart` | Add `📎` `KeyDef` as first item in row 2 `right:` |
| `flutter/lib/custom/strip/widgets/power_strip.dart` | Handle `KeyType.fileSend` in `_handle()` — calls `widget.onFileSend()` callback |
| `flutter/lib/custom/screens/remote_session_screen.dart` | Add `onFileSend` callback wiring + `_onFileSend()` method that shows the bottom sheet |
| `flutter/lib/custom/widgets/file_send_sheet.dart` | **New widget** — the bottom sheet StatefulWidget with all 3 states |

### `FileSendSheet` widget

```
FileSendSheet(
  sessionId: String,          // passed through to FileController
  ffiModel: FfiModel,         // to resolve remote home dir
  onDone: VoidCallback,       // dismisses sheet
)
```

Internal state machine:
```
destinationPicker → sending → done
                       ↑
                 (send more files resets to destinationPicker)
```

Progress tracking uses `FileModel` / `JobController` from `gFFI.fileModel`. The sheet subscribes to job progress events via `ChangeNotifierProvider` / `Consumer` on `FfiModel` (same pattern used by existing file transfer UI).

### File transfer initiation

Uses the existing `FileController.sendFiles()` path:
1. Resolve destination: expand `~` using `bind.mainGetHomeDir()`
2. For each picked file path, call `bind.sessionSendFiles(sessionId, actId, localPath, remoteDest, ...)`
3. Files send sequentially (RustDesk job queue handles ordering)

### Remote path browser (📁 button)

Reuses `FileManagerPage` (mobile) pushed as a full-screen route with `isLocal: false`. On folder selection, pops with the selected path string which the sheet captures via `Navigator.pop(context, selectedPath)`.

---

## Error Handling

- **Transfer failure**: individual file row shows ✗ red badge + error message in grey. Other files continue. Done state shows "2 of 3 files sent" with a "Retry failed" secondary button.
- **Connection lost mid-transfer**: sheet shows an error state ("Connection lost") with a Dismiss button.
- **Empty destination path**: "Choose File & Send" button is disabled (greyed out) when the path field is empty.
- **Remote path does not exist**: RustDesk's existing error propagation surfaces this; shown as a per-file ✗ row.

---

## Out of Scope

- Remote → iPhone (download) direction
- Drag-and-drop from other iOS apps
- Background transfer (app backgrounded mid-send)
- Progress persistence across session switches
