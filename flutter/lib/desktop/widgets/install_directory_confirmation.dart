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
  const InstallDirectoryWarning({super.key, required this.directoryPath});

  final String directoryPath;

  @override
  State<InstallDirectoryWarning> createState() =>
      _InstallDirectoryWarningState();
}

class _InstallDirectoryWarningState extends State<InstallDirectoryWarning> {
  static const _topSpacing = 8.0;
  static const _bottomSpacing = 8.0;
  late Future<bool> _hasContents;

  @override
  void initState() {
    super.initState();
    _hasContents = _containsFiles(widget.directoryPath);
  }

  @override
  void didUpdateWidget(covariant InstallDirectoryWarning oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.directoryPath != widget.directoryPath) {
      _hasContents = _containsFiles(widget.directoryPath);
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<bool>(
      future: _hasContents,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const SizedBox.shrink();
        }
        final String message;
        if (snapshot.hasError) {
          message = '${translate('Error')}: ${snapshot.error}';
        } else if (snapshot.data == true) {
          message = translate(_nonemptyDirectoryWarning);
        } else {
          return const SizedBox.shrink();
        }
        return Padding(
          padding:
              const EdgeInsets.only(top: _topSpacing, bottom: _bottomSpacing),
          child: Text(message,
              style: TextStyle(
                  color: snapshot.hasError
                      ? Theme.of(context).colorScheme.error
                      : Colors.orange.shade700)),
        );
      },
    );
  }
}
