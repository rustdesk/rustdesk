import 'dart:js' as js;
import 'dart:html' as html;

final isAndroid_ = false;
final isIOS_ = false;
final isWindows_ = false;
final isMacOS_ = false;
final isLinux_ = false;
final isWeb_ = true;
final isWebDesktop_ = !js.context.callMethod('isMobile');

final isDesktop_ = false;

String get screenInfo_ => js.context.callMethod('getByName', ['screen_info']);

final _userAgent = html.window.navigator.userAgent.toLowerCase();

final isWebOnWindows_ = _userAgent.contains('win');
final isWebOnLinux_ = _userAgent.contains('linux');
final isWebOnMacOS_ = _userAgent.contains('mac');
