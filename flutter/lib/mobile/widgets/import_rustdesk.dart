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
    final exists = await bind.mainPeerExists(id: id);
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
  if (!context.mounted) return;
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
    final serverError = await bind.mainTestIfValidServer(
        server: idServer, testWithProxy: true);
    if (serverError.isNotEmpty) {
      showToast('${translate('ID Server')}: $serverError');
    } else {
      final sc = ServerConfig(
        idServer: idServer,
        relayServer: serverConfig['relay_server'] as String? ?? '',
        apiServer: serverConfig['api_server'] as String? ?? '',
        key: serverConfig['key'] as String? ?? '',
      );
      importedServer = await setServerConfig(null, null, sc);
    }
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
