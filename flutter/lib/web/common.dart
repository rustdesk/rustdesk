import 'dart:js' as js;

final isAndroid_ = false;
final isIOS_ = false;
final isWindows_ = false;
final isMacOS_ = false;
final isLinux_ = false;
final isWeb_ = true;
final isWebDesktop_ = !js.context.callMethod('isMobile');

final isDesktop_ = false;

String get screenInfo_ => js.context.callMethod('getByName', ['screen_info']);
