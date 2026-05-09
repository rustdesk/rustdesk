# Import from RustDesk Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow Tabby iOS users to import saved peer connections and server config from an existing RustDesk installation on the same device via a folder file picker.

**Architecture:** Two new `SettingsTile` rows are added to Settings. "Import from RustDesk" opens a folder picker, passes the path to a new Rust FFI function `main_import_rustdesk_data` that parses `RustDesk2.toml` and `peers/*.toml` and returns JSON. The Dart side resolves conflicts via a dialog, then writes peers and server config. "How to export from RustDesk" opens a 5-slide walkthrough modal.

**Tech Stack:** Flutter/Dart, Rust FFI (flutter_rust_bridge 1.80.1), `file_picker: ^5.1.0` (already in pubspec), `settings_ui` (existing settings tiles), `serde_json` + `confy` on Rust side.

---

## File Map

| Action | File |
|--------|------|
| Modify | `flutter/lib/mobile/pages/settings_page.dart` — add 2 SettingsTile rows |
| Create | `flutter/lib/mobile/widgets/import_rustdesk_guide.dart` — walkthrough modal |
| Create | `flutter/lib/mobile/widgets/import_rustdesk.dart` — orchestration: picker → FFI → conflict → execute |
| Create | `flutter/lib/mobile/widgets/import_conflict_dialog.dart` — conflict resolution dialog |
| Modify | `src/flutter_ffi.rs` — add `main_import_rustdesk_data` and `main_import_peer` |
| Modify | `flutter/lib/generated_bridge.dart` — add Dart wrappers (hand-written following existing pattern) |

---

## Task 1: Rust FFI — `main_import_rustdesk_data`

Reads a RustDesk `data/` folder and returns parsed peers + server config as JSON. No writes.

**Files:**
- Modify: `src/flutter_ffi.rs`
- Modify: `libs/hbb_common/src/config.rs` (add `load_from_path` helpers)

- [ ] **Step 1: Add `PeerConfig::load_from_path` to config.rs**

  Open `libs/hbb_common/src/config.rs`. After the `load(id: &str)` function at line 1601, add:

  ```rust
  pub fn load_from_path(path: &std::path::Path) -> Option<(String, PeerConfig)> {
      let stem = path.file_stem()?.to_str()?;
      // Decode base64-encoded IDs (IDs with forbidden chars are stored base64-encoded)
      let id = if stem.starts_with("base64_") {
          let encoded = stem.trim_start_matches("base64_");
          String::from_utf8(base64::decode(encoded).ok()?).ok()?
      } else {
          stem.to_owned()
      };
      let config: PeerConfig = confy::load_path(path).unwrap_or_default();
      if config.info.platform.is_empty() {
          return None; // skip empty/invalid peer files
      }
      Some((id, config))
  }
  ```

  > Note: `base64` crate is already used in this file. Check existing imports before adding.

- [ ] **Step 2: Verify `base64` is already imported in config.rs**

  Run:
  ```bash
  grep -n "^use base64\|extern crate base64\|base64::" /Users/ronenmars/Desktop/dev/apps/ios/Tabby/libs/hbb_common/src/config.rs | head -5
  ```

  If not present, add to the top of the file: `use base64;`

- [ ] **Step 3: Add `main_import_rustdesk_data` to flutter_ffi.rs**

  Open `src/flutter_ffi.rs`. Add after `pub fn main_remove_peer` (line 1794):

  ```rust
  pub fn main_import_rustdesk_data(folder: String) -> String {
      use std::path::Path;
      use hbb_common::config::{Config2, PeerConfig};
      use serde_json::{json, Value};

      let folder_path = Path::new(&folder);

      // Parse server config from RustDesk2.toml
      let config2_path = folder_path.join("RustDesk2.toml");
      let server_config: Value = if config2_path.exists() {
          let c: Config2 = confy::load_path(&config2_path).unwrap_or_default();
          let relay = c.options.get("relay-server").cloned().unwrap_or_default();
          let api = c.options.get("api-server").cloned().unwrap_or_default();
          let key = c.options.get("key").cloned().unwrap_or_default();
          json!({
              "id_server": c.rendezvous_server,
              "relay_server": relay,
              "api_server": api,
              "key": key,
          })
      } else {
          json!({ "id_server": "", "relay_server": "", "api_server": "", "key": "" })
      };

      // Parse peers from peers/ subdirectory
      let peers_dir = folder_path.join("peers");
      let mut peers: Vec<Value> = Vec::new();
      if let Ok(entries) = std::fs::read_dir(&peers_dir) {
          for entry in entries.flatten() {
              let path = entry.path();
              if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                  continue;
              }
              if let Some((id, config)) = PeerConfig::load_from_path(&path) {
                  peers.push(json!({
                      "id": id,
                      "username": config.info.username,
                      "hostname": config.info.hostname,
                      "platform": config.info.platform,
                  }));
              }
          }
      }

      json!({
          "server_config": server_config,
          "peers": peers,
      })
      .to_string()
  }
  ```

  > Note: `confy` is already used in config.rs — verify it's accessible from flutter_ffi.rs or import via `hbb_common::config`. Check with: `grep -n "confy" src/flutter_ffi.rs | head -3`

