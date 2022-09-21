import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui';

import 'package:back_button_interceptor/back_button_interceptor.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:window_manager/window_manager.dart';

import 'common/widgets/overlay.dart';
import 'models/model.dart';
import 'models/platform_model.dart';

final globalKey = GlobalKey<NavigatorState>();
final navigationBarKey = GlobalKey();

final isAndroid = Platform.isAndroid;
final isIOS = Platform.isIOS;
final isDesktop = Platform.isWindows || Platform.isMacOS || Platform.isLinux;
var isWeb = false;
var isWebDesktop = false;
var version = "";
int androidVersion = 0;
const windowPrefix = "wm_";
DesktopType? desktopType;

typedef F = String Function(String);
typedef FMethod = String Function(String, dynamic);

typedef StreamEventHandler = Future<void> Function(Map<String, dynamic>);

late final iconKeyboard = MemoryImage(Uint8List.fromList(base64Decode(
    "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAgVBMVEUAAAD///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////9d3yJTAAAAKnRSTlMA0Gd/0y8ILZgbJffDPUwV2nvzt+TMqZxyU7CMb1pYQyzsvKunkXE4AwJnNC24AAAA+0lEQVQ4y83O2U7DMBCF4ZMxk9rZk26kpQs7nPd/QJy4EiLbLf01N5Y/2YP/qxDFQvGB5NPC/ZpVnfJx4b5xyGfF95rkHvNCWH1u+N6J6T0sC7gqRy8uGPfBLEbozPXUjlkQKwGaFPNizwQbwkx0TDvhCii34ExZCSQVBdzIOEOyeclSHgBGXkpeygXSQgStACtWx4Z8rr8COHOvfEP/IbbsQAToFUAAV1M408IIjIGYAPoCSNRP7DQutfQTqxuAiH7UUg1FaJR2AGrrx52sK2ye28LZ0wBAEyR6y8X+NADhm1B4fgiiHXbRrTrxpwEY9RdM9wsepnvFHfUDwYEeiwAJr/gAAAAASUVORK5CYII=")));
