import 'dart:convert';

import '../common.dart';

class AndroidDocuments {
  static const root = '/@rustdesk/';

  static bool isPath(String path) => path.startsWith(root);

  static bool isRoot(String path) =>
      RegExp(r'^/@rustdesk/tree/[^/]+/[^/]+$').hasMatch(path);

  static String displayPath(String path) {
    if (!isPath(path)) return path;
    return path.replaceFirst(RegExp(r'^/@rustdesk/tree/[^/]+/'), '');
  }

  static Future<Map<String, dynamic>?> selectFolder({bool share = false}) async {
    gFFI.ffiModel.beginAndroidDocumentPicker();
    try {
      final result = await gFFI.invokeMethodWithResult<String>(
          'document_folder', {'share': share});
      return result == null ? null : jsonDecode(result) as Map<String, dynamic>;
    } finally {
      gFFI.ffiModel.endAndroidDocumentPicker();
    }
  }

  static Future<String> workspace({bool share = false}) async {
    final path = await gFFI
        .invokeMethodWithResult<String>('document_workspace', {'share': share});
    if (path == null || path.isEmpty) {
      throw StateError('App workspace is unavailable');
    }
    return path;
  }

  static Future<bool> requireLogFolder() async {
    gFFI.ffiModel.beginAndroidDocumentPicker();
    try {
      final ready = await gFFI
          .invokeMethodWithResult<bool>('document_require_log_folder');
      if (ready == null) {
        throw StateError('Log directory setup did not complete');
      }
      return ready;
    } catch (error) {
      showToast(error.toString());
      return false;
    } finally {
      gFFI.ffiModel.endAndroidDocumentPicker();
    }
  }
}
