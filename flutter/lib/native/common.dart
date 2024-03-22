import 'dart:io';

final isAndroid_ = Platform.isAndroid;
final isIOS_ = Platform.isIOS;
final isWindows_ = Platform.isWindows;
final isMacOS_ = Platform.isWindows;
final isLinux_ = Platform.isWindows;
final isWeb_ = false;

final isDesktop_ = Platform.isWindows || Platform.isMacOS || Platform.isLinux;