late final iconClipboard = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAjVBMVEUAAAD///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////8DizOFAAAALnRSTlMAnIsyZy8YZF3NSAuabRL34cq6trCScyZ4qI9CQDwV+fPl2tnTwzkeB+m/pIFK/Xx0ewAAAQlJREFUOMudktduhDAQRWep69iY3tle0+7/f16Qg7MsJUQ5Dwh8jzRzhemJPIaf3GiW7eFQfOwDPp1ek/iMnKgBi5PrhJAhZAa1lCxE9pw5KWMswOMAQXuQOvqTB7tLFJ36wimKLrufZTzUaoRtdthqRA2vEwS+tR4qguiElRKk1YMrYfUQRkwLmwVBYDMvJKF8R0o3V2MOhNrfo+hXSYYjPn1L/S+n438t8gWh+q1F+cYFBMm1Jh8Ia7y2OWXQxMMRLqr2eTc1crSD84cWfEGwYM4LlaACEee2ZjsQXJxR3qmYb+GpC8ZfNM5oh3yxxbxgQE7lEkb3ZvvH1BiRHn1bu02ICcKGWr4AudUkyYxmvywAAAAASUVORK5CYII=')));
late final iconAudio = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAk1BMVEUAAAD////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////ROyVeAAAAMHRSTlMAgfz08DDqCAThvraZjEcoGA751JxzbGdfTRP25NrIpaGTcEM+HAvMuKinhXhWNx9Yzm/gAAABFUlEQVQ4y82S2XLCMAxFheMsQNghCQFalkL39vz/11V4GpNk0r629+Va1pmxPFfyh1ravOP2Y1ydJmBO0lYP3r+PyQ62s2Y7fgF6VRXOYdToT++ogIuoVhCUtX7YpwJG3F8f6V8rr3WABwwUahlEvr8y3IBniGKdKYBQ5OGQpukQakBpIVcfwptIhJcf8hWGakdndAAhBInIGHbdQGJg6jjbDUgEE5EpmB+AAM4uj6gb+AQT6wdhITLvAHJ4VCtgoAlG1tpNA0gWON/f4ioHdSADc1bfgt+PZFkDlD6ojWF+kVoaHlhvFjPHuVRrefohY1GdcFm1N8JvwEyrJ/X2Th2rIoVgIi3Fo6Xf0z5k8psKu5f/oi+nHjjI92o36AAAAABJRU5ErkJggg==')));
late final iconFile = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAGAAAABgCAMAAADVRocKAAAAUVBMVEUAAAD///////////////////////////////////////////////////////////////////////////////////////////////////////8IN+deAAAAGnRSTlMAH+CAESEN8jyZkcIb5N/ONy3vmHhmiGjUm7UwS+YAAAHZSURBVGje7dnbboMwDIBhBwgQoFAO7Ta//4NOqCAXYZQstatq4r+r5ubrgQSpg8iyC4ZURa+PlIpQYGiwrzyeHtYZjAL8T05O4H8BbbKvFgRa4NoBU8pXeYEkDDgaaLQBcwJrmeErJQB/7wes3QBWGnCIX0+AQycL1PO6BMwPa0nA4ZxbgTvOjUYMGPHRnZkQAY4mxPZBjmy53E7ukSkFKYB/D4XsWZQx64sCeYebOogGsoOBYvv6/UCb8F0IOBZ0TlP6lEYdANY350AJqB9/qPVuOI5evw4A1hgLigAlepnyxW80bcCcwN++A2s82Vcu02ta+ceq9BoL5KGTTRwQPlpqA3gCnwWU2kCDgeWRQPj2jAPCDxgCMjhI6uZnToDpvd/BJeFrJQB/fsAa02gCt3mi1wNuy8GgBNDZlysBNNSrADVSjcJl6vCpUn6jOdx0kz0q6PMhQRa4465SFKhx35cgUCBTwj2/NHwZAb71qR8GEP2H1XcmAtBPTEO67GP6FUUAIKGABbDLQ0EArhN2sAIGesRO+iyy+RMAjckVTlMCKFVAbh/4Af9OPgG61SkDVco3BQGT3GXaDAnTIAcYZDuBTwGsAGDxuBFeAQqIqwoFMlAVLrHr/wId5MPt0nilGgAAAABJRU5ErkJggg==')));
late final iconRestart = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAACXBIWXMAAB7BAAAewQHDaVRTAAAAGXRFWHRTb2Z0d2FyZQB3d3cuaW5rc2NhcGUub3Jnm+48GgAAAbhJREFUWIXVlrFqFGEUhb+7UYxaWCQKlrKKxaZSQVGDJih2tj6MD2DnMwiWvoAIRnENIpZiYxEro6IooiS7SPwsMgNLkk3mjmYmnmb45/73nMNwz/x/qH3gMu2gH6rAU+Blw+Lngau4jpmGxVF7qp1iPWjaQKnZ2WnXbuP/NqAeUPc3ZkA9XDwvqc+BVWCgPlJ7tRwUKThZce819b46VH+pfXVRXVO/q2cSul3VOgZUl0ejq86r39TXI8mqZKDuDEwCw3IREQvAbWAGmMsQZQ0sAl3gHPB1Q+0e8BuYzRDuy2yOiFVgaUxtRf0ETGc4syk4rc6PqU0Cx9j8Zf6dAeAK8Fi9sUXtFjABvEgxJlNwRP2svlNPjbw/q35U36oTFbnyMSwabxb/gB/qA3VBHagrauV7RW0DRfP1IvMlXqkXkhz1DYyQTKtHa/Z2VVMx3IiI+PI3/bCHjuOpFrSnAMpL6QfgTcMGesDx0kBr2BMzsNyi/vtQu8CJlgwsRbZDnWP90NkKaxHxJMOXMqAeAn5u0ydwMCKGY+qbkB3C2W3EKWoXk5zVoHbUZ+6Mh7tl4G4F8RJ3qvL+AfV3r5Vdpj70AAAAAElFTkSuQmCC')));

enum DesktopType {
  main,
  remote,
  fileTransfer,
  cm,
  portForward,
}

class IconFont {
  static const _family1 = 'Tabbar';
  static const _family2 = 'PeerSearchbar';
  IconFont._();

  static const IconData max = IconData(0xe606, fontFamily: _family1);
  static const IconData restore = IconData(0xe607, fontFamily: _family1);
  static const IconData close = IconData(0xe668, fontFamily: _family1);
  static const IconData min = IconData(0xe609, fontFamily: _family1);
  static const IconData add = IconData(0xe664, fontFamily: _family1);
  static const IconData menu = IconData(0xe628, fontFamily: _family1);
  static const IconData search = IconData(0xe6a4, fontFamily: _family2);
  static const IconData round_close = IconData(0xe6ed, fontFamily: _family2);
}

class ColorThemeExtension extends ThemeExtension<ColorThemeExtension> {
  const ColorThemeExtension({
    required this.bg,
    required this.grayBg,
    required this.text,
    required this.lightText,
    required this.lighterText,
    required this.placeholder,
    required this.border,
  });

  final Color? bg;
  final Color? grayBg;
  final Color? text;
  final Color? lightText;
  final Color? lighterText;
  final Color? placeholder;
  final Color? border;

