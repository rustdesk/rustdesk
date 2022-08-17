import 'dart:async';
import 'dart:io';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:get/instance_manager.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:window_manager/window_manager.dart';
import 'package:back_button_interceptor/back_button_interceptor.dart';

import 'models/model.dart';
import 'models/platform_model.dart';

final globalKey = GlobalKey<NavigatorState>();
final navigationBarKey = GlobalKey();

var isAndroid = Platform.isAndroid;
var isIOS = Platform.isIOS;
var isWeb = false;
var isWebDesktop = false;
var isDesktop = Platform.isWindows || Platform.isMacOS || Platform.isLinux;
var version = "";
int androidVersion = 0;

typedef F = String Function(String);
typedef FMethod = String Function(String, dynamic);

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
  static const Color dark = Colors.black87;

  static ThemeData lightTheme = ThemeData(
    brightness: Brightness.light,
    primarySwatch: Colors.blue,
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: TabBarTheme(labelColor: Colors.black87),
  );
  static ThemeData darkTheme = ThemeData(
      brightness: Brightness.dark,
      primarySwatch: Colors.blue,
      visualDensity: VisualDensity.adaptivePlatformDensity,
      tabBarTheme: TabBarTheme(labelColor: Colors.white70));
}

bool isDarkTheme() {
  final isDark = "Y" == Get.find<SharedPreferences>().getString("darkTheme");
  return isDark;
}

final ButtonStyle flatButtonStyle = TextButton.styleFrom(
  minimumSize: Size(0, 36),
  padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 10.0),
  shape: const RoundedRectangleBorder(
    borderRadius: BorderRadius.all(Radius.circular(2.0)),
  ),
);

closeConnection({String? id}) {
  if (isAndroid || isIOS) {
    Navigator.popUntil(globalKey.currentContext!, ModalRoute.withName("/"));
  } else {
    closeTab(id);
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
  Map<String, Dialog> _dialogs = Map();
  int _tagCount = 0;

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
    show((setState, close) => CustomAlertDialog(
        content: Container(
            constraints: BoxConstraints(maxWidth: 240),
            child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SizedBox(height: 30),
                  Center(child: CircularProgressIndicator()),
                  SizedBox(height: 20),
                  Center(
                      child: Text(translate(text),
                          style: TextStyle(fontSize: 15))),
                  SizedBox(height: 20),
                  Offstage(
                      offstage: !showCancel,
                      child: Center(
                          child: TextButton(
                              style: flatButtonStyle,
                              onPressed: () {
                                dismissAll();
                                if (onCancel != null) {
                                  onCancel();
                                }
                              },
                              child: Text(translate('Cancel'),
                                  style: TextStyle(color: MyTheme.accent)))))
                ]))));
  }
}

void showToast(String text, {Duration timeout = const Duration(seconds: 2)}) {
  final overlayState = globalKey.currentState?.overlay;
  if (overlayState == null) return;
  final entry = OverlayEntry(builder: (_) {
    return IgnorePointer(
        child: Align(
            alignment: Alignment(0.0, 0.8),
            child: Container(
              decoration: BoxDecoration(
                color: Colors.black.withOpacity(0.6),
                borderRadius: BorderRadius.all(
                  Radius.circular(20),
                ),
              ),
              padding: EdgeInsets.symmetric(horizontal: 20, vertical: 5),
              child: Text(
                text,
                style: TextStyle(
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
  CustomAlertDialog(
      {this.title, required this.content, this.actions, this.contentPadding});

  final Widget? title;
  final Widget content;
  final List<Widget>? actions;
  final double? contentPadding;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      scrollable: true,
      title: title,
      contentPadding:
          EdgeInsets.symmetric(horizontal: contentPadding ?? 25, vertical: 10),
      content: content,
      actions: actions,
    );
  }
}

void msgBox(
    String type, String title, String text, OverlayDialogManager dialogManager,
    {bool? hasCancel}) {
  var wrap = (String text, void Function() onPressed) => ButtonTheme(
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

  dialogManager.dismissAll();
  List<Widget> buttons = [];
  if (type != "connecting" && type != "success" && type.indexOf("nook") < 0) {
    buttons.insert(
        0,
        wrap(translate('OK'), () {
          dialogManager.dismissAll();
          closeConnection();
        }));
  }
  if (hasCancel == null) {
    // hasCancel = type != 'error';
    hasCancel = type.indexOf("error") < 0 &&
        type.indexOf("nocancel") < 0 &&
        type != "restarting";
  }
  if (hasCancel) {
    buttons.insert(
        0,
        wrap(translate('Cancel'), () {
          dialogManager.dismissAll();
        }));
  }
  // TODO: test this button
  if (type.indexOf("hasclose") >= 0) {
    buttons.insert(
        0,
        wrap(translate('Close'), () {
          dialogManager.dismissAll();
        }));
  }
  dialogManager.show((setState, close) => CustomAlertDialog(
      title: Text(translate(title), style: TextStyle(fontSize: 21)),
      content: Text(translate(text), style: TextStyle(fontSize: 15)),
      actions: buttons));
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
    return size.toStringAsFixed(2) + " B";
  } else if (size < M) {
    return (size / K).toStringAsFixed(2) + " KB";
  } else if (size < G) {
    return (size / M).toStringAsFixed(2) + " MB";
  } else {
    return (size / G).toStringAsFixed(2) + " GB";
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
    if (!permissions.contains(type))
      return Future.error("Wrong permission!$type");
    return gFFI.invokeMethod("check_permission", type);
  }

  static Future<bool> request(String type) {
    if (!permissions.contains(type))
      return Future.error("Wrong permission!$type");

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
  // trigger connection status updater
  await bind.mainCheckConnectStatus();
  // global shared preference
  await Get.putAsync(() => SharedPreferences.getInstance());
}

String translate(String name) {
  if (name.startsWith('Failed to') && name.contains(': ')) {
    return name.split(': ').map((x) => translate(x)).join(': ');
  }
  return platformFFI.translate(name, localeName);
}

bool option2bool(String key, String value) {
  bool res;
  if (key.startsWith("enable-")) {
    res = value != "N";
  } else if (key.startsWith("allow-") ||
      key == "stop-service" ||
      key == "direct-server" ||
      key == "stop-rendezvous-service") {
    res = value == "Y";
  } else {
    assert(false);
    res = value != "N";
  }
  return res;
}

String bool2option(String key, bool option) {
  String res;
  if (key.startsWith('enable-')) {
    res = option ? '' : 'N';
  } else if (key.startsWith('allow-') ||
      key == "stop-service" ||
      key == "direct-server" ||
      key == "stop-rendezvous-service") {
    res = option ? 'Y' : '';
  } else {
    assert(false);
    res = option ? 'Y' : 'N';
  }
  return res;
}
