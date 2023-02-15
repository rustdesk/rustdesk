import 'dart:async';
import 'dart:convert';
import 'dart:ffi' hide Size;
import 'dart:io';
import 'dart:math';

import 'package:back_button_interceptor/back_button_interceptor.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/widgets/refresh_wrapper.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/utils/platform_channel.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:uni_links/uni_links.dart';
import 'package:uni_links_desktop/uni_links_desktop.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:win32/win32.dart' as win32;
import 'package:window_manager/window_manager.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../consts.dart';
import 'common/widgets/overlay.dart';
import 'mobile/pages/file_manager_page.dart';
import 'mobile/pages/remote_page.dart';
import 'models/input_model.dart';
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

/// only available for Windows target
int windowsBuildNumber = 0;
DesktopType? desktopType;

/// Check if the app is running with single view mode.
bool isSingleViewApp() {
  return desktopType == DesktopType.cm;
}

/// * debug or test only, DO NOT enable in release build
bool isTest = false;

typedef F = String Function(String);
typedef FMethod = String Function(String, dynamic);

typedef StreamEventHandler = Future<void> Function(Map<String, dynamic>);

final iconKeyboard = MemoryImage(Uint8List.fromList(base64Decode(
    "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAgVBMVEUAAAD///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////9d3yJTAAAAKnRSTlMA0Gd/0y8ILZgbJffDPUwV2nvzt+TMqZxyU7CMb1pYQyzsvKunkXE4AwJnNC24AAAA+0lEQVQ4y83O2U7DMBCF4ZMxk9rZk26kpQs7nPd/QJy4EiLbLf01N5Y/2YP/qxDFQvGB5NPC/ZpVnfJx4b5xyGfF95rkHvNCWH1u+N6J6T0sC7gqRy8uGPfBLEbozPXUjlkQKwGaFPNizwQbwkx0TDvhCii34ExZCSQVBdzIOEOyeclSHgBGXkpeygXSQgStACtWx4Z8rr8COHOvfEP/IbbsQAToFUAAV1M408IIjIGYAPoCSNRP7DQutfQTqxuAiH7UUg1FaJR2AGrrx52sK2ye28LZ0wBAEyR6y8X+NADhm1B4fgiiHXbRrTrxpwEY9RdM9wsepnvFHfUDwYEeiwAJr/gAAAAASUVORK5CYII=")));
