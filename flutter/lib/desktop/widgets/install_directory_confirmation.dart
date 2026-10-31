import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:path/path.dart' as path;

Future<bool> confirmInstallDirectory(BuildContext context,
    {required String existingPath, required String targetPath}) async {
  try {
    final warnings = await _directoryWarnings(existingPath, targetPath);
    if (!context.mounted) return false;
    if (warnings.isEmpty) return true;
    return await _showDirectoryDialog(context, warnings.join('\n\n'));
  } on FileSystemException catch (error) {
    debugPrint('Failed to inspect installation directory: $error');
    if (context.mounted) {
      await _showDirectoryDialog(context, error.toString(), isError: true);
    }
    return false;
  }
}

Future<List<String>> _directoryWarnings(
    String existingPath, String targetPath) async {
  final deletesContents = await _containsFiles(existingPath);
  final overwritesContents = !path.equals(existingPath, targetPath) &&
      await _containsFiles(targetPath);
  return [
    if (deletesContents)
      '${translate('Installation will delete this folder and all its contents:')}\n$existingPath',
    if (overwritesContents)
      '${translate('Installation may overwrite files with the same name in this folder:')}\n$targetPath',
  ];
}

Future<bool> _containsFiles(String directoryPath) async {
  final directory = Directory(directoryPath);
  return await directory.exists() &&
      !await directory.list(followLinks: false).isEmpty;
}

Future<bool> _showDirectoryDialog(BuildContext context, String message,
    {bool isError = false}) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      scrollable: true,
      title: Text(translate(isError ? 'Error' : 'Warning')),
      content: SelectableText(message),
      actions: [
        dialogButton(isError ? 'OK' : 'Cancel',
            isOutline: !isError,
            onPressed: () => Navigator.of(dialogContext).pop(false)),
        if (!isError)
          dialogButton('Continue',
              onPressed: () => Navigator.of(dialogContext).pop(true)),
      ],
    ),
  );
  return confirmed == true;
}