- [ ] **Step 4: Add `main_import_peer` to flutter_ffi.rs**

  Directly after `main_import_rustdesk_data`, add:

  ```rust
  pub fn main_import_peer(id: String, username: String, hostname: String, platform: String) {
      use hbb_common::config::PeerConfig;
      let mut config = PeerConfig::load(&id);
      config.info.username = username;
      config.info.hostname = hostname;
      config.info.platform = platform;
      config.store(&id);
  }
  ```

- [ ] **Step 5: Build Rust to check for compile errors**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby && cargo build --features flutter 2>&1 | grep -E "error\[|warning\[" | head -30
  ```

  Expected: no errors. Fix any type or import errors before continuing.

- [ ] **Step 6: Commit**

  ```bash
  git add src/flutter_ffi.rs libs/hbb_common/src/config.rs
  git commit -m "feat(rust): add main_import_rustdesk_data and main_import_peer FFI functions"
  ```

---

## Task 2: Dart FFI Wrappers

Hand-write the Dart bridge wrappers following the exact pattern in `flutter/lib/generated_bridge.dart`.

**Files:**
- Modify: `flutter/lib/generated_bridge.dart`

- [ ] **Step 1: Add `mainImportRustdeskData` to the abstract class**

  Open `flutter/lib/generated_bridge.dart`. Find the abstract class `Rustdesk` (line ~16). Locate the `mainRemovePeer` declaration and add after it:

  ```dart
  Future<String> mainImportRustdeskData({required String folder, dynamic hint});
  FlutterRustBridgeTaskConstMeta get kMainImportRustdeskDataConstMeta;

  Future<void> mainImportPeer(
      {required String id,
      required String username,
      required String hostname,
      required String platform,
      dynamic hint});
  FlutterRustBridgeTaskConstMeta get kMainImportPeerConstMeta;
  ```

- [ ] **Step 2: Add implementations to `RustdeskImpl`**

  Find `RustdeskImpl` class in `generated_bridge.dart`. Locate the `mainRemovePeer` implementation and add after it:

  ```dart
  Future<String> mainImportRustdeskData(
      {required String folder, dynamic hint}) {
    var arg0 = _platform.api2wire_String(folder);
    return _platform.executeNormal(FlutterRustBridgeTask(
      callFfi: (port_) =>
          _platform.inner.wire_main_import_rustdesk_data(port_, arg0),
      parseSuccessData: _wire2api_String,
      constMeta: kMainImportRustdeskDataConstMeta,
      argValues: [folder],
      hint: hint,
    ));
  }

  FlutterRustBridgeTaskConstMeta get kMainImportRustdeskDataConstMeta =>
      const FlutterRustBridgeTaskConstMeta(
        debugName: "main_import_rustdesk_data",
        argNames: ["folder"],
      );

  Future<void> mainImportPeer(
      {required String id,
      required String username,
      required String hostname,
      required String platform,
      dynamic hint}) {
    var arg0 = _platform.api2wire_String(id);
    var arg1 = _platform.api2wire_String(username);
    var arg2 = _platform.api2wire_String(hostname);
    var arg3 = _platform.api2wire_String(platform);
    return _platform.executeNormal(FlutterRustBridgeTask(
      callFfi: (port_) =>
          _platform.inner.wire_main_import_peer(port_, arg0, arg1, arg2, arg3),
      parseSuccessData: _wire2api_unit,
      constMeta: kMainImportPeerConstMeta,
      argValues: [id, username, hostname, platform],
      hint: hint,
    ));
  }

  FlutterRustBridgeTaskConstMeta get kMainImportPeerConstMeta =>
      const FlutterRustBridgeTaskConstMeta(
        debugName: "main_import_peer",
        argNames: ["id", "username", "hostname", "platform"],
      );
  ```

- [ ] **Step 3: Add the wire-level declarations**

  In `generated_bridge.dart`, find the section with `wire_main_remove_peer` (search for it). Add after it:

  ```dart
  void wire_main_import_rustdesk_data(
    int port_,
    ffi.Pointer<wire_uint_8_list> folder,
  );

  void wire_main_import_peer(
    int port_,
    ffi.Pointer<wire_uint_8_list> id,
    ffi.Pointer<wire_uint_8_list> username,
    ffi.Pointer<wire_uint_8_list> hostname,
    ffi.Pointer<wire_uint_8_list> platform,
  );
  ```

  Then find the `_lookup` section (where wire functions are looked up via DynamicLibrary). Add after the `wire_main_remove_peer` lookup:

  ```dart
  late final wire_main_import_rustdesk_data = _lookup<
      ffi.NativeFunction<
          ffi.Void Function(
    ffi.Int64,
    ffi.Pointer<wire_uint_8_list>,
  )>>('wire_main_import_rustdesk_data');

  late final wire_main_import_peer = _lookup<
      ffi.NativeFunction<
          ffi.Void Function(
    ffi.Int64,
    ffi.Pointer<wire_uint_8_list>,
    ffi.Pointer<wire_uint_8_list>,
    ffi.Pointer<wire_uint_8_list>,
    ffi.Pointer<wire_uint_8_list>,
  )>>('wire_main_import_peer');
  ```

- [ ] **Step 4: Verify Flutter can analyze the file**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter && flutter analyze lib/generated_bridge.dart 2>&1 | grep -E "error|warning" | head -20
  ```

  Expected: no errors.