final iconClipboard = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAjVBMVEUAAAD///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////8DizOFAAAALnRSTlMAnIsyZy8YZF3NSAuabRL34cq6trCScyZ4qI9CQDwV+fPl2tnTwzkeB+m/pIFK/Xx0ewAAAQlJREFUOMudktduhDAQRWep69iY3tle0+7/f16Qg7MsJUQ5Dwh8jzRzhemJPIaf3GiW7eFQfOwDPp1ek/iMnKgBi5PrhJAhZAa1lCxE9pw5KWMswOMAQXuQOvqTB7tLFJ36wimKLrufZTzUaoRtdthqRA2vEwS+tR4qguiElRKk1YMrYfUQRkwLmwVBYDMvJKF8R0o3V2MOhNrfo+hXSYYjPn1L/S+n438t8gWh+q1F+cYFBMm1Jh8Ia7y2OWXQxMMRLqr2eTc1crSD84cWfEGwYM4LlaACEee2ZjsQXJxR3qmYb+GpC8ZfNM5oh3yxxbxgQE7lEkb3ZvvH1BiRHn1bu02ICcKGWr4AudUkyYxmvywAAAAASUVORK5CYII=')));
final iconAudio = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAk1BMVEUAAAD////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////ROyVeAAAAMHRSTlMAgfz08DDqCAThvraZjEcoGA751JxzbGdfTRP25NrIpaGTcEM+HAvMuKinhXhWNx9Yzm/gAAABFUlEQVQ4y82S2XLCMAxFheMsQNghCQFalkL39vz/11V4GpNk0r629+Va1pmxPFfyh1ravOP2Y1ydJmBO0lYP3r+PyQ62s2Y7fgF6VRXOYdToT++ogIuoVhCUtX7YpwJG3F8f6V8rr3WABwwUahlEvr8y3IBniGKdKYBQ5OGQpukQakBpIVcfwptIhJcf8hWGakdndAAhBInIGHbdQGJg6jjbDUgEE5EpmB+AAM4uj6gb+AQT6wdhITLvAHJ4VCtgoAlG1tpNA0gWON/f4ioHdSADc1bfgt+PZFkDlD6ojWF+kVoaHlhvFjPHuVRrefohY1GdcFm1N8JvwEyrJ/X2Th2rIoVgIi3Fo6Xf0z5k8psKu5f/oi+nHjjI92o36AAAAABJRU5ErkJggg==')));
final iconFile = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAGAAAABgCAMAAADVRocKAAAAUVBMVEUAAAD///////////////////////////////////////////////////////////////////////////////////////////////////////8IN+deAAAAGnRSTlMAH+CAESEN8jyZkcIb5N/ONy3vmHhmiGjUm7UwS+YAAAHZSURBVGje7dnbboMwDIBhBwgQoFAO7Ta//4NOqCAXYZQstatq4r+r5ubrgQSpg8iyC4ZURa+PlIpQYGiwrzyeHtYZjAL8T05O4H8BbbKvFgRa4NoBU8pXeYEkDDgaaLQBcwJrmeErJQB/7wes3QBWGnCIX0+AQycL1PO6BMwPa0nA4ZxbgTvOjUYMGPHRnZkQAY4mxPZBjmy53E7ukSkFKYB/D4XsWZQx64sCeYebOogGsoOBYvv6/UCb8F0IOBZ0TlP6lEYdANY350AJqB9/qPVuOI5evw4A1hgLigAlepnyxW80bcCcwN++A2s82Vcu02ta+ceq9BoL5KGTTRwQPlpqA3gCnwWU2kCDgeWRQPj2jAPCDxgCMjhI6uZnToDpvd/BJeFrJQB/fsAa02gCt3mi1wNuy8GgBNDZlysBNNSrADVSjcJl6vCpUn6jOdx0kz0q6PMhQRa4465SFKhx35cgUCBTwj2/NHwZAb71qR8GEP2H1XcmAtBPTEO67GP6FUUAIKGABbDLQ0EArhN2sAIGesRO+iyy+RMAjckVTlMCKFVAbh/4Af9OPgG61SkDVco3BQGT3GXaDAnTIAcYZDuBTwGsAGDxuBFeAQqIqwoFMlAVLrHr/wId5MPt0nilGgAAAABJRU5ErkJggg==')));
final iconRestart = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAACXBIWXMAAB7BAAAewQHDaVRTAAAAGXRFWHRTb2Z0d2FyZQB3d3cuaW5rc2NhcGUub3Jnm+48GgAAAbhJREFUWIXVlrFqFGEUhb+7UYxaWCQKlrKKxaZSQVGDJih2tj6MD2DnMwiWvoAIRnENIpZiYxEro6IooiS7SPwsMgNLkk3mjmYmnmb45/73nMNwz/x/qH3gMu2gH6rAU+Blw+Lngau4jpmGxVF7qp1iPWjaQKnZ2WnXbuP/NqAeUPc3ZkA9XDwvqc+BVWCgPlJ7tRwUKThZce819b46VH+pfXVRXVO/q2cSul3VOgZUl0ejq86r39TXI8mqZKDuDEwCw3IREQvAbWAGmMsQZQ0sAl3gHPB1Q+0e8BuYzRDuy2yOiFVgaUxtRf0ETGc4syk4rc6PqU0Cx9j8Zf6dAeAK8Fi9sUXtFjABvEgxJlNwRP2svlNPjbw/q35U36oTFbnyMSwabxb/gB/qA3VBHagrauV7RW0DRfP1IvMlXqkXkhz1DYyQTKtHa/Z2VVMx3IiI+PI3/bCHjuOpFrSnAMpL6QfgTcMGesDx0kBr2BMzsNyi/vtQu8CJlgwsRbZDnWP90NkKaxHxJMOXMqAeAn5u0ydwMCKGY+qbkB3C2W3EKWoXk5zVoHbUZ+6Mh7tl4G4F8RJ3qvL+AfV3r5Vdpj70AAAAAElFTkSuQmCC')));
final iconRecording = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAAXNSR0IArs4c6QAAANpJREFUWEftltENAiEMhtsJ1NcynG6gI+gGugEOR591gppeQoIYSDBILxEeydH/57u2FMF4obE+TAOTwLoIhBDOAHBExG2n6rgR0akW640AM0sn4SWMiDycc7s8JjN7Ijro/k8NqAAR5RoeAPZxv2ggP9hCJiWZxtGbq3hqbJiBVHy4gVx8qAER8Yi4JFy6huVAKXemgb8icI+1b5KEitq0DOO/Nm1EEX1TK27p/bVvv36MOhl4EtHHbFF7jq8AoG1z08OAiFycczrkFNe6RrIet26NMQlMAuYEXiayryF/QQktAAAAAElFTkSuQmCC')));
final iconHardDrive = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAMgAAADICAMAAACahl6sAAAAmVBMVEUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAjHWqVAAAAMnRSTlMAv0BmzLJNXlhiUu2fxXDgu7WuSUUe29LJvpqUjX53VTstD7ilNujCqTEk5IYH+vEoFjKvAagAAAPpSURBVHja7d0JbhpBEIXhB3jYzb5vBgzYgO04df/DJXGUKMwU9ECmZ6pQfSfw028LCXW3YYwxxhhjjDHGGGOM0eZ9VV1MckdKWLM1bRQ/35GW/WxHHu1me6ShuyHvNl34VhlTKsYVeDWj1EzgUZ1S1DrAk/UDparZgxd9Sl0BHnxSBhpI3jfKQG2FpLUpE69I2ILikv1nsvygjBwPSNKYMlNHggqUoSKS80AZCnwHqQ1zCRvW+CRegwRFeFAMKKrtM8gTPJlzSfwFgT9dJom3IDN4VGaSeAryAK8m0SSeghTg1ZYiql6CjBDhO8mzlyAVhKhIwgXxrh5NojGIhyRckEdwpCdhgpSQgiWTRGMQNonGIGySp0SDvMDBX5KWxiB8Eo1BgE00SYJBykhNnkmSWJAcLpGaJNMgfJKyxiDAK4WNEwryhMtkJsk8CJtEYxA+icYgQIfCcgkEqcJNXhIRQdgkGoPwSTQG+e8khdu/7JOVREwQIKCwF41B2CQljUH4JLcH6SI+OUlEBQHa0SQag/BJNAbhkjxqDMIn0RgEeI4muSlID9eSkERgEKAVTaIxCJ9EYxA2ydVB8hCASVLRGAQYR5NoDMIn0RgEyFHYSGMQPonGII4kziCNvBgNJonEk4u3GAk8Sprk6eYaqbMDY0oKvUm5jfC/viGiSypV7+M3i2iDsAGpNEDYjlTa3W8RdR/r544g50ilnA0RxoZIE2NIXqQbhkAkGyKNDZHGhkhjQ6SxIdLYEGlsiDQ2JGTVeD0264U9zipPh7XOooffpA6pfNCXjxl4/c3pUzlChwzor53zwYYVfpI5pOV6LWFF/2jiJ5FDSs5jdY/0rwUAkUMeXWdBqnSqD0DikBqdqCHsjTvELm9In0IOri/0pwAEDtlSyNaRjAIAAoesKWTtuusxByBwCJp0oomwBXcYUuCQgE50ENajE4OvZAKHLB1/68Br5NqiyCGYOY8YRd77kTkEb64n7lZN+mOIX4QOwb5FX0ZVx3uOxwW+SB0CbBubemWP8/rlaaeRX+M3uUOuZENsiA25zIbYkPsZElBIHwL13U/PTjJ/cyOOEoVM3I+hziDQlELm7pPxw3eI8/7gPh1fpLA6xGnEeDDgO0UcIAzzM35HxLPIq5SXe9BLzOsj9eUaQqyXzxS1QFSfWM2cCANiHcAISJ0AnCKpUwTuIkkA3EeSInAXSQKcs1V18e24wlllUmQp9v9zXKeHi+akRAMOPVKhAqdPBZeUmnnEsO6QcJ0+4qmOSbBxFfGVRiTUqITrdKcCbyYO3/K4wX4+aQ+FfNjXhu3JfAVjjDHGGGOMMcYYY4xIPwCgfqT6TbhCLAAAAABJRU5ErkJggg==')));

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
  static const IconData roundClose = IconData(0xe6ed, fontFamily: _family2);
}

