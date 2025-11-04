import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

class PeerImporter {
  static const _ch = MethodChannel('rustdesk.storage');

  static Future<String?> pickPeersFolder() async {
    return await _ch.invokeMethod<String>('pickTree');
  }

  static Future<List<Map<String, dynamic>>> readTomlsFromTree(String treeUri) async {
    final res = await _ch.invokeMethod<List<dynamic>>('readTomlsFromTree', {
      'treeUri': treeUri,
    });
    return (res ?? [])
        .cast<Map>()
        .map((e) => e.cast<String, dynamic>())
        .toList();
  }

  static Future<Directory> _internalPeersDir() async {
    final docs = await getApplicationDocumentsDirectory(); // .../app_flutter
    final peers = Directory('${docs.path}/peers');
    if (!await peers.exists()) {
      await peers.create(recursive: true);
    }
    return peers;
  }

  static Future<void> _replaceAllTomls(List<Map<String, dynamic>> tomls) async {
    final peersDir = await _internalPeersDir();

    if (await peersDir.exists()) {
      for (final entity in peersDir.listSync()) {
        try { entity.deleteSync(recursive: true); } catch (_) {}
      }
    }

    for (final item in tomls) {
      final name = (item['name'] as String).split('/').last.trim();
      if (!name.toLowerCase().endsWith('.toml')) continue;
      final bytes = base64.decode(item['base64'] as String);
      final file = File('${peersDir.path}/$name');
      await file.writeAsBytes(bytes, flush: true);
    }
  }

  static Future<void> importPeersFromSafFolder() async {
    final tree = await pickPeersFolder();
    if (tree == null) return;

    final tomls = await readTomlsFromTree(tree);
    if (tomls.isEmpty) {
      throw 'La carpeta elegida no contiene archivos .toml';
    }

    await _replaceAllTomls(tomls);
  }
}
