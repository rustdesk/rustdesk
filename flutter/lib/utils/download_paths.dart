import 'dart:io';

import 'package:path/path.dart' as path;

class DownloadPaths {
  static Future<void> ensureNoLinks(String root, String destination) async {
    if (!path.isWithin(root, destination)) {
      throw FormatException('Invalid download directory: $destination');
    }
    var current = root;
    for (final name in path.split(path.relative(destination, from: root))) {
      current = path.join(current, name);
      if (await FileSystemEntity.type(current, followLinks: false) ==
          FileSystemEntityType.link) {
        throw FormatException('Download directory contains a link: $current');
      }
    }
  }

  static bool validName(String name, {required bool windows}) {
    if (name.isEmpty || name == '.' || name == '..' || name.contains('\x00')) {
      return false;
    }
    if (name.contains('/')) return false;
    return !windows ||
        (!name.contains(RegExp(r'[<>:"\\|?*]')) &&
            !name.endsWith('.') &&
            !name.endsWith(' '));
  }

  static String mapDirectory(
      String remoteRoot, String remotePath, String localRoot,
      {required bool remoteWindows, required bool localWindows}) {
    final remote = path.Context(
        style: remoteWindows ? path.Style.windows : path.Style.posix);
    final local = path.Context(
        style: localWindows ? path.Style.windows : path.Style.posix);
    if (remotePath.contains('\x00') ||
        remote.split(remotePath).contains('..') ||
        (!remote.equals(remoteRoot, remotePath) &&
            !remote.isWithin(remoteRoot, remotePath))) {
      throw FormatException('Invalid download directory: $remotePath');
    }
    final relative = remote.relative(remotePath, from: remoteRoot);
    if (relative == '.') return localRoot;
    final names = remote.split(relative);
    if (names.any((name) => !validName(name, windows: localWindows))) {
      throw FormatException('Invalid download directory: $remotePath');
    }
    return local.joinAll([localRoot, ...names]);
  }
}