class ColorThemeExtension extends ThemeExtension<ColorThemeExtension> {
  const ColorThemeExtension({
    required this.border,
    required this.highlight,
  });

  final Color? border;
  final Color? highlight;

  static const light = ColorThemeExtension(
    border: Color(0xFFCCCCCC),
    highlight: Color(0xFFE5E5E5),
  );

  static const dark = ColorThemeExtension(
    border: Color(0xFF555555),
    highlight: Color(0xFF3F3F3F),
  );

  @override
  ThemeExtension<ColorThemeExtension> copyWith(
      {Color? border, Color? highlight}) {
    return ColorThemeExtension(
      border: border ?? this.border,
      highlight: highlight ?? this.highlight,
    );
  }

  @override
  ThemeExtension<ColorThemeExtension> lerp(
      ThemeExtension<ColorThemeExtension>? other, double t) {
    if (other is! ColorThemeExtension) {
      return this;
    }
    return ColorThemeExtension(
      border: Color.lerp(border, other.border, t),
      highlight: Color.lerp(highlight, other.highlight, t),
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
    backgroundColor: Color(0xFFFFFFFF),
    scaffoldBackgroundColor: Color(0xFFEEEEEE),
    textTheme: const TextTheme(
        titleLarge: TextStyle(fontSize: 19, color: Colors.black87),
        titleSmall: TextStyle(fontSize: 14, color: Colors.black87),
        bodySmall: TextStyle(fontSize: 12, color: Colors.black87, height: 1.25),
        bodyMedium:
            TextStyle(fontSize: 14, color: Colors.black87, height: 1.25),
        labelLarge: TextStyle(fontSize: 16.0, color: MyTheme.accent80)),
    hintColor: Color(0xFFAAAAAA),
    primarySwatch: Colors.blue,
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: const TabBarTheme(
      labelColor: Colors.black87,
    ),
    splashColor: Colors.transparent,
    highlightColor: Colors.transparent,
    splashFactory: isDesktop ? NoSplash.splashFactory : null,
    textButtonTheme: isDesktop
        ? TextButtonThemeData(
            style: ButtonStyle(splashFactory: NoSplash.splashFactory),
          )
        : null,
  ).copyWith(
    extensions: <ThemeExtension<dynamic>>[
      ColorThemeExtension.light,
      TabbarTheme.light,
    ],
  );
  static ThemeData darkTheme = ThemeData(
    brightness: Brightness.dark,
    backgroundColor: Color(0xFF252525),
    scaffoldBackgroundColor: Color(0xFF141414),
    textTheme: const TextTheme(
        titleLarge: TextStyle(fontSize: 19),
        titleSmall: TextStyle(fontSize: 14),
        bodySmall: TextStyle(fontSize: 12, height: 1.25),
        bodyMedium: TextStyle(fontSize: 14, height: 1.25),
        labelLarge: TextStyle(
            fontSize: 16.0, fontWeight: FontWeight.bold, color: accent80)),
    cardColor: Color(0xFF252525),
    primarySwatch: Colors.blue,
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: const TabBarTheme(
      labelColor: Colors.white70,
    ),
    splashColor: Colors.transparent,
    highlightColor: Colors.transparent,
    splashFactory: isDesktop ? NoSplash.splashFactory : null,
    outlinedButtonTheme: OutlinedButtonThemeData(
        style:
            OutlinedButton.styleFrom(side: BorderSide(color: Colors.white38))),
    textButtonTheme: isDesktop
        ? TextButtonThemeData(
            style: ButtonStyle(splashFactory: NoSplash.splashFactory),
          )
        : null,
  ).copyWith(
    extensions: <ThemeExtension<dynamic>>[
      ColorThemeExtension.dark,
      TabbarTheme.dark,
    ],
  );

  static ThemeMode getThemeModePreference() {
    return themeModeFromString(bind.mainGetLocalOption(key: kCommConfKeyTheme));
  }

  static void changeDarkMode(ThemeMode mode) async {
    Get.changeThemeMode(mode);
    if (desktopType == DesktopType.main) {
      if (mode == ThemeMode.system) {
        await bind.mainSetLocalOption(key: kCommConfKeyTheme, value: '');
      } else {
        await bind.mainSetLocalOption(
            key: kCommConfKeyTheme, value: mode.toShortString());
      }
      await bind.mainChangeTheme(dark: mode.toShortString());
      // Synchronize the window theme of the system.
      updateSystemWindowTheme();
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
  if (!isDesktop) {
    return;
  }
  if (id == null) {
    // main window
    windowManager.restore();
    windowManager.show();
    windowManager.focus();
    rustDeskWinManager.registerActiveWindow(kWindowMainId);
  } else {
    WindowController.fromWindowId(id)
      ..focus()
      ..show();
    rustDeskWinManager.call(WindowType.Main, kWindowEventShow, {"id": id});
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

class OverlayKeyState {
  final _overlayKey = GlobalKey<OverlayState>();

  /// use global overlay by default
  OverlayState? get state =>
      _overlayKey.currentState ?? globalKey.currentState?.overlay;

  GlobalKey<OverlayState>? get key => _overlayKey;
}

class OverlayDialogManager {
  final Map<String, Dialog> _dialogs = {};
  var _overlayKeyState = OverlayKeyState();
  int _tagCount = 0;

  OverlayEntry? _mobileActionsOverlayEntry;

  void setOverlayState(OverlayKeyState overlayKeyState) {
    _overlayKeyState = overlayKeyState;
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
        forceGlobal ? globalKey.currentState?.overlay : _overlayKeyState.state;

    if (overlayState == null) {
      return Future.error(
          "[OverlayDialogManager] Failed to show dialog, _overlayState is null, call [setOverlayState] first");
    }

    final String dialogTag;
    if (tag != null) {
      dialogTag = tag;
    } else {
      dialogTag = _tagCount.toString();
      _tagCount++;
    }

    final dialog = Dialog<T>();
    _dialogs[dialogTag] = dialog;

    close([res]) {
      _dialogs.remove(dialogTag);
      dialog.complete(res);
      BackButtonInterceptor.removeByName(dialogTag);
    }

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
    }, name: dialogTag);
    return dialog.completer.future;
  }

  String showLoading(String text,
      {bool clickMaskDismiss = false,
      bool showCancel = true,
      VoidCallback? onCancel}) {
    final tag = _tagCount.toString();
    _tagCount++;
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
                          child: isDesktop
                              ? dialogButton('Cancel', onPressed: cancel)
                              : TextButton(
                                  style: flatButtonStyle,
                                  onPressed: cancel,
                                  child: Text(translate('Cancel'),
                                      style: const TextStyle(
                                          color: MyTheme.accent)))))
                ])),
        onCancel: showCancel ? cancel : null,
      );
    }, tag: tag);
    return tag;
  }

  void resetMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry == null) return;
    hideMobileActionsOverlay();
    showMobileActionsOverlay(ffi: ffi);
  }

  void showMobileActionsOverlay({FFI? ffi}) {
    if (_mobileActionsOverlayEntry != null) return;
    final overlayState = _overlayKeyState.state;
    if (overlayState == null) return;

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
        onBackPressed: () => session.inputModel.tap(MouseButtons.right),
        onHomePressed: () => session.inputModel.tap(MouseButtons.wheel),
        onRecentPressed: () async {
          session.inputModel.sendMouse('down', MouseButtons.wheel);
          await Future.delayed(const Duration(milliseconds: 500));
          session.inputModel.sendMouse('up', MouseButtons.wheel);
        },
        onHidePressed: () => hideMobileActionsOverlay(),
      );
    });
    overlayState.insert(overlay);
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

  bool existing(String tag) {
    return _dialogs.keys.contains(tag);
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
      this.contentBoxConstraints = const BoxConstraints(maxWidth: 500),
      this.onSubmit,
      this.onCancel})
      : super(key: key);

  final Widget? title;
  final Widget content;
  final List<Widget>? actions;
  final double? contentPadding;
  final BoxConstraints contentBoxConstraints;
  final Function()? onSubmit;
  final Function()? onCancel;

  @override
  Widget build(BuildContext context) {
    // request focus
    FocusScopeNode scopeNode = FocusScopeNode();
    Future.delayed(Duration.zero, () {
      if (!scopeNode.hasFocus) scopeNode.requestFocus();
    });
    const double padding = 16;
    bool tabTapped = false;
    return FocusScope(
      node: scopeNode,
      autofocus: true,
      onKey: (node, key) {
        if (key.logicalKey == LogicalKeyboardKey.escape) {
          if (key is RawKeyDownEvent) {
            onCancel?.call();
          }
          return KeyEventResult.handled; // avoid TextField exception on escape
        } else if (!tabTapped &&
            onSubmit != null &&
            key.logicalKey == LogicalKeyboardKey.enter) {
          if (key is RawKeyDownEvent) onSubmit?.call();
          return KeyEventResult.handled;
        } else if (key.logicalKey == LogicalKeyboardKey.tab) {
          if (key is RawKeyDownEvent) {
            scopeNode.nextFocus();
            tabTapped = true;
          }
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: AlertDialog(
        scrollable: true,
        title: title,
        titlePadding: EdgeInsets.fromLTRB(padding, 24, padding, 0),
        contentPadding: EdgeInsets.fromLTRB(
            contentPadding ?? padding, 25, contentPadding ?? padding, 10),
        content: ConstrainedBox(
          constraints: contentBoxConstraints,
          child: Theme(
              data: Theme.of(context).copyWith(
                  inputDecorationTheme: InputDecorationTheme(
                      isDense: true, contentPadding: EdgeInsets.all(15))),
              child: content),
        ),
        actions: actions,
        actionsPadding: EdgeInsets.fromLTRB(padding, 0, padding, padding),
      ),
    );
  }
}

void msgBox(String id, String type, String title, String text, String link,
    OverlayDialogManager dialogManager,
    {bool? hasCancel, ReconnectHandle? reconnect}) {
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

  jumplink() {
    if (link.startsWith('http')) {
      launchUrl(Uri.parse(link));
    }
  }

  if (type != "connecting" && type != "success" && !type.contains("nook")) {
    hasOk = true;
    buttons.insert(0, dialogButton('OK', onPressed: submit));
  }
  hasCancel ??= !type.contains("error") &&
      !type.contains("nocancel") &&
      type != "restarting";
  if (hasCancel) {
    buttons.insert(
        0, dialogButton('Cancel', onPressed: cancel, isOutline: true));
  }
  if (type.contains("hasclose")) {
    buttons.insert(
        0,
        dialogButton('Close', onPressed: () {
          dialogManager.dismissAll();
        }));
  }
  if (reconnect != null && title == "Connection Error") {
    buttons.insert(
        0,
        dialogButton('Reconnect', isOutline: true, onPressed: () {
          reconnect(dialogManager, id, false);
        }));
  }
  if (link.isNotEmpty) {
    buttons.insert(0, dialogButton('JumpLink', onPressed: jumplink));
  }
  dialogManager.show(
    (setState, close) => CustomAlertDialog(
      title: null,
      content: SelectionArea(child: msgboxContent(type, title, text)),
      actions: buttons,
      onSubmit: hasOk ? submit : null,
      onCancel: hasCancel == true ? cancel : null,
    ),
    tag: '$id-$type-$title-$text-$link',
  );
}

Color? _msgboxColor(String type) {
  if (type == "input-password" || type == "custom-os-password") {
    return Color(0xFFAD448E);
  }
  if (type.contains("success")) {
    return Color(0xFF32bea6);
  }
  if (type.contains("error") || type == "re-input-password") {
    return Color(0xFFE04F5F);
  }
  return Color(0xFF2C8CFF);
}

Widget msgboxIcon(String type) {
  IconData? iconData;
  if (type.contains("error") || type == "re-input-password") {
    iconData = Icons.cancel;
  }
  if (type.contains("success")) {
    iconData = Icons.check_circle;
  }
  if (type == "wait-uac" || type == "wait-remote-accept-nook") {
    iconData = Icons.hourglass_top;
  }
  if (type == 'on-uac' || type == 'on-foreground-elevated') {
    iconData = Icons.admin_panel_settings;
  }
  if (type == "info") {
    iconData = Icons.info;
  }
  if (iconData != null) {
    return Icon(iconData, size: 50, color: _msgboxColor(type))
        .marginOnly(right: 16);
  }

  return Offstage();
}

// title should be null
Widget msgboxContent(String type, String title, String text) {
  return Row(
    children: [
      msgboxIcon(type),
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              translate(title),
              style: TextStyle(fontSize: 21),
            ).marginOnly(bottom: 10),
            Text(translate(text), style: const TextStyle(fontSize: 15)),
          ],
        ),
      ),
    ],
  ).marginOnly(bottom: 12);
}

