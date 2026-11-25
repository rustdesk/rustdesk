import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:desktop_multi_window/experimental_windowing.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/screen/desktop_file_transfer_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_terminal_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_view_camera_screen.dart';
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
  // Cocoa must enable multiview before attaching its first view controller.
  // RustDesk's current nib runner already has a controller at this point.
  if (Platform.isMacOS) return null;
  final preview = ExperimentalWindowingBackend.install(
    keepImplicitMainView: true,
    useOfficialWindow: (arguments) {
      if (arguments == null) return false;
      final payload = jsonDecode(arguments) as Map<String, dynamic>;
      if (payload['type'] == WindowType.FileTransfer.index) return true;
      if (Platform.isWindows &&
          (payload['type'] == WindowType.RemoteDesktop.index ||
              payload['type'] == WindowType.ViewCamera.index)) {
        return true;
      }
      if (Platform.isWindows && payload['type'] == WindowType.Terminal.index) {
        return true;
      }
      return payload['type'] == WindowType.PortForward.index &&
          (Platform.isWindows || payload['isRDP'] != true);
    },
    builder: (window) {
      if (window.windowId == 0) return mainWindow;
      return _SessionWindow(key: ValueKey(window.windowId), window: window);
    },
  );
  _preview = preview;
  return preview.root;
}

class _SessionWindow extends StatefulWidget {
  const _SessionWindow({super.key, required this.window});

  final ExperimentalWindowContext window;

  @override
  State<_SessionWindow> createState() => _SessionWindowState();
}

class _SessionWindowState extends State<_SessionWindow> {
  late final StateGlobal windowState;
  late final Widget screen;
  late final WindowType type;
  late final Zone windowZone;

  @override
  void initState() {
    super.initState();
    final window = widget.window;
    windowState = StateGlobal.forDesktopWindow(window.windowId);
    windowZone = window.run(() => Zone.current);
    final args = jsonDecode(window.arguments!) as Map<String, dynamic>;
    args['windowId'] = window.windowId;
    type = WindowType.values[args['type'] as int];
    screen = switch (type) {
      WindowType.FileTransfer => DesktopFileTransferScreen(params: args),
      WindowType.RemoteDesktop => DesktopRemoteScreen(params: args),
      WindowType.ViewCamera => DesktopViewCameraScreen(params: args),
      WindowType.Terminal => DesktopTerminalScreen(params: args),
      _ => DesktopPortForwardScreen(params: args),
    };
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
          windowType: type,
          windowState: windowState,
          zone: windowZone,
          nativeTitleBar: true,
          child: screen,
        ),
      ),
    );
  }
}
