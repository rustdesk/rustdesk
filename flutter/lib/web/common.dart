import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'package:back_button_interceptor/back_button_interceptor.dart';
import 'package:uuid/uuid.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:flutter_hbb/web/models/model.dart';
import 'package:flutter_hbb/web/bind.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/utils/platform_channel.dart';
import 'package:flutter_hbb/common/theme.dart';
import 'package:flutter_hbb/web/pages/remote_page.dart';

int? kWindowId;

var isWeb = true;
var isWebDesktop = true;
const isAndroid = false;
const isIOS = false;
const isMobile = false;
const isDesktop = false;
var version = '';
DesktopType? desktopType;

final globalKey = GlobalKey<NavigatorState>();
final navigationBarKey = GlobalKey();

typedef F = String Function(String);
typedef FMethod = String Function(String, dynamic);

typedef StreamEventHandler = Future<void> Function(Map<String, dynamic>);
typedef SessionID = UuidValue;

enum DesktopType {
  main,
  remote,
  fileTransfer,
  cm,
  portForward,
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

class ThemeConfig {
  static ThemeMode getThemeModePreference() {
    return ThemeMode.system;
  }

  static void changeDarkMode(ThemeMode mode) async {
    Get.changeThemeMode(mode);
    updateSystemWindowTheme();
  }
}

closeConnection({String? id}) {}

/// Connect to a peer with [id].
/// If [isFileTransfer], starts a session only for file transfer.
/// If [isTcpTunneling], starts a session only for tcp tunneling.
/// If [isRDP], starts a session only for rdp.
connect(
  BuildContext context,
  String id, {
  bool isFileTransfer = false,
  bool isTcpTunneling = false,
  bool isRDP = false,
  bool forceRelay = false,
}) async {
  if (id == '') return;
  try {
    if (Get.isRegistered<IDTextEditingController>()) {
      final idController = Get.find<IDTextEditingController>();
      idController.text = formatID(id);
    }
    if (Get.isRegistered<TextEditingController>()) {
      final fieldTextEditingController = Get.find<TextEditingController>();
      fieldTextEditingController.text = formatID(id);
    }
  } catch (_) {}
  id = id.replaceAll(' ', '');

  Navigator.push(
    context,
    MaterialPageRoute(
      builder: (BuildContext context) => RemotePage(id: id),
    ),
  );

  FocusScopeNode currentFocus = FocusScope.of(context);
  if (!currentFocus.hasPrimaryFocus) {
    currentFocus.unfocus();
  }
}

bool handleUriLink({List<String>? cmdArgs, Uri? uri, String? uriString}) {
  return false;
}

String translate(String name) {
  if (name.startsWith('Failed to') && name.contains(': ')) {
    return name.split(': ').map((x) => translate(x)).join(': ');
  }
  // TODO: implement translate
  return name;
}

Map<String, String> getHttpHeaders() {
  return {'Authorization': 'Bearer ${mainGetLocalOption(key: 'access_token')}'};
}

List<Locale> supportedLocales = const [
  Locale('en', 'US'),
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
  Locale('es'),
];

/// Global FFI object
late FFI _globalFFI;

FFI get gFFI => _globalFFI;

Future<void> initGlobalFFI() async {
  debugPrint("_globalFFI init");
  _globalFFI = FFI(null);
  debugPrint("_globalFFI init end");
  // after `put`, can also be globally found by Get.find<FFI>();
  Get.put(_globalFFI, permanent: true);
}

// TODO
// - Remove argument "contentPadding", no need for it, all should look the same.
// - Remove "required" for argument "content". See simple confirm dialog "delete peer", only title and actions are used. No need to "content: SizedBox.shrink()".
// - Make dead code alive, transform arguments "onSubmit" and "onCancel" into correspondenting buttons "ConfirmOkButton", "CancelButton".
class CustomAlertDialog extends StatelessWidget {
  const CustomAlertDialog(
      {Key? key,
      this.title,
      this.titlePadding,
      required this.content,
      this.actions,
      this.contentPadding,
      this.contentBoxConstraints = const BoxConstraints(maxWidth: 500),
      this.onSubmit,
      this.onCancel})
      : super(key: key);