void msgBoxCommon(OverlayDialogManager dialogManager, String title,
    Widget content, List<Widget> buttons,
    {bool hasCancel = true}) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close) => CustomAlertDialog(
        title: Text(
          translate(title),
          style: TextStyle(fontSize: 21),
        ),
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
    String name, T toValue, T curValue, void Function(T?) onChange,
    {EdgeInsetsGeometry? contentPadding}) {
  return RadioListTile<T>(
    contentPadding: contentPadding,
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
      option == "stop-rendezvous-service" ||
      option == "force-always-relay") {
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
      option == "stop-rendezvous-service" ||
      option == "force-always-relay") {
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

/// Get the image for the current [platform].
Widget getPlatformImage(String platform, {double size = 50}) {
  if (platform == kPeerPlatformMacOS) {
    platform = 'mac';
  } else if (platform != kPeerPlatformLinux &&
      platform != kPeerPlatformAndroid) {
    platform = 'win';
  } else {
    platform = platform.toLowerCase();
  }
  return SvgPicture.asset('assets/$platform.svg', height: size, width: size);
}

class LastWindowPosition {
  double? width;
  double? height;
  double? offsetWidth;
  double? offsetHeight;
  bool? isMaximized;

  LastWindowPosition(this.width, this.height, this.offsetWidth,
      this.offsetHeight, this.isMaximized);

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      "width": width,
      "height": height,
      "offsetWidth": offsetWidth,
      "offsetHeight": offsetHeight,
      "isMaximized": isMaximized,
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
      return LastWindowPosition(m["width"], m["height"], m["offsetWidth"],
          m["offsetHeight"], m["isMaximized"]);
    } catch (e) {
      debugPrintStack(label: e.toString());
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
      final position = await windowManager.getPosition();
      final sz = await windowManager.getSize();
      final isMaximized = await windowManager.isMaximized();
      final pos = LastWindowPosition(
          sz.width, sz.height, position.dx, position.dy, isMaximized);
      await bind.setLocalFlutterConfig(
          k: kWindowPrefix + type.name, v: pos.toString());
      break;
    default:
      final wc = WindowController.fromWindowId(windowId!);
      final frame = await wc.getFrame();
      final position = frame.topLeft;
      final sz = frame.size;
      final isMaximized = await wc.isMaximized();
      final pos = LastWindowPosition(
          sz.width, sz.height, position.dx, position.dy, isMaximized);
      debugPrint(
          "saving frame: $windowId: ${pos.width}/${pos.height}, offset:${pos.offsetWidth}/${pos.offsetHeight}");
      await bind.setLocalFlutterConfig(
          k: kWindowPrefix + type.name, v: pos.toString());
      break;
  }
}

Future<Size> _adjustRestoreMainWindowSize(double? width, double? height) async {
  const double minWidth = 600;
  const double minHeight = 100;
  double maxWidth = (((isDesktop || isWebDesktop)
          ? kDesktopMaxDisplayWidth
          : kMobileMaxDisplayWidth))
      .toDouble();
  double maxHeight = ((isDesktop || isWebDesktop)
          ? kDesktopMaxDisplayHeight
          : kMobileMaxDisplayHeight)
      .toDouble();

  if (isDesktop || isWebDesktop) {
    final screen = (await window_size.getWindowInfo()).screen;
    if (screen != null) {
      maxWidth = screen.visibleFrame.width;
      maxHeight = screen.visibleFrame.height;
    }
  }

  final defaultWidth =
      ((isDesktop || isWebDesktop) ? 1280 : kMobileDefaultDisplayWidth)
          .toDouble();
  final defaultHeight =
      ((isDesktop || isWebDesktop) ? 720 : kMobileDefaultDisplayHeight)
          .toDouble();
  double restoreWidth = width ?? defaultWidth;
  double restoreHeight = height ?? defaultHeight;

  if (restoreWidth < minWidth) {
    restoreWidth = minWidth;
  }
  if (restoreHeight < minHeight) {
    restoreHeight = minHeight;
  }
  if (restoreWidth > maxWidth) {
    restoreWidth = maxWidth;
  }
  if (restoreHeight > maxHeight) {
    restoreHeight = maxHeight;
  }
  return Size(restoreWidth, restoreHeight);
}

/// return null means center
Future<Offset?> _adjustRestoreMainWindowOffset(
    double? left, double? top) async {
  if (left == null || top == null) {
    await windowManager.center();
  } else {
    double windowLeft = max(0.0, left);
    double windowTop = max(0.0, top);

    double frameLeft = double.infinity;
    double frameTop = double.infinity;
    double frameRight = ((isDesktop || isWebDesktop)
            ? kDesktopMaxDisplayWidth
            : kMobileMaxDisplayWidth)
        .toDouble();
    double frameBottom = ((isDesktop || isWebDesktop)
            ? kDesktopMaxDisplayHeight
            : kMobileMaxDisplayHeight)
        .toDouble();

    if (isDesktop || isWebDesktop) {
      for (final screen in await window_size.getScreenList()) {
        frameLeft = min(screen.visibleFrame.left, frameLeft);
        frameTop = min(screen.visibleFrame.top, frameTop);
        frameRight = max(screen.visibleFrame.right, frameRight);
        frameBottom = max(screen.visibleFrame.bottom, frameBottom);
      }
    }

    if (windowLeft < frameLeft ||
        windowLeft > frameRight ||
        windowTop < frameTop ||
        windowTop > frameBottom) {
      return null;
    } else {
      return Offset(windowLeft, windowTop);
    }
  }
  return null;
}

/// Restore window position and size on start
/// Note that windowId must be provided if it's subwindow
Future<bool> restoreWindowPosition(WindowType type, {int? windowId}) async {
  if (type != WindowType.Main && windowId == null) {
    debugPrint(
        "Error: windowId cannot be null when saving positions for sub window");
  }
  final pos = bind.getLocalFlutterConfig(k: kWindowPrefix + type.name);
  var lpos = LastWindowPosition.loadFromString(pos);
  if (lpos == null) {
    debugPrint("no window position saved, ignoring position restoration");
    return false;
  }

  switch (type) {
    case WindowType.Main:
      if (lpos.isMaximized == true) {
        await windowManager.maximize();
      } else {
        final size =
            await _adjustRestoreMainWindowSize(lpos.width, lpos.height);
        final offset = await _adjustRestoreMainWindowOffset(
            lpos.offsetWidth, lpos.offsetHeight);
        await windowManager.setSize(size);
        if (offset == null) {
          await windowManager.center();
        } else {
          await windowManager.setPosition(offset);
        }
      }
      return true;
    default:
      final wc = WindowController.fromWindowId(windowId!);
      if (lpos.isMaximized == true) {
        await wc.maximize();
      } else {
        final size =
            await _adjustRestoreMainWindowSize(lpos.width, lpos.height);
        final offset = await _adjustRestoreMainWindowOffset(
            lpos.offsetWidth, lpos.offsetHeight);
        debugPrint(
            "restore lpos: ${size.width}/${size.height}, offset:${offset?.dx}/${offset?.dy}");
        if (offset == null) {
          await wc.center();
        } else {
          final frame =
              Rect.fromLTWH(offset.dx, offset.dy, size.width, size.height);
          await wc.setFrame(frame);
        }
      }
      break;
  }
  return false;
}

/// Initialize uni links for macos/windows
///
/// [Availability]
/// initUniLinks should only be used on macos/windows.
/// we use dbus for linux currently.
Future<bool> initUniLinks() async {
  if (Platform.isLinux) {
    return false;
  }
  // Register uni links for Windows. The required info of url scheme is already
  // declared in `Info.plist` for macOS.
  if (Platform.isWindows) {
    registerProtocol('rustdesk');
  }
  // check cold boot
  try {
    final initialLink = await getInitialLink();
    if (initialLink == null) {
      return false;
    }
    return parseRustdeskUri(initialLink);
  } catch (err) {
    debugPrintStack(label: "$err");
    return false;
  }
}

/// Listen for uni links.
///
/// * handleByFlutter: Should uni links be handled by Flutter.
///
/// Returns a [StreamSubscription] which can listen the uni links.
StreamSubscription? listenUniLinks({handleByFlutter = true}) {
  if (Platform.isLinux) {
    return null;
  }

  final sub = uriLinkStream.listen((Uri? uri) {
    debugPrint("A uri was received: $uri.");
    if (uri != null) {
      if (handleByFlutter) {
        callUniLinksUriHandler(uri);
      } else {
        bind.sendUrlScheme(url: uri.toString());
      }
    } else {
      print("uni listen error: uri is empty.");
    }
  }, onError: (err) {
    print("uni links error: $err");
  });
  return sub;
}

/// Handle command line arguments
///
/// * Returns true if we successfully handle the startup arguments.
bool checkArguments() {
  if (kBootArgs.isNotEmpty) {
    final ret = parseRustdeskUri(kBootArgs.first);
    if (ret) {
      return true;
    }
  }
  // bootArgs:[--connect, 362587269, --switch_uuid, e3d531cc-5dce-41e0-bd06-5d4a2b1eec05]
  // check connect args
  var connectIndex = kBootArgs.indexOf("--connect");
  if (connectIndex == -1) {
    return false;
  }
  String? id =
      kBootArgs.length < connectIndex + 1 ? null : kBootArgs[connectIndex + 1];
  final switchUuidIndex = kBootArgs.indexOf("--switch_uuid");
  String? switchUuid = kBootArgs.length < switchUuidIndex + 1
      ? null
      : kBootArgs[switchUuidIndex + 1];
  if (id != null) {
    if (id.startsWith(kUniLinksPrefix)) {
      return parseRustdeskUri(id);
    } else {
      // remove "--connect xxx" in the `bootArgs` array
      kBootArgs.removeAt(connectIndex);
      kBootArgs.removeAt(connectIndex);
      // fallback to peer id
      Future.delayed(Duration.zero, () {
        rustDeskWinManager.newRemoteDesktop(id, switch_uuid: switchUuid);
      });
      return true;
    }
  }
  return false;
}

/// Parse `rustdesk://` unilinks
///
/// Returns true if we successfully handle the uri provided.
/// [Functions]
/// 1. New Connection: rustdesk://connection/new/your_peer_id
bool parseRustdeskUri(String uriPath) {
  final uri = Uri.tryParse(uriPath);
  if (uri == null) {
    debugPrint("uri is not valid: $uriPath");
    return false;
  }
  return callUniLinksUriHandler(uri);
}

/// uri handler
///
/// Returns true if we successfully handle the uri provided.
bool callUniLinksUriHandler(Uri uri) {
  debugPrint("uni links called: $uri");
  // new connection
  if (uri.authority == "connection" && uri.path.startsWith("/new/")) {
    final peerId = uri.path.substring("/new/".length);
    var param = uri.queryParameters;
    String? switch_uuid = param["switch_uuid"];
    Future.delayed(Duration.zero, () {
      rustDeskWinManager.newRemoteDesktop(peerId, switch_uuid: switch_uuid);
    });
    return true;
  }
  return false;
}

connectMainDesktop(String id,
    {required bool isFileTransfer,
    required bool isTcpTunneling,
    required bool isRDP,
    bool? forceRelay}) async {
  if (isFileTransfer) {
    await rustDeskWinManager.newFileTransfer(id, forceRelay: forceRelay);
  } else if (isTcpTunneling || isRDP) {
    await rustDeskWinManager.newPortForward(id, isRDP, forceRelay: forceRelay);
  } else {
    await rustDeskWinManager.newRemoteDesktop(id, forceRelay: forceRelay);
  }
}

/// Connect to a peer with [id].
/// If [isFileTransfer], starts a session only for file transfer.
/// If [isTcpTunneling], starts a session only for tcp tunneling.
/// If [isRDP], starts a session only for rdp.
connect(BuildContext context, String id,
    {bool isFileTransfer = false,
    bool isTcpTunneling = false,
    bool isRDP = false,
    bool forceRelay = false}) async {
  if (id == '') return;
  id = id.replaceAll(' ', '');
  assert(!(isFileTransfer && isTcpTunneling && isRDP),
      "more than one connect type");

  if (isDesktop) {
    if (desktopType == DesktopType.main) {
      await connectMainDesktop(id,
          isFileTransfer: isFileTransfer,
          isTcpTunneling: isTcpTunneling,
          isRDP: isRDP,
          forceRelay: forceRelay);
    } else {
      await rustDeskWinManager.call(WindowType.Main, kWindowConnect, {
        'id': id,
        'isFileTransfer': isFileTransfer,
        'isTcpTunneling': isTcpTunneling,
        'isRDP': isRDP,
        "forceRelay": forceRelay,
      });
    }
  } else {
    if (isFileTransfer) {
      if (!await PermissionManager.check("file")) {
        if (!await PermissionManager.request("file")) {
          return;
        }
      }
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => FileManagerPage(id: id),
        ),
      );
    } else {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => RemotePage(id: id),
        ),
      );
    }
  }

  FocusScopeNode currentFocus = FocusScope.of(context);
  if (!currentFocus.hasPrimaryFocus) {
    currentFocus.unfocus();
  }
}