  static const light = ColorThemeExtension(
    bg: Color(0xFFFFFFFF),
    grayBg: Color(0xFFEEEEEE),
    text: Color(0xFF222222),
    lightText: Color(0xFF666666),
    lighterText: Color(0xFF888888),
    placeholder: Color(0xFFAAAAAA),
    border: Color(0xFFCCCCCC),
  );

  static const dark = ColorThemeExtension(
    bg: Color(0xFF252525),
    grayBg: Color(0xFF141414),
    text: Color(0xFFFFFFFF),
    lightText: Color(0xFF999999),
    lighterText: Color(0xFF777777),
    placeholder: Color(0xFF555555),
    border: Color(0xFF555555),
  );

  @override
  ThemeExtension<ColorThemeExtension> copyWith(
      {Color? bg,
      Color? grayBg,
      Color? text,
      Color? lightText,
      Color? lighterText,
      Color? placeholder,
      Color? border}) {
    return ColorThemeExtension(
      bg: bg ?? this.bg,
      grayBg: grayBg ?? this.grayBg,
      text: text ?? this.text,
      lightText: lightText ?? this.lightText,
      lighterText: lighterText ?? this.lighterText,
      placeholder: placeholder ?? this.placeholder,
      border: border ?? this.border,
    );
  }

  @override
  ThemeExtension<ColorThemeExtension> lerp(
      ThemeExtension<ColorThemeExtension>? other, double t) {
    if (other is! ColorThemeExtension) {
      return this;
    }
    return ColorThemeExtension(
      bg: Color.lerp(bg, other.bg, t),
      grayBg: Color.lerp(grayBg, other.grayBg, t),
      text: Color.lerp(text, other.text, t),
      lightText: Color.lerp(lightText, other.lightText, t),
      lighterText: Color.lerp(lighterText, other.lighterText, t),
      placeholder: Color.lerp(placeholder, other.placeholder, t),
      border: Color.lerp(border, other.border, t),
    );
  }
}

class MyTheme {
  MyTheme._();

  static const Color grayBg = Color(0xFFEEEEEE);
  static const Color white = Color(0xFFFFFFFF);
  static const Color accent = Color(0xFF0071FF);
  static const Color accent50 = Color(0x770071FF);
  static const Color accent80 = Color(0xAA0071FF);
  static const Color canvasColor = Color(0xFF212121);
  static const Color border = Color(0xFFCCCCCC);
  static const Color idColor = Color(0xFF00B6F0);
  static const Color darkGray = Color(0xFFB9BABC);
  static const Color cmIdColor = Color(0xFF21790B);
  static const Color dark = Colors.black87;
  static const Color button = Color(0xFF2C8CFF);
  static const Color hoverBorder = Color(0xFF999999);

  static ThemeData lightTheme = ThemeData(
    brightness: Brightness.light,
    primarySwatch: Colors.blue,
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: const TabBarTheme(
      labelColor: Colors.black87,
    ),
    splashColor: Colors.transparent,
    highlightColor: Colors.transparent,
  ).copyWith(
    extensions: <ThemeExtension<dynamic>>[
      ColorThemeExtension.light,
      TabbarTheme.light,
    ],
  );
  static ThemeData darkTheme = ThemeData(
    brightness: Brightness.dark,
    primarySwatch: Colors.blue,
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: const TabBarTheme(
      labelColor: Colors.white70,
    ),
    splashColor: Colors.transparent,
    highlightColor: Colors.transparent,
  ).copyWith(
    extensions: <ThemeExtension<dynamic>>[
      ColorThemeExtension.dark,
      TabbarTheme.dark,
    ],
  );

  static ThemeMode getThemeModePreference() {
    return themeModeFromString(
        Get.find<SharedPreferences>().getString("themeMode") ?? "");
  }

  static void changeDarkMode(ThemeMode mode) {
    final preference = getThemeModePreference();
    if (preference != mode) {
      if (mode == ThemeMode.system) {
        Get.find<SharedPreferences>().setString("themeMode", "");
      } else {
        Get.find<SharedPreferences>()
            .setString("themeMode", mode.toShortString());
      }
      Get.changeThemeMode(mode);
      if (desktopType == DesktopType.main) {
        bind.mainChangeTheme(dark: currentThemeMode().toShortString());
      }
    }
  }

  static ThemeMode currentThemeMode() {
    final preference = getThemeModePreference();
    if (preference == ThemeMode.system) {
      if (WidgetsBinding.instance.platformDispatcher.platformBrightness ==
          Brightness.light) {
        return ThemeMode.light;
      } else {
        return ThemeMode.dark;
      }
    } else {
      return preference;
    }
  }

  static ColorThemeExtension color(BuildContext context) {
    return Theme.of(context).extension<ColorThemeExtension>()!;
  }

  static TabbarTheme tabbar(BuildContext context) {
    return Theme.of(context).extension<TabbarTheme>()!;
  }