  final Widget? title;
  final EdgeInsetsGeometry? titlePadding;
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
    bool tabTapped = false;
    if (isAndroid) gFFI.invokeMethod("enable_soft_keyboard", true);

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
          content: ConstrainedBox(
            constraints: contentBoxConstraints,
            child: content,
          ),
          actions: actions,
          titlePadding: titlePadding ?? MyTheme.dialogTitlePadding(),
          contentPadding:
              MyTheme.dialogContentPadding(actions: actions is List),
          actionsPadding: MyTheme.dialogActionsPadding(),
          buttonPadding: MyTheme.dialogButtonPadding),
    );
  }
}

typedef DialogBuilder = CustomAlertDialog Function(
    StateSetter setState, void Function([dynamic]) close, BuildContext context);

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

final ButtonStyle flatButtonStyle = TextButton.styleFrom(
  minimumSize: Size(0, 36),
  padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 10.0),
  shape: const RoundedRectangleBorder(
    borderRadius: BorderRadius.all(Radius.circular(2.0)),
  ),
);

class OverlayDialogManager {
  final Map<String, Dialog> _dialogs = {};
  var _overlayKeyState = OverlayKeyState();
  int _tagCount = 0;

  RxBool mobileActionsOverlayVisible = false.obs;

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

    dialog.entry = OverlayEntry(builder: (context) {
      bool innerClicked = false;
      return Listener(
          onPointerUp: (_) {
            if (!innerClicked && clickMaskDismiss) {
              close();
            }
            innerClicked = false;
          },
          child: Container(
              color: Theme.of(context).brightness == Brightness.light
                  ? Colors.black12
                  : Colors.black45,
              child: StatefulBuilder(builder: (context, setState) {
                return Listener(
                  onPointerUp: (_) => innerClicked = true,
                  child: builder(setState, close, overlayState.context),
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
      VoidCallback? onCancel,
      String? tag}) {
    if (tag == null) {
      tag = _tagCount.toString();
      _tagCount++;
    }
    show((setState, close, context) {
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
    }, tag: tag);
    return tag;
  }

  bool existing(String tag) {
    return _dialogs.keys.contains(tag);
  }
}

Widget dialogButton(String text,
    {required VoidCallback? onPressed,
    bool isOutline = false,
    Widget? icon,
    TextStyle? style,
    ButtonStyle? buttonStyle}) {
  if (isDesktop) {
    if (isOutline) {
      return icon == null
          ? OutlinedButton(
              onPressed: onPressed,
              child: Text(translate(text), style: style),
            )
          : OutlinedButton.icon(
              icon: icon,
              onPressed: onPressed,
              label: Text(translate(text), style: style),
            );
    } else {
      return icon == null
          ? ElevatedButton(
              style: ElevatedButton.styleFrom(elevation: 0).merge(buttonStyle),
              onPressed: onPressed,
              child: Text(translate(text), style: style),
            )
          : ElevatedButton.icon(
              icon: icon,
              style: ElevatedButton.styleFrom(elevation: 0).merge(buttonStyle),
              onPressed: onPressed,
              label: Text(translate(text), style: style),
            );
    }
  } else {
    return TextButton(
      onPressed: onPressed,
      child: Text(
        translate(text),
        style: style,
      ),
    );
  }
}

typedef ReconnectHandle = Function(OverlayDialogManager, SessionID, bool);

void msgBox(SessionID sessionId, String type, String title, String text,
    String link, OverlayDialogManager dialogManager,
    {bool? hasCancel, ReconnectHandle? reconnect, int? reconnectTimeout}) {
  dialogManager.dismissAll();
  List<Widget> buttons = [];
  bool hasOk = false;
  submit() {
    dialogManager.dismissAll();
    // https://github.com/fufesou/rustdesk/blob/5e9a31340b899822090a3731769ae79c6bf5f3e5/src/ui/common.tis#L263
    if (!type.contains("custom")) {
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
  if (reconnect != null &&
      title == "Connection Error" &&
      reconnectTimeout != null) {
    // `enabled` is used to disable the dialog button once the button is clicked.
    final enabled = true.obs;
    final button = Obx(() => _ReconnectCountDownButton(
          second: reconnectTimeout,
          onPressed: enabled.isTrue
              ? () {
                  // Disable the button
                  enabled.value = false;
                  reconnect(dialogManager, sessionId, false);
                }
              : null,
        ));
    buttons.insert(0, button);
  }
  if (link.isNotEmpty) {
    buttons.insert(0, dialogButton('JumpLink', onPressed: jumplink));
  }
  dialogManager.show(
    (setState, close, context) => CustomAlertDialog(
      title: null,
      content: SelectionArea(child: msgboxContent(type, title, text)),
      actions: buttons,
      onSubmit: hasOk ? submit : null,
      onCancel: hasCancel == true ? cancel : null,
    ),
    tag: '$sessionId-$type-$title-$text-$link',
  );
}

class _ReconnectCountDownButton extends StatefulWidget {
  _ReconnectCountDownButton({
    Key? key,
    required this.second,
    required this.onPressed,
  }) : super(key: key);
  final VoidCallback? onPressed;
  final int second;

  @override
  State<_ReconnectCountDownButton> createState() =>
      _ReconnectCountDownButtonState();
}

class _ReconnectCountDownButtonState extends State<_ReconnectCountDownButton> {
  late int _countdownSeconds = widget.second;

  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _startCountdownTimer();
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _startCountdownTimer() {
    _timer = Timer.periodic(Duration(seconds: 1), (timer) {
      if (_countdownSeconds <= 0) {
        timer.cancel();
      } else {
        setState(() {
          _countdownSeconds--;
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return dialogButton(
      '${translate('Reconnect')} (${_countdownSeconds}s)',
      onPressed: widget.onPressed,
      isOutline: true,
    );
  }
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
  if (type.contains('info')) {
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
  String translateText(String text) {
    if (text.indexOf('Failed') == 0 && text.indexOf(': ') > 0) {
      List<String> words = text.split(': ');
      for (var i = 0; i < words.length; ++i) {
        words[i] = translate(words[i]);
      }
      text = words.join(': ');
    } else {
      List<String> words = text.split(' ');
      if (words.length > 1 && words[0].endsWith('_tip')) {
        words[0] = translate(words[0]);
        final rest = text.substring(words[0].length + 1);
        text = '${words[0]} ${translate(rest)}';
      } else {
        text = translate(text);
      }
    }
    return text;
  }

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
            Text(translateText(text), style: const TextStyle(fontSize: 15)),
          ],
        ),
      ),
    ],
  ).marginOnly(bottom: 12);
}

void msgBoxCommon(OverlayDialogManager dialogManager, String title,
    Widget content, List<Widget> buttons,
    {bool hasCancel = true}) {
  dialogManager.show((setState, close, context) => CustomAlertDialog(
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

// Simple wrapper of built-in types for reference use.
class SimpleWrapper<T> {
  T value;
  SimpleWrapper(this.value);
}

// ignore: must_be_immutable
class ComboBox extends StatelessWidget {
  late final List<String> keys;
  late final List<String> values;
  late final String initialKey;
  late final Function(String key) onChanged;
  late final bool enabled;
  late String current;

  ComboBox({
    Key? key,
    required this.keys,
    required this.values,
    required this.initialKey,
    required this.onChanged,
    this.enabled = true,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var index = keys.indexOf(initialKey);
    if (index < 0) {
      index = 0;
    }
    var ref = values[index].obs;
    current = keys[index];
    return Container(
      decoration: BoxDecoration(
        border: Border.all(
          color: enabled
              ? MyTheme.color(context).border2 ?? MyTheme.border
              : MyTheme.border,
        ),
        borderRadius:
            BorderRadius.circular(8), //border raiuds of dropdown button
      ),
      height: 42, // should be the height of a TextField
      child: Obx(() => DropdownButton<String>(
            isExpanded: true,
            value: ref.value,
            elevation: 16,
            underline: Container(),
            style: TextStyle(
                color: enabled
                    ? Theme.of(context).textTheme.titleMedium?.color
                    : disabledTextColor(context, enabled)),
            icon: const Icon(
              Icons.expand_more_sharp,
              size: 20,
            ).marginOnly(right: 15),
            onChanged: enabled
                ? (String? newValue) {
                    if (newValue != null && newValue != ref.value) {
                      ref.value = newValue;
                      current = newValue;
                      onChanged(keys[values.indexOf(newValue)]);
                    }
                  }
                : null,
            items: values.map<DropdownMenuItem<String>>((String value) {
              return DropdownMenuItem<String>(
                value: value,
                child: Text(
                  value,
                  style: const TextStyle(fontSize: 15),
                  overflow: TextOverflow.ellipsis,
                ).marginOnly(left: 15),
              );
            }).toList(),
          )),
    ).marginOnly(bottom: 5);
  }
}

Color? disabledTextColor(BuildContext context, bool enabled) {
  return enabled
      ? null
      : Theme.of(context).textTheme.titleLarge?.color?.withOpacity(0.6);
}

// TODO move this to mobile/widgets.
// Used only for mobile, pages remote, settings, dialog
// TODO remove argument contentPadding, it’s not used, getToggle() has not
RadioListTile<T> getRadio<T>(
    Widget title, T toValue, T curValue, ValueChanged<T?>? onChange,
    {EdgeInsetsGeometry? contentPadding, bool? dense}) {
  return RadioListTile<T>(
    contentPadding: contentPadding ?? EdgeInsets.zero,
    visualDensity: VisualDensity.compact,
    controlAffinity: ListTileControlAffinity.trailing,
    title: title,
    value: toValue,
    groupValue: curValue,
    onChanged: onChange,
    dense: dense,
  );
}

Color str2color2(String str, {List<int> existing = const []}) {
  Map<String, Color> colorMap = {
    "red": Colors.red,
    "green": Colors.green,
    "blue": Colors.blue,
    "orange": Colors.orange,
    "purple": Colors.purple,
    "grey": Colors.grey,
    "cyan": Colors.cyan,
    "lime": Colors.lime,
    "teal": Colors.teal,
    "pink": Colors.pink[200]!,
    "indigo": Colors.indigo,
    "brown": Colors.brown,
  };
  final color = colorMap[str.toLowerCase()];
  if (color != null) {
    return color.withAlpha(0xFF);
  }
  if (str.toLowerCase() == 'yellow') {
    return Colors.yellow.withAlpha(0xFF);
  }
  var hash = 0;
  for (var i = 0; i < str.length; i++) {
    hash += str.codeUnitAt(i);
  }
  List<Color> colorList = colorMap.values.toList();
  hash = hash % colorList.length;
  var result = colorList[hash].withAlpha(0xFF);
  if (existing.contains(result.value)) {
    Color? notUsed =
        colorList.firstWhereOrNull((e) => !existing.contains(e.value));
    if (notUsed != null) {
      result = notUsed;
    }
  }
  return result;
}

void showToast(String text, {Duration timeout = const Duration(seconds: 3)}) {
  final overlayState = globalKey.currentState?.overlay;
  if (overlayState == null) return;
  final entry = OverlayEntry(builder: (context) {
    return IgnorePointer(
        child: Align(
            alignment: const Alignment(0.0, 0.8),
            child: Container(
              decoration: BoxDecoration(
                color: MyTheme.color(context).toastBg,
                borderRadius: const BorderRadius.all(
                  Radius.circular(20),
                ),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 5),
              child: Text(
                text,
                textAlign: TextAlign.center,
                style: TextStyle(
                    decoration: TextDecoration.none,
                    fontWeight: FontWeight.w300,
                    fontSize: 18,
                    color: MyTheme.color(context).toastText),
              ),
            )));
  });
  overlayState.insert(entry);
  Future.delayed(timeout, () {
    entry.remove();
  });
}

class LoadEvent {
  static const String recent = 'load_recent_peers';
  static const String favorite = 'load_fav_peers';
  static const String lan = 'load_lan_peers';
  static const String addressBook = 'load_address_book_peers';
  static const String group = 'load_group_peers';
}