Map<String, String> getHttpHeaders() {
  return {
    'Authorization': 'Bearer ${bind.mainGetLocalOption(key: 'access_token')}'
  };
}

// Simple wrapper of built-in types for reference use.
class SimpleWrapper<T> {
  T value;
  SimpleWrapper(this.value);
}

/// call this to reload current window.
///
/// [Note]
/// Must have [RefreshWrapper] on the top of widget tree.
void reloadCurrentWindow() {
  if (Get.context != null) {
    // reload self window
    RefreshWrapper.of(Get.context!)?.rebuild();
  } else {
    debugPrint(
        "reload current window failed, global BuildContext does not exist");
  }
}

/// call this to reload all windows, including main + all sub windows.
Future<void> reloadAllWindows() async {
  reloadCurrentWindow();
  try {
    final ids = await DesktopMultiWindow.getAllSubWindowIds();
    for (final id in ids) {
      DesktopMultiWindow.invokeMethod(id, kWindowActionRebuild);
    }
  } on AssertionError {
    // ignore
  }
}

/// Indicate the flutter app is running in portable mode.
///
/// [Note]
/// Portable build is only available on Windows.
bool isRunningInPortableMode() {
  if (!Platform.isWindows) {
    return false;
  }
  return bool.hasEnvironment(kEnvPortableExecutable);
}