  static ThemeMode themeModeFromString(String v) {
    switch (v) {
      case "light":
        return ThemeMode.light;
      case "dark":
        return ThemeMode.dark;
      default:
        return ThemeMode.system;
    }
  }
}

extension ParseToString on ThemeMode {
  String toShortString() {
    return toString().split('.').last;
  }
}

final ButtonStyle flatButtonStyle = TextButton.styleFrom(
  minimumSize: Size(0, 36),
  padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 10.0),
  shape: const RoundedRectangleBorder(
    borderRadius: BorderRadius.all(Radius.circular(2.0)),
  ),
);

List<Locale> supportedLocales = const [
  // specify CN/TW to fix CJK issue in flutter
  Locale('zh', 'CN'),
  Locale('zh', 'TW'),
  Locale('zh', 'SG'),
  Locale('fr'),
  Locale('de'),
  Locale('it'),
  Locale('ja'),
  Locale('cs'),
  Locale('pl'),
  Locale('ko'),
  Locale('hu'),
  Locale('pt'),
  Locale('ru'),
  Locale('sk'),
  Locale('id'),
  Locale('da'),
  Locale('eo'),
  Locale('tr'),
  Locale('vi'),
  Locale('pl'),
  Locale('kz'),
  Locale('en', 'US'),
];

String formatDurationToTime(Duration duration) {
  var totalTime = duration.inSeconds;
  final secs = totalTime % 60;
  totalTime = (totalTime - secs) ~/ 60;
  final mins = totalTime % 60;
  totalTime = (totalTime - mins) ~/ 60;
  return "${totalTime.toString().padLeft(2, "0")}:${mins.toString().padLeft(2, "0")}:${secs.toString().padLeft(2, "0")}";
}

closeConnection({String? id}) {
  if (isAndroid || isIOS) {
    Navigator.popUntil(globalKey.currentContext!, ModalRoute.withName("/"));
  } else {
    final controller = Get.find<DesktopTabController>();
    controller.closeBy(id);
  }
}

void window_on_top(int? id) {
  if (id == null) {
    // main window
    windowManager.restore();
    windowManager.show();
    windowManager.focus();
  } else {
    WindowController.fromWindowId(id)
      ..focus()
      ..show();
  }
}

typedef DialogBuilder = CustomAlertDialog Function(
    StateSetter setState, void Function([dynamic]) close);

class Dialog<T> {
  OverlayEntry? entry;
  Completer<T?> completer = Completer<T?>();

  Dialog();

  void complete(T? res) {
    try {
      if (!completer.isCompleted) {
        completer.complete(res);
      }
    } catch (e) {
      debugPrint("Dialog complete catch error: $e");
    } finally {
      entry?.remove();
    }
  }
}

class OverlayDialogManager {
  OverlayState? _overlayState;
  final Map<String, Dialog> _dialogs = {};
  int _tagCount = 0;

  OverlayEntry? _mobileActionsOverlayEntry;

  /// By default OverlayDialogManager use global overlay
  OverlayDialogManager() {
    _overlayState = globalKey.currentState?.overlay;
  }

  void setOverlayState(OverlayState? overlayState) {
    _overlayState = overlayState;
  }

  void dismissAll() {
    _dialogs.forEach((key, value) {
      value.complete(null);
      BackButtonInterceptor.removeByName(key);
    });
    _dialogs.clear();
  }

  void dismissByTag(String tag) {
    _dialogs[tag]?.complete(null);
    _dialogs.remove(tag);
    BackButtonInterceptor.removeByName(tag);
  }

  Future<T?> show<T>(DialogBuilder builder,
      {bool clickMaskDismiss = false,
      bool backDismiss = false,
      String? tag,
      bool useAnimation = true,
      bool forceGlobal = false}) {
    final overlayState =
        forceGlobal ? globalKey.currentState?.overlay : _overlayState;

    if (overlayState == null) {
      return Future.error(
          "[OverlayDialogManager] Failed to show dialog, _overlayState is null, call [setOverlayState] first");
    }

    final _tag;
    if (tag != null) {
      _tag = tag;
    } else {
      _tag = _tagCount.toString();
      _tagCount++;
    }

    final dialog = Dialog<T>();
    _dialogs[_tag] = dialog;

    final close = ([res]) {
      _dialogs.remove(_tag);
      dialog.complete(res);
      BackButtonInterceptor.removeByName(_tag);
    };
    dialog.entry = OverlayEntry(builder: (_) {
      bool innerClicked = false;
      return Listener(
          onPointerUp: (_) {
            if (!innerClicked && clickMaskDismiss) {
              close();
            }
            innerClicked = false;
          },
          child: Container(
              color: Colors.black12,
              child: StatefulBuilder(builder: (context, setState) {
                return Listener(
                  onPointerUp: (_) => innerClicked = true,
                  child: builder(setState, close),
                );
              })));
    });
    overlayState.insert(dialog.entry!);
    BackButtonInterceptor.add((stopDefaultButtonEvent, routeInfo) {
      if (backDismiss) {
        close();
      }
      return true;
    }, name: _tag);
    return dialog.completer.future;
  }

