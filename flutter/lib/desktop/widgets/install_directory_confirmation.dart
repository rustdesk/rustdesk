import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:path/path.dart' as path;

const _nonemptyDirectoryWarning = 'install-directory-not-empty-tip';

Future<bool> confirmInstallDirectory(BuildContext context,
    {required String targetPath}) async {
  try {
    final needsWarning = await _needsDirectoryWarning(targetPath);
    if (!context.mounted) return false;
    if (!needsWarning) return true;
    return await _showDirectoryDialog(
        context, '${translate(_nonemptyDirectoryWarning)}\n$targetPath');
  } on FileSystemException catch (error) {
    debugPrint('Failed to inspect installation directory: $error');
    if (context.mounted) {
      await _showDirectoryDialog(context, error.toString(), isError: true);
    }
    return false;
  }
}

Future<bool> _needsDirectoryWarning(String directoryPath) async {
  if (bind.mainIsInstalled() &&
      path.equals(directoryPath, bind.installInstallPath())) {
    return false;
  }
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

class InstallDirectoryWarning extends StatefulWidget {
  InstallDirectoryWarning({required this.directoryPath})
      : super(key: ValueKey(directoryPath));

  final String directoryPath;

  @override
  State<InstallDirectoryWarning> createState() =>
      _InstallDirectoryWarningState();
}

class _InstallDirectoryWarningState extends State<InstallDirectoryWarning> {
  static const _spacing = 8.0;
  late final Future<bool> _needsWarning =
      _needsDirectoryWarning(widget.directoryPath);

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<bool>(
      future: _needsWarning,
      builder: (context, snapshot) {
        if (!snapshot.hasError && snapshot.data != true) {
          return const SizedBox.shrink();
        }
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: _spacing),
          child: Text(
              snapshot.hasError
                  ? '${translate('Error')}: ${snapshot.error}'
                  : translate(_nonemptyDirectoryWarning),
              style: TextStyle(
                  color: snapshot.hasError
                      ? Theme.of(context).colorScheme.error
                      : Colors.orange.shade700)),
        );
      },
    );
  }
}