/// Window status callback
Future<void> onActiveWindowChanged() async {
  print(
      "[MultiWindowHandler] active window changed: ${rustDeskWinManager.getActiveWindows()}");
  if (rustDeskWinManager.getActiveWindows().isEmpty) {
    // close all sub windows
    try {
      await Future.wait([
        saveWindowPosition(WindowType.Main),
        rustDeskWinManager.closeAllSubWindows()
      ]);
    } catch (err) {
      debugPrintStack(label: "$err");
    } finally {
      debugPrint("Start closing RustDesk...");
      await windowManager.setPreventClose(false);
      await windowManager.close();
      if (Platform.isMacOS) {
        RdPlatformChannel.instance.terminate();
      }
    }
  }
}

Timer periodic_immediate(Duration duration, Future<void> Function() callback) {
  Future.delayed(Duration.zero, callback);
  return Timer.periodic(duration, (timer) async {
    await callback();
  });
}

/// return a human readable windows version
WindowsTarget getWindowsTarget(int buildNumber) {
  if (!Platform.isWindows) {
    return WindowsTarget.naw;
  }
  if (buildNumber >= 22000) {
    return WindowsTarget.w11;
  } else if (buildNumber >= 10240) {
    return WindowsTarget.w10;
  } else if (buildNumber >= 9600) {
    return WindowsTarget.w8_1;
  } else if (buildNumber >= 9200) {
    return WindowsTarget.w8;
  } else if (buildNumber >= 7601) {
    return WindowsTarget.w7;
  } else if (buildNumber >= 6002) {
    return WindowsTarget.vista;
  } else {
    // minimum support
    return WindowsTarget.xp;
  }
}