  void showLoading(String text,
      {bool clickMaskDismiss = false,
      bool showCancel = true,
      VoidCallback? onCancel}) {
    show((setState, close) {
      cancel() {
        dismissAll();
        if (onCancel != null) {
          onCancel();
        }
      }

      return CustomAlertDialog(
        content: Container(
            constraints: const BoxConstraints(maxWidth: 240),
            child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const SizedBox(height: 30),
                  const Center(child: CircularProgressIndicator()),
                  const SizedBox(height: 20),
                  Center(
                      child: Text(translate(text),
                          style: const TextStyle(fontSize: 15))),
                  const SizedBox(height: 20),
                  Offstage(
                      offstage: !showCancel,
                      child: Center(
                          child: TextButton(
                              style: flatButtonStyle,
                              onPressed: cancel,
                              child: Text(translate('Cancel'),
                                  style:
                                      const TextStyle(color: MyTheme.accent)))))
                ])),
        onCancel: showCancel ? cancel : null,
      );
    });
  }

  void resetMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry == null) return;
    hideMobileActionsOverlay();
    showMobileActionsOverlay(ffi: ffi);
  }

  void showMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry != null) return;
    if (_overlayState == null) return;

    // compute overlay position
    final screenW = MediaQuery.of(globalKey.currentContext!).size.width;
    final screenH = MediaQuery.of(globalKey.currentContext!).size.height;
    const double overlayW = 200;
    const double overlayH = 45;
    final left = (screenW - overlayW) / 2;
    final top = screenH - overlayH - 80;

    final overlay = OverlayEntry(builder: (context) {
      final session = ffi ?? gFFI;
      return DraggableMobileActions(
        position: Offset(left, top),
        width: overlayW,
        height: overlayH,
        onBackPressed: () => session.tap(MouseButtons.right),
        onHomePressed: () => session.tap(MouseButtons.wheel),
        onRecentPressed: () async {
          session.sendMouse('down', MouseButtons.wheel);
          await Future.delayed(const Duration(milliseconds: 500));
          session.sendMouse('up', MouseButtons.wheel);
        },
        onHidePressed: () => hideMobileActionsOverlay(),
      );
    });
    _overlayState!.insert(overlay);
    _mobileActionsOverlayEntry = overlay;
  }

  void hideMobileActionsOverlay() {
    if (_mobileActionsOverlayEntry != null) {
      _mobileActionsOverlayEntry!.remove();
      _mobileActionsOverlayEntry = null;
      return;
    }
  }

  void toggleMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry == null) {
      showMobileActionsOverlay(ffi: ffi);
    } else {
      hideMobileActionsOverlay();
    }
  }
}

void showToast(String text, {Duration timeout = const Duration(seconds: 2)}) {
  final overlayState = globalKey.currentState?.overlay;
  if (overlayState == null) return;
  final entry = OverlayEntry(builder: (_) {
    return IgnorePointer(
        child: Align(
            alignment: const Alignment(0.0, 0.8),
            child: Container(
              decoration: BoxDecoration(
                color: Colors.black.withOpacity(0.6),
                borderRadius: const BorderRadius.all(
                  Radius.circular(20),
                ),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 5),
              child: Text(
                text,
                style: const TextStyle(
                    decoration: TextDecoration.none,
                    fontWeight: FontWeight.w300,
                    fontSize: 18,
                    color: Colors.white),
              ),
            )));
  });
  overlayState.insert(entry);
  Future.delayed(timeout, () {
    entry.remove();
  });
}

class CustomAlertDialog extends StatelessWidget {
  const CustomAlertDialog(
      {Key? key,
      this.title,
      required this.content,
      this.actions,
      this.contentPadding,
      this.onSubmit,
      this.onCancel})
      : super(key: key);

  final Widget? title;
  final Widget content;
  final List<Widget>? actions;
  final double? contentPadding;
  final Function()? onSubmit;
  final Function()? onCancel;

