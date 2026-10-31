import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:path/path.dart' as path;

const _nonemptyDirectoryWarning = 'The selected folder is not empty. '
    'Installation may delete existing data. '
    'Consider choosing a different folder.';

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
  final isSameDirectory = path.equals(existingPath, targetPath);
  final deletesContents = await _containsFiles(existingPath);
  final overwritesContents =
      !isSameDirectory && await _containsFiles(targetPath);
  final existingWarning = isSameDirectory
      ? _nonemptyDirectoryWarning
      : 'Installation will delete this folder and all its contents:';
  return [
    if (deletesContents) '${translate(existingWarning)}\n$existingPath',
    if (overwritesContents)
      '${translate(_nonemptyDirectoryWarning)}\n$targetPath',
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
  late final Future<bool> _hasContents = _containsFiles(widget.directoryPath);

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<bool>(
      future: _hasContents,
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