/// Get windows target build number.
///
/// [Note]
/// Please use this function wrapped with `Platform.isWindows`.
int getWindowsTargetBuildNumber() {
  final rtlGetVersion = DynamicLibrary.open('ntdll.dll').lookupFunction<
      Void Function(Pointer<win32.OSVERSIONINFOEX>),
      void Function(Pointer<win32.OSVERSIONINFOEX>)>('RtlGetVersion');
  final osVersionInfo = getOSVERSIONINFOEXPointer();
  rtlGetVersion(osVersionInfo);
  int buildNumber = osVersionInfo.ref.dwBuildNumber;
  calloc.free(osVersionInfo);
  return buildNumber;
}

/// Get Windows OS version pointer
///
/// [Note]
/// Please use this function wrapped with `Platform.isWindows`.
Pointer<win32.OSVERSIONINFOEX> getOSVERSIONINFOEXPointer() {
  final pointer = calloc<win32.OSVERSIONINFOEX>();
  pointer.ref
    ..dwOSVersionInfoSize = sizeOf<win32.OSVERSIONINFOEX>()
    ..dwBuildNumber = 0
    ..dwMajorVersion = 0
    ..dwMinorVersion = 0
    ..dwPlatformId = 0
    ..szCSDVersion = ''
    ..wServicePackMajor = 0
    ..wServicePackMinor = 0
    ..wSuiteMask = 0
    ..wProductType = 0
    ..wReserved = 0;
  return pointer;
}