  @override
  Widget build(BuildContext context) {
    FocusNode focusNode = FocusNode();
    // request focus if there is no focused FocusNode in the dialog
    Future.delayed(Duration.zero, () {
      if (!focusNode.hasFocus) focusNode.requestFocus();
    });
    return Focus(
      focusNode: focusNode,
      autofocus: true,
      onKey: (node, key) {
        if (key.logicalKey == LogicalKeyboardKey.escape) {
          if (key is RawKeyDownEvent) {
            onCancel?.call();
          }
          return KeyEventResult.handled; // avoid TextField exception on escape
        } else if (onSubmit != null &&
            key.logicalKey == LogicalKeyboardKey.enter) {
          if (key is RawKeyDownEvent) onSubmit?.call();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: AlertDialog(
        scrollable: true,
        title: title,
        contentPadding: EdgeInsets.symmetric(
            horizontal: contentPadding ?? 25, vertical: 10),
        content: content,
        actions: actions,
      ),
    );
  }
}

void msgBox(
    String type, String title, String text, OverlayDialogManager dialogManager,
    {bool? hasCancel}) {
  dialogManager.dismissAll();
  List<Widget> buttons = [];
  bool hasOk = false;
  submit() {
    dialogManager.dismissAll();
    // https://github.com/fufesou/rustdesk/blob/5e9a31340b899822090a3731769ae79c6bf5f3e5/src/ui/common.tis#L263
    if (!type.contains("custom") && desktopType != DesktopType.portForward) {
      closeConnection();
    }
  }

  cancel() {
    dialogManager.dismissAll();
  }

  if (type != "connecting" && type != "success" && !type.contains("nook")) {
    hasOk = true;
    buttons.insert(0, msgBoxButton(translate('OK'), submit));
  }
  hasCancel ??= !type.contains("error") &&
      !type.contains("nocancel") &&
      type != "restarting";
  if (hasCancel) {
    buttons.insert(0, msgBoxButton(translate('Cancel'), cancel));
  }
  // TODO: test this button
  if (type.contains("hasclose")) {
    buttons.insert(
        0,
        msgBoxButton(translate('Close'), () {
          dialogManager.dismissAll();
        }));
  }
  dialogManager.show((setState, close) => CustomAlertDialog(
        title: _msgBoxTitle(title),
        content: Text(translate(text), style: const TextStyle(fontSize: 15)),
        actions: buttons,
        onSubmit: hasOk ? submit : null,
        onCancel: hasCancel == true ? cancel : null,
      ));
}

Widget msgBoxButton(String text, void Function() onPressed) {
  return ButtonTheme(
      padding: EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      //limits the touch area to the button area
      minWidth: 0,
      //wraps child's width
      height: 0,
      child: TextButton(
          style: flatButtonStyle,
          onPressed: onPressed,
          child:
              Text(translate(text), style: TextStyle(color: MyTheme.accent))));
}

Widget _msgBoxTitle(String title) =>
    Text(translate(title), style: TextStyle(fontSize: 21));

void msgBoxCommon(OverlayDialogManager dialogManager, String title,
    Widget content, List<Widget> buttons,
    {bool hasCancel = true}) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close) => CustomAlertDialog(
        title: _msgBoxTitle(title),
        content: content,
        actions: buttons,
        onCancel: hasCancel ? close : null,
      ));
}

Color str2color(String str, [alpha = 0xFF]) {
  var hash = 160 << 16 + 114 << 8 + 91;
  for (var i = 0; i < str.length; i += 1) {
    hash = str.codeUnitAt(i) + ((hash << 5) - hash);
  }
  hash = hash % 16777216;
  return Color((hash & 0xFF7FFF) | (alpha << 24));
}

const K = 1024;
const M = K * K;
const G = M * K;

String readableFileSize(double size) {
  if (size < K) {
    return "${size.toStringAsFixed(2)} B";
  } else if (size < M) {
    return "${(size / K).toStringAsFixed(2)} KB";
  } else if (size < G) {
    return "${(size / M).toStringAsFixed(2)} MB";
  } else {
    return "${(size / G).toStringAsFixed(2)} GB";
  }
}

/// Flutter can't not catch PointerMoveEvent when size is 1
/// This will happen in Android AccessibilityService Input
/// android can't init dispatching size yet ,see: https://stackoverflow.com/questions/59960451/android-accessibility-dispatchgesture-is-it-possible-to-specify-pressure-for-a
/// use this temporary solution until flutter or android fixes the bug
class AccessibilityListener extends StatelessWidget {
  final Widget? child;
  static final offset = 100;

  AccessibilityListener({this.child});

  @override
  Widget build(BuildContext context) {
    return Listener(
        onPointerDown: (evt) {
          if (evt.size == 1) {
            GestureBinding.instance.handlePointerEvent(PointerAddedEvent(
                pointer: evt.pointer + offset, position: evt.position));
            GestureBinding.instance.handlePointerEvent(PointerDownEvent(
                pointer: evt.pointer + offset,
                size: 0.1,
                position: evt.position));
          }
        },
        onPointerUp: (evt) {
          if (evt.size == 1) {
            GestureBinding.instance.handlePointerEvent(PointerUpEvent(
                pointer: evt.pointer + offset,
                size: 0.1,
                position: evt.position));
            GestureBinding.instance.handlePointerEvent(PointerRemovedEvent(
                pointer: evt.pointer + offset, position: evt.position));
          }
        },
        onPointerMove: (evt) {
          if (evt.size == 1) {
            GestureBinding.instance.handlePointerEvent(PointerMoveEvent(
                pointer: evt.pointer + offset,
                size: 0.1,
                delta: evt.delta,
                position: evt.position));
          }
        },
        child: child);
  }
}