- [ ] **Step 5: Commit**

  ```bash
  git add flutter/lib/generated_bridge.dart
  git commit -m "feat(dart): add mainImportRustdeskData and mainImportPeer FFI wrappers"
  ```

---

## Task 3: Walkthrough Guide Modal

A 5-slide full-screen modal explaining how to export from RustDesk via Files app.

**Files:**
- Create: `flutter/lib/mobile/widgets/import_rustdesk_guide.dart`

- [ ] **Step 1: Create the guide modal file**

  Create `flutter/lib/mobile/widgets/import_rustdesk_guide.dart`:

  ```dart
  import 'package:flutter/material.dart';
  import '../../common.dart';

  class ImportRustdeskGuideModal extends StatefulWidget {
    const ImportRustdeskGuideModal({Key? key}) : super(key: key);

    @override
    State<ImportRustdeskGuideModal> createState() =>
        _ImportRustdeskGuideModalState();
  }

  class _ImportRustdeskGuideModalState
      extends State<ImportRustdeskGuideModal> {
    final PageController _controller = PageController();
    int _currentPage = 0;

    final List<_GuideSlide> _slides = const [
      _GuideSlide(
        icon: Icons.swap_horiz,
        title: 'Import from RustDesk',
        body:
            'This will import your saved connections and server settings from the RustDesk app on this device.\n\nFollow the steps below to export your data first.',
      ),
      _GuideSlide(
        icon: Icons.folder_open,
        title: 'Open Files App',
        body:
            'Open the Files app on your iPhone. You can find it on your Home Screen or in your App Library.',
      ),
      _GuideSlide(
        icon: Icons.phone_iphone,
        title: 'Find the RustDesk Folder',
        body:
            'Navigate to:\n\nOn My iPhone → RustDesk → data\n\nThis folder contains your saved connections and settings.',
      ),
      _GuideSlide(
        icon: Icons.share,
        title: 'Save the Folder',
        body:
            'Long-press the "data" folder, then tap Share → Save to Files.\n\nSave it somewhere easy to find, such as iCloud Drive or On My iPhone.',
      ),
      _GuideSlide(
        icon: Icons.check_circle_outline,
        title: 'You\'re Ready',
        body:
            'Go back to Tabby Settings and tap "Import from RustDesk" to select the folder you just saved.',
      ),
    ];

    @override
    void dispose() {
      _controller.dispose();
      super.dispose();
    }

    void _next() {
      if (_currentPage < _slides.length - 1) {
        _controller.nextPage(
            duration: const Duration(milliseconds: 300),
            curve: Curves.easeInOut);
      } else {
        Navigator.of(context).pop();
      }
    }

    @override
    Widget build(BuildContext context) {
      return Scaffold(
        appBar: AppBar(
          title: Text(translate('How to Export from RustDesk')),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: Text(translate('Skip'),
                  style: const TextStyle(color: Colors.white)),
            ),
          ],
        ),
        body: Column(
          children: [
            Expanded(
              child: PageView.builder(
                controller: _controller,
                itemCount: _slides.length,
                onPageChanged: (i) => setState(() => _currentPage = i),
                itemBuilder: (context, i) =>
                    _SlideView(slide: _slides[i]),
              ),
            ),
            _DotsIndicator(
                count: _slides.length, current: _currentPage),
            const SizedBox(height: 16),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
              child: SizedBox(
                width: double.infinity,
                child: ElevatedButton(
                  onPressed: _next,
                  child: Text(
                    _currentPage < _slides.length - 1
                        ? translate('Next')
                        : translate('Done'),
                  ),
                ),
              ),
            ),
          ],
        ),
      );
    }
  }

  class _GuideSlide {
    final IconData icon;
    final String title;
    final String body;
    const _GuideSlide(
        {required this.icon, required this.title, required this.body});
  }

  class _SlideView extends StatelessWidget {
    final _GuideSlide slide;
    const _SlideView({required this.slide});

    @override
    Widget build(BuildContext context) {
      return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 24),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(slide.icon, size: 72, color: Theme.of(context).primaryColor),
            const SizedBox(height: 32),
            Text(slide.title,
                style: Theme.of(context).textTheme.headlineSmall,
                textAlign: TextAlign.center),
            const SizedBox(height: 20),
            Text(slide.body,
                style: Theme.of(context).textTheme.bodyLarge,
                textAlign: TextAlign.center),
          ],
        ),
      );
    }
  }

  class _DotsIndicator extends StatelessWidget {
    final int count;
    final int current;
    const _DotsIndicator({required this.count, required this.current});

    @override
    Widget build(BuildContext context) {
      return Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: List.generate(
          count,
          (i) => AnimatedContainer(
            duration: const Duration(milliseconds: 200),
            margin: const EdgeInsets.symmetric(horizontal: 4),
            width: i == current ? 16 : 8,
            height: 8,
            decoration: BoxDecoration(
              color: i == current
                  ? Theme.of(context).primaryColor
                  : Colors.grey.shade400,
              borderRadius: BorderRadius.circular(4),
            ),
          ),
        ),
      );
    }
  }

  void showImportRustdeskGuide(BuildContext context) {
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => const ImportRustdeskGuideModal(),
      fullscreenDialog: true,
    ));
  }
  ```