/// Indicating we need to use compatible ui mode.
///
/// [Conditions]
/// - Windows 7, window will overflow when we use frameless ui.
bool get kUseCompatibleUiMode =>
    Platform.isWindows &&
    const [WindowsTarget.w7].contains(windowsBuildNumber.windowsVersion);

class ServerConfig {
  late String idServer;
  late String relayServer;
  late String apiServer;
  late String key;

  ServerConfig(
      {String? idServer, String? relayServer, String? apiServer, String? key}) {
    this.idServer = idServer?.trim() ?? '';
    this.relayServer = relayServer?.trim() ?? '';
    this.apiServer = apiServer?.trim() ?? '';
    this.key = key?.trim() ?? '';
  }

  /// decode from shared string (from user shared or rustdesk-server generated)
  /// also see [encode]
  /// throw when decoding failure
  ServerConfig.decode(String msg) {
    final input = msg.split('').reversed.join('');
    final bytes = base64Decode(base64.normalize(input));
    final json = jsonDecode(utf8.decode(bytes));

    idServer = json['host'] ?? '';
    relayServer = json['relay'] ?? '';
    apiServer = json['api'] ?? '';
    key = json['key'] ?? '';
  }

  /// encode to shared string
  /// also see [ServerConfig.decode]
  String encode() {
    Map<String, String> config = {};
    config['host'] = idServer.trim();
    config['relay'] = relayServer.trim();
    config['api'] = apiServer.trim();
    config['key'] = key.trim();
    return base64Encode(Uint8List.fromList(jsonEncode(config).codeUnits))
        .split('')
        .reversed
        .join();
  }

  /// from local options
  ServerConfig.fromOptions(Map<String, dynamic> options)
      : idServer = options['custom-rendezvous-server'] ?? "",
        relayServer = options['relay-server'] ?? "",
        apiServer = options['api-server'] ?? "",
        key = options['key'] ?? "";
}

Widget dialogButton(String text,
    {required VoidCallback? onPressed,
    bool isOutline = false,
    TextStyle? style,
    ButtonStyle? buttonStyle}) {
  if (isDesktop) {
    if (isOutline) {
      return OutlinedButton(
        onPressed: onPressed,
        child: Text(translate(text), style: style),
      );
    } else {
      return ElevatedButton(
        style: ElevatedButton.styleFrom(elevation: 0).merge(buttonStyle),
        onPressed: onPressed,
        child: Text(translate(text), style: style),
      );
    }
  } else {
    return TextButton(
        onPressed: onPressed,
        child: Text(
          translate(text),
          style: style,
        ));
  }
}

int version_cmp(String v1, String v2) {
  return bind.versionToNumber(v: v1) - bind.versionToNumber(v: v2);
}

String getWindowName({WindowType? overrideType}) {
  switch (overrideType ?? kWindowType) {
    case WindowType.Main:
      return "RustDesk";
    case WindowType.FileTransfer:
      return "File Transfer - RustDesk";
    case WindowType.PortForward:
      return "Port Forward - RustDesk";
    case WindowType.RemoteDesktop:
      return "Remote Desktop - RustDesk";
    default:
      break;
  }
  return "RustDesk";
}

String getWindowNameWithId(String id, {WindowType? overrideType}) {
  return "${DesktopTab.labelGetterAlias(id).value} - ${getWindowName(overrideType: overrideType)}";
}

Future<void> updateSystemWindowTheme() async {
  // Set system window theme for macOS.
  final userPreference = MyTheme.getThemeModePreference();
  if (userPreference != ThemeMode.system) {
    if (Platform.isMacOS) {
      await RdPlatformChannel.instance.changeSystemWindowTheme(
          userPreference == ThemeMode.light
              ? SystemWindowTheme.light
              : SystemWindowTheme.dark);
    }
  }
}

/// macOS only
///
/// Note: not found a general solution for rust based AVFoundation bingding.
/// [AVFoundation] crate has compile error.
const kMacOSPermChannel = MethodChannel("org.rustdesk.rustdesk/macos");

enum PermissionAuthorizeType {
  undetermined,
  authorized,
  denied, // and restricted
}

Future<PermissionAuthorizeType> osxCanRecordAudio() async {
  int res = await kMacOSPermChannel.invokeMethod("canRecordAudio");
  print(res);
  if (res > 0) {
    return PermissionAuthorizeType.authorized;
  } else if (res == 0) {
    return PermissionAuthorizeType.undetermined;
  } else {
    return PermissionAuthorizeType.denied;
  }
}

Future<bool> osxRequestAudio() async {
  return await kMacOSPermChannel.invokeMethod("requestRecordAudio");
}

class DraggableNeverScrollableScrollPhysics extends ScrollPhysics {
  /// Creates scroll physics that does not let the user scroll.
  const DraggableNeverScrollableScrollPhysics({super.parent});

  @override
  DraggableNeverScrollableScrollPhysics applyTo(ScrollPhysics? ancestor) {
    return DraggableNeverScrollableScrollPhysics(parent: buildParent(ancestor));
  }

  @override
  bool shouldAcceptUserOffset(ScrollMetrics position) {
    // TODO: find a better solution to check if the offset change is caused by the scrollbar.
    // Workaround: when dragging with the scrollbar, it always triggers an [IdleScrollActivity].
    if (position is ScrollPositionWithSingleContext) {
      // ignore: invalid_use_of_protected_member, invalid_use_of_visible_for_testing_member
      return position.activity is IdleScrollActivity;
    }
    return false;
  }

  @override
  bool get allowImplicitScrolling => false;
}