class PermissionManager {
  static Completer<bool>? _completer;
  static Timer? _timer;
  static var _current = "";

  static final permissions = [
    "audio",
    "file",
    "ignore_battery_optimizations",
    "application_details_settings"
  ];

  static bool isWaitingFile() {
    if (_completer != null) {
      return !_completer!.isCompleted && _current == "file";
    }
    return false;
  }

  static Future<bool> check(String type) {
    if (isDesktop) {
      return Future.value(true);
    }
    if (!permissions.contains(type)) {
      return Future.error("Wrong permission!$type");
    }
    return gFFI.invokeMethod("check_permission", type);
  }

  static Future<bool> request(String type) {
    if (isDesktop) {
      return Future.value(true);
    }
    if (!permissions.contains(type)) {
      return Future.error("Wrong permission!$type");
    }

    gFFI.invokeMethod("request_permission", type);
    if (type == "ignore_battery_optimizations") {
      return Future.value(false);
    }
    _current = type;
    _completer = Completer<bool>();
    gFFI.invokeMethod("request_permission", type);

    // timeout
    _timer?.cancel();
    _timer = Timer(Duration(seconds: 60), () {
      if (_completer == null) return;
      if (!_completer!.isCompleted) {
        _completer!.complete(false);
      }
      _completer = null;
      _current = "";
    });
    return _completer!.future;
  }

  static complete(String type, bool res) {
    if (type != _current) {
      res = false;
    }
    _timer?.cancel();
    _completer?.complete(res);
    _current = "";
  }
}

RadioListTile<T> getRadio<T>(
    String name, T toValue, T curValue, void Function(T?) onChange) {
  return RadioListTile<T>(
    controlAffinity: ListTileControlAffinity.trailing,
    title: Text(translate(name)),
    value: toValue,
    groupValue: curValue,
    onChanged: onChange,
    dense: true,
  );
}

CheckboxListTile getToggle(
    String id, void Function(void Function()) setState, option, name,
    {FFI? ffi}) {
  final opt = bind.sessionGetToggleOptionSync(id: id, arg: option);
  return CheckboxListTile(
      value: opt,
      onChanged: (v) {
        setState(() {
          bind.sessionToggleOption(id: id, value: option);
        });
        if (option == "show-quality-monitor") {
          (ffi ?? gFFI).qualityMonitorModel.checkShowQualityMonitor(id);
        }
      },
      dense: true,
      title: Text(translate(name)));
}

/// find ffi, tag is Remote ID
/// for session specific usage
FFI ffi(String? tag) {
  return Get.find<FFI>(tag: tag);
}

/// Global FFI object
late FFI _globalFFI;

FFI get gFFI => _globalFFI;

Future<void> initGlobalFFI() async {
  debugPrint("_globalFFI init");
  _globalFFI = FFI();
  debugPrint("_globalFFI init end");
  // after `put`, can also be globally found by Get.find<FFI>();
  Get.put(_globalFFI, permanent: true);
}

String translate(String name) {
  if (name.startsWith('Failed to') && name.contains(': ')) {
    return name.split(': ').map((x) => translate(x)).join(': ');
  }
  return platformFFI.translate(name, localeName);
}

bool option2bool(String option, String value) {
  bool res;
  if (option.startsWith("enable-")) {
    res = value != "N";
  } else if (option.startsWith("allow-") ||
      option == "stop-service" ||
      option == "direct-server" ||
      option == "stop-rendezvous-service") {
    res = value == "Y";
  } else {
    assert(false);
    res = value != "N";
  }
  return res;
}

String bool2option(String option, bool b) {
  String res;
  if (option.startsWith('enable-')) {
    res = b ? '' : 'N';
  } else if (option.startsWith('allow-') ||
      option == "stop-service" ||
      option == "direct-server" ||
      option == "stop-rendezvous-service") {
    res = b ? 'Y' : '';
  } else {
    assert(false);
    res = b ? 'Y' : 'N';
  }
  return res;
}

Future<bool> matchPeer(String searchText, Peer peer) async {
  if (searchText.isEmpty) {
    return true;
  }
  if (peer.id.toLowerCase().contains(searchText)) {
    return true;
  }
  if (peer.hostname.toLowerCase().contains(searchText) ||
      peer.username.toLowerCase().contains(searchText)) {
    return true;
  }
  final alias = await bind.mainGetPeerOption(id: peer.id, key: 'alias');
  if (alias.isEmpty) {
    return false;
  }
  return alias.toLowerCase().contains(searchText);
}

