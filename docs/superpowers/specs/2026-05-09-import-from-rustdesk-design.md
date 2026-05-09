# Import from RustDesk — Design Spec

**Date:** 2026-05-09  
**Status:** Approved

---

## Overview

Allow Tabby users to migrate their saved peer connections and server configuration from an existing RustDesk installation on the same iOS device. Because iOS sandboxes apps, the user must manually export the RustDesk `data/` folder via the Files app; Tabby then reads the folder via a file picker.

---

## Scope

**In scope:**
- Saved peers (`peers/*.toml`) — ID, password, hostname, username, platform
- Server config (`RustDesk2.toml`) — rendezvous server, relay server, API server, API key

**Out of scope:**
- Address book (`RustDesk_ab`) — not imported
- Device identity / keypair (`RustDesk.toml`) — not imported
- LAN peers (`RustDesk_lan_peers.toml`) — not imported

---

## Entry Points (Settings Page)

Two always-visible rows added to the main settings section in `flutter/lib/mobile/pages/settings_page.dart`, import row first:

1. **"Import from RustDesk"** — launches the file picker directly
2. **"How to export from RustDesk"** — opens the walkthrough modal

---

## Walkthrough Modal

A full-screen slide modal (`ImportFromRustdeskGuideModal`) opened by row 2. 5 slides, swipe to advance, no action button on the last slide:

| Slide | Title | Content |
|-------|-------|---------|
| 1 | What this does | "This imports your saved connections and server settings from the RustDesk app on this device." — includes a "Skip guide" link |
| 2 | Open Files app | Instruction to open the iOS Files app |
| 3 | Find the RustDesk folder | Path: On My iPhone → RustDesk → data |
| 4 | Save the folder | "Long-press the `data` folder → Share → Save to Files (or compress it to a zip)" |
| 5 | You're ready | "Go back to Settings and tap 'Import from RustDesk' to choose the folder." |

The modal is purely instructional. No file picker is launched from within it.

---

## File Picker & Parsing

**Trigger:** "Import from RustDesk" row in Settings.

**Picker type:** `FilePicker.platform.getDirectoryPath()` (already in pubspec as `file_picker: ^5.1.0`). User selects the `data/` folder directly — no zip support needed since RustDesk's Files sharing exposes the folder without compression.

**Parsing — Rust FFI:**  
A new Rust FFI function `mainImportRustdeskData(folderPath: String) -> String` (returns JSON).

It performs on the Rust side:
1. Locate `RustDesk2.toml` in the folder root — parse with `confy` into `Config2`, extract `rendezvous_server`, `relay_server`, `options["api-server"]`, `options["key"]`.
2. Scan `peers/` subdirectory — call `PeerConfig::load()` for each `.toml` file (existing logic, handles decryption automatically). Extract `id` (filename without `.toml`), `password`, `info.username`, `info.hostname`, `info.platform`.
3. Return JSON:
```json
{
  "server_config": {
    "id_server": "...",
    "relay_server": "...",
    "api_server": "...",
    "key": "..."
  },
  "peers": [
    { "id": "...", "password": "...", "username": "...", "hostname": "...", "platform": "..." }
  ]
}
```
If a section is missing or empty, its key is present but empty/null.

**Dart side:**  
Parse the returned JSON. Check each peer ID against existing Tabby peers. Separate into:
- `newPeers` — no conflict
- `conflictPeers` — ID already exists in Tabby

---

## Conflict Resolution

If `conflictPeers` is non-empty, show `ImportConflictDialog` — a list of conflicting peer IDs with per-row action buttons and global actions at the bottom.

**Per-row actions:** Override | Skip  
**Global actions (bottom of dialog):**
- **Override All** — apply override to all remaining unresolved conflicts
- **Skip All** — apply skip to all remaining unresolved conflicts
- **Stop** — abort the entire import; no changes are written

"Stop" means zero side effects — nothing is written to disk before the conflict dialog is resolved.

---

## Import Execution

After conflict resolution, apply in this order:
1. Write new and overridden peer configs via `bind.mainImportPeer(id, peerJson)`  (new FFI call — stores a `PeerConfig` to the `peers/` directory)
2. Apply server config via existing `setServerConfig()` (validates servers before writing)

Show result toast: `"Imported X peers and server config"` / `"Imported X peers"` / `"Imported server config"` as appropriate.

---

## New Code

| File | Change |
|------|--------|
| `flutter/lib/mobile/pages/settings_page.dart` | Add two new rows to main settings section |
| `flutter/lib/mobile/widgets/import_rustdesk_guide.dart` | New: walkthrough modal widget |
| `flutter/lib/mobile/widgets/import_conflict_dialog.dart` | New: conflict resolution dialog |
| `flutter/lib/mobile/widgets/import_rustdesk.dart` | New: orchestration logic (file picker → FFI → conflict → execute) |
| `flutter/lib/bridge_generated.dart` + Rust `flutter_ffi.rs` | New FFI: `mainImportRustdeskData`, `mainImportPeer` |
| `libs/hbb_common/src/config.rs` or `src/flutter_ffi.rs` | Implement `main_import_rustdesk_data` and `main_import_peer` |

---

## Error Handling

- Folder contains no recognizable RustDesk files → toast: `"No RustDesk data found in selected folder"`
- `RustDesk2.toml` missing → skip server config, import peers only (and vice versa)
- Individual peer file parse failure → skip that peer, log warning, continue
- Server validation fails in `setServerConfig()` → show error, peers already written are kept

---

## Testing Criteria

- Selecting a valid RustDesk `data/` folder imports all peers and server config correctly
- Peers with forbidden path characters (base64-encoded filenames) are decoded correctly
- Conflict dialog: Override, Override All, Skip, Skip All, Stop each behave as specified
- Stop leaves Tabby state completely unchanged
- Missing `RustDesk2.toml` → peers imported, server config skipped
- Empty `peers/` directory → server config imported, peer section skipped
- Invalid folder (no recognized files) → error toast, no state change
