# Tabby — Feature Migration Recommendations

Upstream: RustDesk mobile (`flutter/lib/mobile/`)  
Custom: Tabby-specific UI (`flutter/lib/custom/`)

Priorities: **High** = user-facing capability loss, **Medium** = UX friction, **Low** = nice-to-have.

---

## 1. PowerStrip / Keyboard

### P1 — High

**Function keys (F1–F12)**  
`KeyType.layer` is defined but stubbed ("not implemented in v1"). Upstream `KeyHelpTools` exposes a full Fn row. Without it, users have no way to send function keys to the remote.  
_Implementation:_ Add a second layer layout; tapping a `layer` key swaps the strip to the Fn row for one tap then reverts.

**Home, End, Insert, PrtScr, ScrollLock, Pause, Menu**  
Upstream exposes these via a "More" state in `KeyHelpTools`. Not in the PowerStrip at all.  
_Implementation:_ Row 2 right-side has room; add as optional overflow or inside a "More" popover.

**Android system navigation buttons (Back, Home, Apps, Volume, Power)**  
Upstream `showActions()` dynamically adds these for Android peers. The custom `_onMacrosTap` is an unimplemented stub.  
_Implementation:_ Detect `pi.platform == kPeerPlatformAndroid`; surface these in the macros sheet or as a conditional strip row.

### P2 — Medium

**Touch / mouse mode toggle**  
Upstream bottom bar shows a touch/mouse icon that also opens `GestureHelp`. Removed with `hideBottomBar: true` and not replaced.  
_Implementation:_ Add `KeyType.touchModeToggle` to the strip; toggle `ffiModel.touchMode` and show a brief overlay.

**Fn layer label feedback**  
When modifiers are held, upstream shows visual state on each key. The custom `ModifierController` handles this — extend the same pattern to the Fn layer keys.

---

## 2. Connect / Login Screen

### P1 — High

**Full peer search & autocomplete**  
Upstream `ConnectionPage` uses `RawAutocomplete` backed by `AllPeersLoader` — searches all known peers by ID, alias, hostname, and username with async loading. Tabby's `ConnectScreen` only shows the 5 most recent peers.  
_Implementation:_ Replace `_PeerIdField` with a `RawAutocomplete` widget backed by `bind.mainGetRecentPeers()` + `bind.mainGetAllPeers()`. Limit dropdown to 6 results.

### P2 — Medium

**Tab navigation (Settings, Server, Chat)**  
Upstream `HomePage` is a `BottomNavigationBar` with Connection, Server, and Settings tabs. Tabby has no in-app settings; users must find them elsewhere.  
_Implementation:_ Add a `BottomNavigationBar` to `ConnectScreen` (or a parent scaffold) with at minimum a Settings tab pointing to the existing upstream `SettingsPage`.

**Auto-fill last remote ID**  
Upstream loads the last-used peer ID on init. Tabby's ID field always starts empty.  
_Implementation:_ On `initState`, call `bind.mainGetLastRemoteId()` and pre-populate `_idController`.

### P3 — Low

**Update notification banner**  
Upstream shows a pink banner when `updateUrl` is set. Easy to add; not urgent.

**Clear ID button**  
Upstream shows an `×` icon when the ID field is non-empty. Minor UX polish.

---

## 3. Session Layout (`RemoteSessionScreen`)

### P1 — High

**iOS soft keyboard workarounds**  
Upstream `RemotePage` has dedicated `_handleIOSSoftKeyboardInput()` with 100 ms timers to work around Flutter bugs [#39900](https://github.com/flutter/flutter/issues/39900) and [#159384](https://github.com/flutter/flutter/issues/159384). The custom `TextFieldBridge` simplifies this — worth auditing against the upstream workarounds to confirm none are regressed.

### P2 — Medium

**Floating Action Button (FAB) for bar visibility**  
Upstream shows a FAB when `_showBar` is false so users can always reveal the toolbar. The PowerStrip collapse button (`▲▼`) is the only affordance in Tabby — not obvious on first use.  
_Implementation:_ Show a small floating pill/button when the strip is fully collapsed.

**Gesture help / touch–mouse toggle**  
`hideKeyHelpTools: true` suppresses `GestureHelp` entirely. New users have no in-session reminder of touch gestures.  
_Implementation:_ Expose a one-time dismissible overlay on first session, or add a `KeyType.gestureHelp` strip button.

### P3 — Low

**Quality monitor**  
`QualityMonitor` widget is rendered inside `RemotePage` but not explicitly positioned in `RemoteSessionScreen`. Verify it's not occluded by the strip.

---

## 4. RustDesk Data Import

### RustDesk iOS file layout (verified on-device)

On iOS, RustDesk stores all data at the **root app folder** (On My iPhone → RustDesk), not in a `data/` subfolder. The `data/` subfolder exists but is always empty. The root folder contains:

| File / Folder | Contents |
|---|---|
| `RustDesk2.toml` | Server config (ID server, relay, API, key) |
| `RustDesk.toml` | Main settings (enc_id, key pair, options) |
| `RustDesk_local.toml` | Local display/UI preferences |
| `peers/` | One `.toml` per saved connection (hostname, username, platform, options) |
| `RustDesk_ab` | Address book entries |
| `RustDesk_lan_peers.toml` | LAN-discovered peers |
| `data/` | Always empty on iOS (possibly used on other platforms) |
| `Shared/` | Shared storage folder (usually empty) |

### What Tabby currently imports

`main_import_rustdesk_data` (Rust FFI) reads from whatever folder path is passed:
- `<folder>/RustDesk2.toml` → server config (id_server, relay_server, api_server, key)
- `<folder>/peers/*.toml` → peer list (id, hostname, username, platform)

The Dart import flow picks the folder via `FilePicker.getDirectoryPath`, then calls the FFI.

### Confirmed working

Copying files from the root RustDesk folder (not `data/`) into the folder picker successfully migrates the connections list. The guide slides were updated to direct users to save the root `RustDesk` folder, not the empty `data` subfolder.

### Not imported (known gaps)

- `RustDesk.toml` — main settings (password, key pair) — not read; would require more careful handling to avoid overwriting Tabby's own identity
- `RustDesk_local.toml` — local UI preferences — low value to import
- `RustDesk_ab` — address book — binary/custom format, not TOML; would need a separate parser
- `RustDesk_lan_peers.toml` — LAN peers — ephemeral, not worth importing

---

## Summary Table

| Feature | Area | Priority | Effort |
|---|---|---|---|
| F1–F12 function keys | Strip | **P1** | Medium |
| Home / End / PrtScr / etc. | Strip | **P1** | Small |
| Android system nav buttons | Strip | **P1** | Medium |
| Full peer search / autocomplete | Connect | **P1** | Medium |
| iOS keyboard workaround audit | Session | **P1** | Small |
| Touch / mouse toggle | Strip | P2 | Small |
| Tab navigation + Settings | Connect | P2 | Medium |
| Auto-fill last remote ID | Connect | P2 | Small |
| FAB when strip collapsed | Session | P2 | Small |
| Gesture help access | Session | P2 | Small |
| Update banner | Connect | P3 | Small |
| Clear ID button | Connect | P3 | Small |
| Quality monitor positioning | Session | P3 | Small |