Future<List<Peer>>? matchPeers(String searchText, List<Peer> peers) async {
  searchText = searchText.trim();
  if (searchText.isEmpty) {
    return peers;
  }
  searchText = searchText.toLowerCase();
  final matches =
      await Future.wait(peers.map((peer) => matchPeer(searchText, peer)));
  final filteredList = List<Peer>.empty(growable: true);
  for (var i = 0; i < peers.length; i++) {
    if (matches[i]) {
      filteredList.add(peers[i]);
    }
  }
  return filteredList;
}

/// Get the image for the current [platform].
Widget getPlatformImage(String platform, {double size = 50}) {
  platform = platform.toLowerCase();
  if (platform == 'mac os') {
    platform = 'mac';
  } else if (platform != 'linux' && platform != 'android') {
    platform = 'win';
  }
  return Image.asset('assets/$platform.png', height: size, width: size);
}

class LastWindowPosition {
  double? width;
  double? height;
  double? offsetWidth;
  double? offsetHeight;

  LastWindowPosition(
      this.width, this.height, this.offsetWidth, this.offsetHeight);

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      "width": width,
      "height": height,
      "offsetWidth": offsetWidth,
      "offsetHeight": offsetHeight
    };
  }

  @override
  String toString() {
    return jsonEncode(toJson());
  }

  static LastWindowPosition? loadFromString(String content) {
    if (content.isEmpty) {
      return null;
    }
    try {
      final m = jsonDecode(content);
      return LastWindowPosition(
          m["width"], m["height"], m["offsetWidth"], m["offsetHeight"]);
    } catch (e) {
      debugPrint(e.toString());
      return null;
    }
  }
}

/// Save window position and size on exit
/// Note that windowId must be provided if it's subwindow
Future<void> saveWindowPosition(WindowType type, {int? windowId}) async {
  if (type != WindowType.Main && windowId == null) {
    debugPrint(
        "Error: windowId cannot be null when saving positions for sub window");
  }
  switch (type) {
    case WindowType.Main:
      List resp = await Future.wait(
          [windowManager.getPosition(), windowManager.getSize()]);
      Offset position = resp[0];
      Size sz = resp[1];
      final pos =
          LastWindowPosition(sz.width, sz.height, position.dx, position.dy);
      await Get.find<SharedPreferences>()
          .setString(windowPrefix + type.name, pos.toString());
      break;
    default:
      // TODO: implement window
      break;
  }
}

/// Save window position and size on exit
/// Note that windowId must be provided if it's subwindow
Future<bool> restoreWindowPosition(WindowType type, {int? windowId}) async {
  if (type != WindowType.Main && windowId == null) {
    debugPrint(
        "Error: windowId cannot be null when saving positions for sub window");
  }
  switch (type) {
    case WindowType.Main:
      var pos =
          Get.find<SharedPreferences>().getString(windowPrefix + type.name);
      if (pos == null) {
        debugPrint("no window position saved, ignore restore");
        return false;
      }
      var lpos = LastWindowPosition.loadFromString(pos);
      if (lpos == null) {
        debugPrint("window position saved, but cannot be parsed");
        return false;
      }
      await windowManager.setSize(Size(lpos.width ?? 1280, lpos.height ?? 720));
      if (lpos.offsetWidth == null || lpos.offsetHeight == null) {
        await windowManager.center();
      } else {
        await windowManager
            .setPosition(Offset(lpos.offsetWidth!, lpos.offsetHeight!));
      }
      return true;
    default:
      // TODO: implement subwindow
      break;
  }
  return false;
}

/// Connect to a peer with [id].
/// If [isFileTransfer], starts a session only for file transfer.
/// If [isTcpTunneling], starts a session only for tcp tunneling.
/// If [isRDP], starts a session only for rdp.
void connect(BuildContext context, String id,
    {bool isFileTransfer = false,
    bool isTcpTunneling = false,
    bool isRDP = false}) async {
  if (id == '') return;
  id = id.replaceAll(' ', '');
  assert(!(isFileTransfer && isTcpTunneling && isRDP),
      "more than one connect type");

  FocusScopeNode currentFocus = FocusScope.of(context);
  if (isFileTransfer) {
    await rustDeskWinManager.newFileTransfer(id);
  } else if (isTcpTunneling || isRDP) {
    await rustDeskWinManager.newPortForward(id, isRDP);
  } else {
    await rustDeskWinManager.newRemoteDesktop(id);
  }
  if (!currentFocus.hasPrimaryFocus) {
    currentFocus.unfocus();
  }
}
