import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

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
      final ready = await gFFI.invokeMethodWithResult<bool>(
          'document_require_log_folder', {'checkOnly': true});
      if (ready == null) {
        throw StateError('Log directory setup did not complete');
      }
      if (ready) return true;
      if (!await _confirmLogFolder()) return false;
      final selected = await gFFI
          .invokeMethodWithResult<bool>('document_require_log_folder');
      if (selected == null) {
        throw StateError('Log directory setup did not complete');
      }
      return selected;
    } catch (error) {
      debugPrint('Log directory setup failed');
      await gFFI.dialogManager.show<void>(
        (_, close, __) => CustomAlertDialog(
          title: Text(translate('Error')),
          content: Text(error is PlatformException
              ? translate(error.message ?? 'Failed')
              : error.toString()),
          actions: [
            TextButton(onPressed: close, child: Text(translate('OK'))),
          ],
          onSubmit: close,
          onCancel: close,
        ),
        backDismiss: true,
        forceGlobal: true,
      );
      rethrow;
    } finally {
      gFFI.ffiModel.endAndroidDocumentPicker();
    }
  }

  static Future<bool> _confirmLogFolder() async =>
      await gFFI.dialogManager.show<bool>(
        (_, close, __) => CustomAlertDialog(
          title: Text(translate('Permissions')),
          content: Text(
              '${translate('android-logs-folder-tip')}\n'),
          actions: [
            TextButton(
              onPressed: () => close(true),
              child: Text(translate('Choose folder')),
            ),
          ],
          onCancel: () => close(false),
        ),
        backDismiss: true,
        forceGlobal: true,
      ) ==
      true;
}