- [ ] **Step 2: Analyze the new file**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter && flutter analyze lib/mobile/widgets/import_rustdesk_guide.dart 2>&1 | grep -E "error|warning" | head -20
  ```

  Expected: no errors.

- [ ] **Step 3: Commit**

  ```bash
  git add flutter/lib/mobile/widgets/import_rustdesk_guide.dart
  git commit -m "feat(ui): add ImportRustdeskGuideModal walkthrough widget"
  ```

---

## Task 4: Conflict Resolution Dialog

Shows conflicting peer IDs with per-row Override/Skip and global Override All / Skip All / Stop actions.

**Files:**
- Create: `flutter/lib/mobile/widgets/import_conflict_dialog.dart`

- [ ] **Step 1: Create the conflict dialog file**

  Create `flutter/lib/mobile/widgets/import_conflict_dialog.dart`:

  ```dart
  import 'package:flutter/material.dart';
  import '../../common.dart';

  enum _PeerChoice { override, skip }

  enum ConflictResolution { overrideAll, skipAll, stop }

  class ConflictDialogResult {
    // Maps peer id → true (override) / false (skip)
    final Map<String, bool> choices;
    final ConflictResolution? globalAction;

    const ConflictDialogResult({required this.choices, this.globalAction});
  }

  class ImportConflictDialog extends StatefulWidget {
    final List<String> conflictIds;

    const ImportConflictDialog({Key? key, required this.conflictIds})
        : super(key: key);

    @override
    State<ImportConflictDialog> createState() => _ImportConflictDialogState();
  }

  class _ImportConflictDialogState extends State<ImportConflictDialog> {
    late final Map<String, _PeerChoice> _choices;

    @override
    void initState() {
      super.initState();
      _choices = {for (final id in widget.conflictIds) id: _PeerChoice.skip};
    }

    void _applyGlobal(ConflictResolution resolution) {
      Navigator.of(context)
          .pop(ConflictDialogResult(choices: _buildChoiceMap(), globalAction: resolution));
    }

    Map<String, bool> _buildChoiceMap() {
      return _choices.map((id, c) => MapEntry(id, c == _PeerChoice.override));
    }

    @override
    Widget build(BuildContext context) {
      return AlertDialog(
        title: Text(translate('Conflicting Peers')),
        content: SizedBox(
          width: double.maxFinite,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                translate(
                    'The following peers already exist in Tabby. Choose how to handle each:'),
                style: Theme.of(context).textTheme.bodyMedium,
              ),
              const SizedBox(height: 12),
              ConstrainedBox(
                constraints: BoxConstraints(
                    maxHeight: MediaQuery.of(context).size.height * 0.4),
                child: ListView.builder(
                  shrinkWrap: true,
                  itemCount: widget.conflictIds.length,
                  itemBuilder: (context, i) {
                    final id = widget.conflictIds[i];
                    return ListTile(
                      dense: true,
                      title: Text(id,
                          style: const TextStyle(fontFamily: 'monospace')),
                      trailing: ToggleButtons(
                        isSelected: [
                          _choices[id] == _PeerChoice.override,
                          _choices[id] == _PeerChoice.skip,
                        ],
                        onPressed: (index) {
                          setState(() {
                            _choices[id] = index == 0
                                ? _PeerChoice.override
                                : _PeerChoice.skip;
                          });
                        },
                        children: [
                          Padding(
                            padding:
                                const EdgeInsets.symmetric(horizontal: 10),
                            child: Text(translate('Override')),
                          ),
                          Padding(
                            padding:
                                const EdgeInsets.symmetric(horizontal: 10),
                            child: Text(translate('Skip')),
                          ),
                        ],
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => _applyGlobal(ConflictResolution.stop),
            child: Text(translate('Stop'),
                style: const TextStyle(color: Colors.red)),
          ),
          TextButton(
            onPressed: () => _applyGlobal(ConflictResolution.skipAll),
            child: Text(translate('Skip All')),
          ),
          TextButton(
            onPressed: () => _applyGlobal(ConflictResolution.overrideAll),
            child: Text(translate('Override All')),
          ),
          ElevatedButton(
            onPressed: () => Navigator.of(context).pop(
                ConflictDialogResult(choices: _buildChoiceMap())),
            child: Text(translate('Apply')),
          ),
        ],
      );
    }
  }

  Future<ConflictDialogResult?> showImportConflictDialog(
      BuildContext context, List<String> conflictIds) {
    return showDialog<ConflictDialogResult>(
      context: context,
      barrierDismissible: false,
      builder: (_) => ImportConflictDialog(conflictIds: conflictIds),
    );
  }
  ```

- [ ] **Step 2: Analyze the new file**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter && flutter analyze lib/mobile/widgets/import_conflict_dialog.dart 2>&1 | grep -E "error|warning" | head -20
  ```

  Expected: no errors.

- [ ] **Step 3: Commit**

  ```bash
  git add flutter/lib/mobile/widgets/import_conflict_dialog.dart
  git commit -m "feat(ui): add ImportConflictDialog for per-peer conflict resolution"
  ```

---

## Task 5: Import Orchestration Logic

Ties together: file picker → Rust FFI parse → conflict check → execute imports.

**Files:**
- Create: `flutter/lib/mobile/widgets/import_rustdesk.dart`

- [ ] **Step 1: Create the orchestration file**

  Create `flutter/lib/mobile/widgets/import_rustdesk.dart`:

  ```dart
  import 'dart:convert';
  import 'package:file_picker/file_picker.dart';
  import 'package:flutter/material.dart';
  import '../../common.dart';
  import '../../models/platform_model.dart';
  import 'import_conflict_dialog.dart';

  Future<void> runImportFromRustdesk(BuildContext context) async {
    // 1. Pick the RustDesk data/ folder
    final folder = await FilePicker.platform.getDirectoryPath(
      dialogTitle: translate('Select RustDesk data folder'),
    );
    if (folder == null) return; // user cancelled

    // 2. Parse via Rust FFI
    final raw = await bind.mainImportRustdeskData(folder: folder);
    Map<String, dynamic> parsed;
    try {
      parsed = jsonDecode(raw);
    } catch (_) {
      showToast(translate('No RustDesk data found in selected folder'));
      return;
    }

    final List peers = parsed['peers'] ?? [];
    final Map<String, dynamic> serverConfig =
        parsed['server_config'] ?? {};

    if (peers.isEmpty && (serverConfig['id_server'] as String? ?? '').isEmpty) {
      showToast(translate('No RustDesk data found in selected folder'));
      return;
    }

    // 3. Separate new vs conflict peers
    final newPeers = <Map<String, dynamic>>[];
    final conflictPeers = <Map<String, dynamic>>[];

    for (final peer in peers.cast<Map<String, dynamic>>()) {
      final id = peer['id'] as String? ?? '';
      if (id.isEmpty) continue;
      final exists = bind.mainPeerExists(id: id);
      if (exists) {
        conflictPeers.add(peer);
      } else {
        newPeers.add(peer);
      }
    }

    // 4. Resolve conflicts
    Map<String, bool> overrideDecisions = {};
    if (conflictPeers.isNotEmpty && context.mounted) {
      final conflictIds =
          conflictPeers.map((p) => p['id'] as String).toList();
      final result =
          await showImportConflictDialog(context, conflictIds);

      if (result == null) return; // dialog dismissed unexpectedly

      if (result.globalAction == ConflictResolution.stop) return;

      if (result.globalAction == ConflictResolution.overrideAll) {
        overrideDecisions = {for (final id in conflictIds) id: true};
      } else if (result.globalAction == ConflictResolution.skipAll) {
        overrideDecisions = {for (final id in conflictIds) id: false};
      } else {
        overrideDecisions = result.choices;
      }
    }

    // 5. Write peers
    int importedPeers = 0;

    for (final peer in newPeers) {
      await _writePeer(peer);
      importedPeers++;
    }

    for (final peer in conflictPeers) {
      final id = peer['id'] as String;
      if (overrideDecisions[id] == true) {
        await _writePeer(peer);
        importedPeers++;
      }
    }

    // 6. Apply server config
    bool importedServer = false;
    final idServer = serverConfig['id_server'] as String? ?? '';
    if (idServer.isNotEmpty) {
      final sc = ServerConfig(
        idServer: idServer,
        relayServer: serverConfig['relay_server'] as String? ?? '',
        apiServer: serverConfig['api_server'] as String? ?? '',
        key: serverConfig['key'] as String? ?? '',
      );
      importedServer = await setServerConfig(null, null, sc);
    }

    // 7. Show result toast
    if (!context.mounted) return;
    if (importedPeers > 0 && importedServer) {
      showToast(translate('Imported $importedPeers peers and server config'));
    } else if (importedPeers > 0) {
      showToast(translate('Imported $importedPeers peers'));
    } else if (importedServer) {
      showToast(translate('Imported server config'));
    } else {
      showToast(translate('Nothing to import'));
    }
  }

  Future<void> _writePeer(Map<String, dynamic> peer) async {
    await bind.mainImportPeer(
      id: peer['id'] as String? ?? '',
      username: peer['username'] as String? ?? '',
      hostname: peer['hostname'] as String? ?? '',
      platform: peer['platform'] as String? ?? '',
    );
  }
  ```

- [ ] **Step 2: Analyze the new file**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter && flutter analyze lib/mobile/widgets/import_rustdesk.dart 2>&1 | grep -E "error|warning" | head -20
  ```

  Expected: no errors. Fix any missing imports (e.g. `ServerConfig`, `setServerConfig` come from `../../common.dart`).

- [ ] **Step 3: Commit**

  ```bash
  git add flutter/lib/mobile/widgets/import_rustdesk.dart
  git commit -m "feat(dart): add runImportFromRustdesk orchestration logic"
  ```

---

## Task 6: Settings Page — Add the Two Rows

Wire the two new SettingsTile rows into the mobile settings page.

**Files:**
- Modify: `flutter/lib/mobile/pages/settings_page.dart`

- [ ] **Step 1: Add imports to settings_page.dart**

  Open `flutter/lib/mobile/pages/settings_page.dart`. Find the existing imports at the top (around lines 1-20). Add:

  ```dart
  import '../widgets/import_rustdesk.dart';
  import '../widgets/import_rustdesk_guide.dart';
  ```

- [ ] **Step 2: Add the two SettingsTile rows**

  In `settings_page.dart`, find the main settings section tiles list (around line 715 — the `SettingsTile` for `'ID/Relay Server'`). Add the two new tiles immediately **before** the `'ID/Relay Server'` tile:

  ```dart
  SettingsTile(
    title: Text(translate('Import from RustDesk')),
    leading: const Icon(Icons.download),
    onPressed: (context) => runImportFromRustdesk(context),
  ),
  SettingsTile(
    title: Text(translate('How to export from RustDesk')),
    leading: const Icon(Icons.help_outline),
    onPressed: (context) => showImportRustdeskGuide(context),
  ),
  ```

- [ ] **Step 3: Analyze the modified file**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter && flutter analyze lib/mobile/pages/settings_page.dart 2>&1 | grep -E "error|warning" | head -20
  ```

  Expected: no errors.

- [ ] **Step 4: Commit**

  ```bash
  git add flutter/lib/mobile/pages/settings_page.dart
  git commit -m "feat(settings): add Import from RustDesk and guide rows to settings page"
  ```

---

## Task 7: Manual Test Checklist

The Rust FFI bridge cannot be unit tested from Flutter tests without a device. Verify on a real iOS device or simulator.

- [ ] **Step 1: Build and run on iOS simulator**

  ```bash
  cd /Users/ronenmars/Desktop/dev/apps/ios/Tabby/flutter && flutter run --debug 2>&1 | tail -5
  ```

- [ ] **Step 2: Verify Settings UI**
  - Open Settings — confirm "Import from RustDesk" and "How to export from RustDesk" rows appear above "ID/Relay Server"
  - Tap "How to export from RustDesk" — confirm 5-slide modal opens, swipe works, Skip closes, Done closes

- [ ] **Step 3: Test import with a real or mocked data folder**

  Create a test folder with the RustDesk structure:
  ```bash
  mkdir -p /tmp/rustdesk_test/data/peers
  cat > /tmp/rustdesk_test/data/RustDesk2.toml <<'EOF'
  rendezvous_server = "rs-sg.rustdesk.com"

  [options]
  EOF
  ```

  Tap "Import from RustDesk", pick the `data/` folder, verify toast: "Imported server config".

- [ ] **Step 4: Test conflict flow**
  - Ensure at least one peer exists in Tabby
  - Create a matching peer file in the test folder
  - Tap import — verify conflict dialog appears
  - Test each button: Override, Skip, Override All, Skip All, Stop

- [ ] **Step 5: Test empty/invalid folder**
  - Pick an empty folder — verify toast: "No RustDesk data found in selected folder"

- [ ] **Step 6: Commit final verification note**

  ```bash
  git tag import-rustdesk-verified
  ```

---

## Self-Review Against Spec

| Spec Requirement | Covered In |
|---|---|
| Two always-visible rows (import + guide), import first | Task 6 |
| Guide is 5-slide modal with Skip link | Task 3 |
| File picker for folder selection | Task 5 Step 1 |
| Parse `RustDesk2.toml` for server config | Task 1 Step 3 |
| Parse `peers/*.toml` for peer list | Task 1 Step 3 |
| Rust-side decryption (via `PeerConfig::load_from_path`) | Task 1 Step 1 |
| Separate new vs conflict peers | Task 5 Step 1 |
| Conflict dialog: Override, Skip, Override All, Skip All, Stop | Task 4 |
| Stop = zero side effects | Task 5 (writes happen after conflict resolution) |
| Write peers via `mainImportPeer` FFI | Tasks 1, 2, 5 |
| Apply server config via `setServerConfig` | Task 5 |
| Result toast (peers + server / peers only / server only) | Task 5 |
| Invalid folder → error toast | Task 5 |
| Missing `RustDesk2.toml` → peers only | Task 1 (server_config empty) + Task 5 |
| Empty `peers/` → server config only | Task 1 (peers empty) + Task 5 |
