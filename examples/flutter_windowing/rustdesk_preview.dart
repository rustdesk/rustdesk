import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/experimental_windowing.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/screen/desktop_port_forward_screen.dart';
import 'package:flutter_hbb/desktop/widgets/refresh_wrapper.dart';
import 'package:flutter_hbb/desktop/window_scope.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

ExperimentalWindowingBackend? _preview;

bool isOfficialSessionWindow(int windowId) =>
    windowId > 0 && (_preview?.ownsWindow(windowId) ?? false);

Widget? mountWindowingPreview(Widget mainWindow, List<String> arguments) {
  if (!arguments.contains('--experimental-windowing')) return null;
  final preview = ExperimentalWindowingBackend.install(
    keepImplicitMainView: true,
    useOfficialWindow: (arguments) {
      if (arguments == null) return false;
      final payload = jsonDecode(arguments) as Map<String, dynamic>;
      return payload['type'] == WindowType.PortForward.index &&
          payload['isRDP'] != true;
    },
    builder: (window) {
      if (window.windowId == 0) return mainWindow;
      return _PortForwardWindow(key: ValueKey(window.windowId), window: window);
    },
  );
  _preview = preview;
  return preview.root;
}

class _PortForwardWindow extends StatefulWidget {
  const _PortForwardWindow({super.key, required this.window});

  final ExperimentalWindowContext window;

  @override
  State<_PortForwardWindow> createState() => _PortForwardWindowState();
}

class _PortForwardWindowState extends State<_PortForwardWindow> {
  late final StateGlobal windowState;
  late final DesktopPortForwardScreen screen;
  late final Zone windowZone;

  @override
  void initState() {
    super.initState();
    final window = widget.window;
    windowState = StateGlobal.forDesktopWindow(window.windowId);
    windowZone = window.run(() => Zone.current);
    final args = jsonDecode(window.arguments!) as Map<String, dynamic>;
    args['windowId'] = window.windowId;
    screen = DesktopPortForwardScreen(params: args);
    debugPrint('Official Port Forward window mounted: ${window.windowId}');
  }

  @override
  Widget build(BuildContext context) {
    return RefreshWrapper(
      builder: (context) => MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: MyTheme.lightTheme,
        darkTheme: MyTheme.darkTheme,
        themeMode: MyTheme.currentThemeMode(),
        localizationsDelegates: GlobalMaterialLocalizations.delegates,
        supportedLocales: supportedLocales,
        home: DesktopWindowScope(
          windowId: widget.window.windowId,
          windowType: WindowType.PortForward,
          windowState: windowState,
          zone: windowZone,
          nativeTitleBar: true,
          child: screen,
        ),
      ),
    );
  }
}
