import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../../common.dart';
import '../../mobile/widgets/dialog.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../common/shared_state.dart';
import './popup_menu.dart';
import './kb_layout_type_chooser.dart';

const _kKeyLegacyMode = 'legacy';
const _kKeyMapMode = 'map';
const _kKeyTranslateMode = 'translate';

class MenubarState {
  final kStoreKey = 'remoteMenubarState';
  late RxBool show;
  late RxBool _pin;
  RxString viewStyle = RxString(kRemoteViewStyleOriginal);

  MenubarState() {
    final s = bind.getLocalFlutterConfig(k: kStoreKey);
    if (s.isEmpty) {
      _initSet(false, false);
      return;
    }

    try {
      final m = jsonDecode(s);
      if (m == null) {
        _initSet(false, false);
      } else {
        _initSet(m['pin'] ?? false, m['pin'] ?? false);
      }
    } catch (e) {
      debugPrint('Failed to decode menubar state ${e.toString()}');
      _initSet(false, false);
    }
  }

  _initSet(bool s, bool p) {
    // Show remubar when connection is established.
    show = RxBool(true);
    _pin = RxBool(p);
  }

  bool get pin => _pin.value;

  switchShow() async {
    show.value = !show.value;
  }

  setShow(bool v) async {
    if (show.value != v) {
      show.value = v;
    }
  }

  switchPin() async {
    _pin.value = !_pin.value;
    // Save everytime changed, as this func will not be called frequently
    await _savePin();
  }

  setPin(bool v) async {
    if (_pin.value != v) {
      _pin.value = v;
      // Save everytime changed, as this func will not be called frequently
      await _savePin();
    }
  }

  _savePin() async {
    bind.setLocalFlutterConfig(
        k: kStoreKey, v: jsonEncode({'pin': _pin.value}));
  }

  save() async {
    await _savePin();
  }
}

class _MenubarTheme {
  static const Color blueColor = MyTheme.button;
  static const Color hoverBlueColor = MyTheme.accent;
  static const Color redColor = Colors.redAccent;
  static const Color hoverRedColor = Colors.red;
  // kMinInteractiveDimension
  static const double height = 20.0;
  static const double dividerHeight = 12.0;

  static const double buttonSize = 32;
  static const double buttonHMargin = 3;
  static const double buttonVMargin = 6;
  static const double iconRadius = 8;
  static const double elevation = 3;
}

typedef DismissFunc = void Function();

class RemoteMenuEntry {
  static MenuEntryRadios<String> viewStyle(
    String remoteId,
    FFI ffi,
    EdgeInsets padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
    RxString? rxViewStyle,
  }) {
    return MenuEntryRadios<String>(
      text: translate('Ratio'),
      optionsGetter: () => [
        MenuEntryRadioOption(
          text: translate('Scale original'),
          value: kRemoteViewStyleOriginal,
          dismissOnClicked: true,
          dismissCallback: dismissCallback,
        ),
        MenuEntryRadioOption(
          text: translate('Scale adaptive'),
          value: kRemoteViewStyleAdaptive,
          dismissOnClicked: true,
          dismissCallback: dismissCallback,
        ),
      ],
      curOptionGetter: () async {
        // null means peer id is not found, which there's no need to care about
        final viewStyle = await bind.sessionGetViewStyle(id: remoteId) ?? '';
        if (rxViewStyle != null) {
          rxViewStyle.value = viewStyle;
        }
        return viewStyle;
      },
      optionSetter: (String oldValue, String newValue) async {
        await bind.sessionSetViewStyle(id: remoteId, value: newValue);
        if (rxViewStyle != null) {
          rxViewStyle.value = newValue;
        }
        ffi.canvasModel.updateViewStyle();
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: dismissCallback,
    );
  }

  static MenuEntrySwitch2<String> showRemoteCursor(
    String remoteId,
    EdgeInsets padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    final state = ShowRemoteCursorState.find(remoteId);
    final optKey = 'show-remote-cursor';
    return MenuEntrySwitch2<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Show remote cursor'),
      getter: () {
        return state;
      },
      setter: (bool v) async {
        await bind.sessionToggleOption(id: remoteId, value: optKey);
        state.value =
            bind.sessionGetToggleOptionSync(id: remoteId, arg: optKey);
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: dismissCallback,
    );
  }

  static MenuEntrySwitch<String> disableClipboard(
    String remoteId,
    EdgeInsets? padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    return createSwitchMenuEntry(
      remoteId,
      'Disable clipboard',
      'disable-clipboard',
      padding,
      true,
      dismissCallback: dismissCallback,
    );
  }

  static MenuEntrySwitch<String> createSwitchMenuEntry(
    String remoteId,
    String text,
    String option,
    EdgeInsets? padding,
    bool dismissOnClicked, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    return MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate(text),
      getter: () async {
        return bind.sessionGetToggleOptionSync(id: remoteId, arg: option);
      },
      setter: (bool v) async {
        await bind.sessionToggleOption(id: remoteId, value: option);
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: dismissOnClicked,
      dismissCallback: dismissCallback,
    );
  }

  static MenuEntryButton<String> insertLock(
    String remoteId,
    EdgeInsets? padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Insert Lock'),
        style: style,
      ),
      proc: () {
        bind.sessionLockScreen(id: remoteId);
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: dismissCallback,
    );
  }

  static insertCtrlAltDel(
    String remoteId,
    EdgeInsets? padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        '${translate("Insert")} Ctrl + Alt + Del',
        style: style,
      ),
      proc: () {
        bind.sessionCtrlAltDel(id: remoteId);
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: dismissCallback,
    );
  }
}

class RemoteMenubar extends StatefulWidget {
  final String id;
  final FFI ffi;
  final MenubarState state;
  final Function(Function(bool)) onEnterOrLeaveImageSetter;
  final Function() onEnterOrLeaveImageCleaner;

  RemoteMenubar({
    Key? key,
    required this.id,
    required this.ffi,
    required this.state,
    required this.onEnterOrLeaveImageSetter,
    required this.onEnterOrLeaveImageCleaner,
  }) : super(key: key);

  @override
  State<RemoteMenubar> createState() => _RemoteMenubarState();
}

class _RemoteMenubarState extends State<RemoteMenubar> {
  late Debouncer<int> _debouncerHide;
  bool _isCursorOverImage = false;
  final _fractionX = 0.5.obs;
  final _dragging = false.obs;

  int get windowId => stateGlobal.windowId;

  bool get isFullscreen => stateGlobal.fullscreen;
  void _setFullscreen(bool v) {
    stateGlobal.setFullscreen(v);
    setState(() {});
  }

  RxBool get show => widget.state.show;
  bool get pin => widget.state.pin;

  @override
  initState() {
    super.initState();

    _debouncerHide = Debouncer<int>(
      Duration(milliseconds: 5000),
      onChanged: _debouncerHideProc,
      initialValue: 0,
    );

    widget.onEnterOrLeaveImageSetter((enter) {
      if (enter) {
        _debouncerHide.value = 0;
        _isCursorOverImage = true;
      } else {
        _isCursorOverImage = false;
      }
    });
  }

  _debouncerHideProc(int v) {
    if (!pin && show.isTrue && _isCursorOverImage && _dragging.isFalse) {
      show.value = false;
    }
  }

  @override
  dispose() {
    super.dispose();

    widget.onEnterOrLeaveImageCleaner();
  }

  @override
  Widget build(BuildContext context) {
    // No need to use future builder here.
    return Align(
      alignment: Alignment.topCenter,
      child: Obx(() => show.value
          ? _buildMenubar(context)
          : _buildDraggableShowHide(context)),
    );
  }

  Widget _buildDraggableShowHide(BuildContext context) {
    return Obx(() {
      if (show.isTrue && _dragging.isFalse) {
        _debouncerHide.value = 1;
      }
      return Align(
        alignment: FractionalOffset(_fractionX.value, 0),
        child: Offstage(
          offstage: _dragging.isTrue,
          child: Material(
            elevation: _MenubarTheme.elevation,
            child: _DraggableShowHide(
              dragging: _dragging,
              fractionX: _fractionX,
              show: show,
            ),
          ),
        ),
      );
    });
  }

  Widget _buildMenubar(BuildContext context) {
    final List<Widget> menubarItems = [];
    if (!isWebDesktop) {
      menubarItems.add(_PinMenu(state: widget.state));
      menubarItems.add(
          _FullscreenMenu(state: widget.state, setFullscreen: _setFullscreen));
      menubarItems.add(_MobileActionMenu(ffi: widget.ffi));
    }
    menubarItems.add(_MonitorMenu(id: widget.id, ffi: widget.ffi));
    menubarItems
        .add(_ControlMenu(id: widget.id, ffi: widget.ffi, state: widget.state));
    menubarItems.add(_DisplayMenu(
      id: widget.id,
      ffi: widget.ffi,
      state: widget.state,
      setFullscreen: _setFullscreen,
    ));
    menubarItems.add(_KeyboardMenu(id: widget.id, ffi: widget.ffi));
    if (!isWeb) {
      menubarItems.add(_ChatMenu(id: widget.id, ffi: widget.ffi));
      menubarItems.add(_VoiceCallMenu(id: widget.id, ffi: widget.ffi));
    }
    menubarItems.add(_RecordMenu());
    menubarItems.add(_CloseMenu(id: widget.id, ffi: widget.ffi));
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Material(
          elevation: _MenubarTheme.elevation,
          borderRadius: BorderRadius.all(Radius.circular(4.0)),
          color: Theme.of(context)
              .menuBarTheme
              .style
              ?.backgroundColor
              ?.resolve(MaterialState.values.toSet()),
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Theme(
              data: themeData(),
              child: Row(
                children: [
                  SizedBox(width: _MenubarTheme.buttonHMargin * 2),
                  ...menubarItems,
                  SizedBox(width: _MenubarTheme.buttonHMargin * 2)
                ],
              ),
            ),
          ),
        ),
        _buildDraggableShowHide(context),
      ],
    );
  }

  ThemeData themeData() {
    return Theme.of(context).copyWith(
      menuButtonTheme: MenuButtonThemeData(
        style: ButtonStyle(
          minimumSize: MaterialStatePropertyAll(Size(64, 36)),
          textStyle: MaterialStatePropertyAll(
            TextStyle(fontWeight: FontWeight.normal),
          ),
        ),
      ),
      dividerTheme: DividerThemeData(space: 4),
      menuBarTheme: MenuBarThemeData(
          style: MenuStyle(
        padding: MaterialStatePropertyAll(EdgeInsets.zero),
        elevation: MaterialStatePropertyAll(0),
        shape: MaterialStatePropertyAll(BeveledRectangleBorder()),
      ).copyWith(
              backgroundColor:
                  Theme.of(context).menuBarTheme.style?.backgroundColor)),
    );
  }
}

class _PinMenu extends StatelessWidget {
  final MenubarState state;
  const _PinMenu({Key? key, required this.state}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(
      () => _IconMenuButton(
        assetName: state.pin ? "assets/pinned.svg" : "assets/unpinned.svg",
        tooltip: state.pin ? 'Unpin menubar' : 'Pin menubar',
        onPressed: state.switchPin,
        color: state.pin ? _MenubarTheme.blueColor : Colors.grey[800]!,
        hoverColor:
            state.pin ? _MenubarTheme.hoverBlueColor : Colors.grey[850]!,
      ),
    );
  }
}

class _FullscreenMenu extends StatelessWidget {
  final MenubarState state;
  final Function(bool) setFullscreen;
  bool get isFullscreen => stateGlobal.fullscreen;
  const _FullscreenMenu(
      {Key? key, required this.state, required this.setFullscreen})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _IconMenuButton(
      assetName:
          isFullscreen ? "assets/fullscreen_exit.svg" : "assets/fullscreen.svg",
      tooltip: isFullscreen ? 'Exit Fullscreen' : 'Fullscreen',
      onPressed: () => setFullscreen(!isFullscreen),
      color: _MenubarTheme.blueColor,
      hoverColor: _MenubarTheme.hoverBlueColor,
    );
  }
}

class _MobileActionMenu extends StatelessWidget {
  final FFI ffi;
  const _MobileActionMenu({Key? key, required this.ffi}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    if (!ffi.ffiModel.isPeerAndroid) return Offstage();
    return _IconMenuButton(
      assetName: 'assets/actions_mobile.svg',
      tooltip: 'Mobile Actions',
      onPressed: () => ffi.dialogManager.toggleMobileActionsOverlay(ffi: ffi),
      color: _MenubarTheme.blueColor,
      hoverColor: _MenubarTheme.hoverBlueColor,
    );
  }
}

class _MonitorMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  const _MonitorMenu({Key? key, required this.id, required this.ffi})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    if (PrivacyModeState.find(id).isTrue ||
        stateGlobal.displaysCount.value < 2) {
      return Offstage();
    }
    return _IconSubmenuButton(
        tooltip: 'Select Monitor',
        icon: icon(),
        ffi: ffi,
        color: _MenubarTheme.blueColor,
        hoverColor: _MenubarTheme.hoverBlueColor,
        menuStyle: MenuStyle(
            padding:
                MaterialStatePropertyAll(EdgeInsets.symmetric(horizontal: 6))),
        menuChildren: [Row(children: displays(context))]);
  }

  icon() {
    final pi = ffi.ffiModel.pi;
    return Stack(
      alignment: Alignment.center,
      children: [
        SvgPicture.asset(
          "assets/display.svg",
          color: Colors.white,
        ),
        Padding(
          padding: const EdgeInsets.only(bottom: 3.9),
          child: Obx(() {
            RxInt display = CurrentDisplayState.find(id);
            return Text(
              '${display.value + 1}/${pi.displays.length}',
              style: const TextStyle(color: Colors.white, fontSize: 8),
            );
          }),
        )
      ],
    );
  }

  List<Widget> displays(BuildContext context) {
    final List<Widget> rowChildren = [];
    final pi = ffi.ffiModel.pi;
    for (int i = 0; i < pi.displays.length; i++) {
      rowChildren.add(_IconMenuButton(
        topLevel: false,
        color: _MenubarTheme.blueColor,
        hoverColor: _MenubarTheme.hoverBlueColor,
        tooltip: "",
        hMargin: 6,
        vMargin: 12,
        icon: Container(
          alignment: AlignmentDirectional.center,
          constraints: const BoxConstraints(minHeight: _MenubarTheme.height),
          child: Stack(
            alignment: Alignment.center,
            children: [
              SvgPicture.asset(
                "assets/display.svg",
                color: Colors.white,
              ),
              Padding(
                padding: const EdgeInsets.only(bottom: 3.5 /*2.5*/),
                child: Text(
                  (i + 1).toString(),
                  style: TextStyle(
                    color: Colors.white,
                    fontSize: 12,
                  ),
                ),
              )
            ],
          ),
        ),
        onPressed: () {
          _menuDismissCallback(ffi);
          RxInt display = CurrentDisplayState.find(id);
          if (display.value != i) {
            bind.sessionSwitchDisplay(id: id, value: i);
          }
        },
      ));
    }
    return rowChildren;
  }
}

class _ControlMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  final MenubarState state;
  _ControlMenu(
      {Key? key, required this.id, required this.ffi, required this.state})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _IconSubmenuButton(
        tooltip: 'Control Actions',
        svg: "assets/actions.svg",
        color: _MenubarTheme.blueColor,
        hoverColor: _MenubarTheme.hoverBlueColor,
        ffi: ffi,
        menuChildren: [
          requestElevation(),
          osPassword(),
          transferFile(context),
          tcpTunneling(context),
          note(),
          Divider(),
          ctrlAltDel(),
          restart(),
          insertLock(),
          blockUserInput(),
          switchSides(),
          refresh(),
        ]);
  }

  requestElevation() {
    final visible = ffi.elevationModel.showRequestMenu;
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Text(translate('Request Elevation')),
        ffi: ffi,
        onPressed: () => showRequestElevationDialog(id, ffi.dialogManager));
  }

  osPassword() {
    return _MenuItemButton(
        child: Text(translate('OS Password')),
        trailingIcon: Transform.scale(scale: 0.8, child: Icon(Icons.edit)),
        ffi: ffi,
        onPressed: () => _showSetOSPassword(id, false, ffi.dialogManager));
  }

  _showSetOSPassword(
      String id, bool login, OverlayDialogManager dialogManager) async {
    final controller = TextEditingController();
    var password =
        await bind.sessionGetOption(id: id, arg: 'os-password') ?? '';
    var autoLogin =
        await bind.sessionGetOption(id: id, arg: 'auto-login') != '';
    controller.text = password;
    dialogManager.show((setState, close) {
      submit() {
        var text = controller.text.trim();
        bind.sessionPeerOption(id: id, name: 'os-password', value: text);
        bind.sessionPeerOption(
            id: id, name: 'auto-login', value: autoLogin ? 'Y' : '');
        if (text != '' && login) {
          bind.sessionInputOsPassword(id: id, value: text);
        }
        close();
      }

      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.password_rounded, color: MyTheme.accent),
            Text(translate('OS Password')).paddingOnly(left: 10),
          ],
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            PasswordWidget(controller: controller),
            CheckboxListTile(
              contentPadding: const EdgeInsets.all(0),
              dense: true,
              controlAffinity: ListTileControlAffinity.leading,
              title: Text(
                translate('Auto Login'),
              ),
              value: autoLogin,
              onChanged: (v) {
                if (v == null) return;
                setState(() => autoLogin = v);
              },
            ),
          ],
        ),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: close,
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  transferFile(BuildContext context) {
    return _MenuItemButton(
        child: Text(translate('Transfer File')),
        ffi: ffi,
        onPressed: () => connect(context, id, isFileTransfer: true));
  }

  tcpTunneling(BuildContext context) {
    return _MenuItemButton(
        child: Text(translate('TCP Tunneling')),
        ffi: ffi,
        onPressed: () => connect(context, id, isTcpTunneling: true));
  }

  note() {
    final auditServer = bind.sessionGetAuditServerSync(id: id, typ: "conn");
    final visible = auditServer.isNotEmpty;
    if (!visible) return Offstage();
    return _MenuItemButton(
      child: Text(translate('Note')),
      ffi: ffi,
      onPressed: () => _showAuditDialog(id, ffi.dialogManager),
    );
  }

  _showAuditDialog(String id, dialogManager) async {
    final controller = TextEditingController();
    dialogManager.show((setState, close) {
      submit() {
        var text = controller.text.trim();
        if (text != '') {
          bind.sessionSendNote(id: id, note: text);
        }
        close();
      }

      late final focusNode = FocusNode(
        onKey: (FocusNode node, RawKeyEvent evt) {
          if (evt.logicalKey.keyLabel == 'Enter') {
            if (evt is RawKeyDownEvent) {
              int pos = controller.selection.base.offset;
              controller.text =
                  '${controller.text.substring(0, pos)}\n${controller.text.substring(pos)}';
              controller.selection =
                  TextSelection.fromPosition(TextPosition(offset: pos + 1));
            }
            return KeyEventResult.handled;
          }
          if (evt.logicalKey.keyLabel == 'Esc') {
            if (evt is RawKeyDownEvent) {
              close();
            }
            return KeyEventResult.handled;
          } else {
            return KeyEventResult.ignored;
          }
        },
      );

      return CustomAlertDialog(
        title: Text(translate('Note')),
        content: SizedBox(
            width: 250,
            height: 120,
            child: TextField(
              autofocus: true,
              keyboardType: TextInputType.multiline,
              textInputAction: TextInputAction.newline,
              decoration: const InputDecoration.collapsed(
                hintText: 'input note here',
              ),
              maxLines: null,
              maxLength: 256,
              controller: controller,
              focusNode: focusNode,
            )),
        actions: [
          dialogButton('Cancel', onPressed: close, isOutline: true),
          dialogButton('OK', onPressed: submit)
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  ctrlAltDel() {
    final perms = ffi.ffiModel.permissions;
    final pi = ffi.ffiModel.pi;
    final visible = perms['keyboard'] != false &&
        (pi.platform == kPeerPlatformLinux || pi.sasEnabled);
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Text('${translate("Insert")} Ctrl + Alt + Del'),
        ffi: ffi,
        onPressed: () => bind.sessionCtrlAltDel(id: id));
  }

  restart() {
    final perms = ffi.ffiModel.permissions;
    final pi = ffi.ffiModel.pi;
    final visible = perms['restart'] != false &&
        (pi.platform == kPeerPlatformLinux ||
            pi.platform == kPeerPlatformWindows ||
            pi.platform == kPeerPlatformMacOS);
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Text(translate('Restart Remote Device')),
        ffi: ffi,
        onPressed: () => showRestartRemoteDevice(pi, id, ffi.dialogManager));
  }

  insertLock() {
    final perms = ffi.ffiModel.permissions;
    final visible = perms['keyboard'] != false;
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Text(translate('Insert Lock')),
        ffi: ffi,
        onPressed: () => bind.sessionLockScreen(id: id));
  }

  blockUserInput() {
    final perms = ffi.ffiModel.permissions;
    final pi = ffi.ffiModel.pi;
    final visible =
        perms['keyboard'] != false && pi.platform == kPeerPlatformWindows;
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Obx(() => Text(translate(
            '${BlockInputState.find(id).value ? 'Unb' : 'B'}lock user input'))),
        ffi: ffi,
        onPressed: () {
          RxBool blockInput = BlockInputState.find(id);
          bind.sessionToggleOption(
              id: id, value: '${blockInput.value ? 'un' : ''}block-input');
          blockInput.value = !blockInput.value;
        });
  }

  switchSides() {
    final perms = ffi.ffiModel.permissions;
    final pi = ffi.ffiModel.pi;
    final visible = perms['keyboard'] != false &&
        pi.platform != kPeerPlatformAndroid &&
        pi.platform != kPeerPlatformMacOS &&
        version_cmp(pi.version, '1.2.0') >= 0;
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Text(translate('Switch Sides')),
        ffi: ffi,
        onPressed: () => _showConfirmSwitchSidesDialog(id, ffi.dialogManager));
  }

  void _showConfirmSwitchSidesDialog(
      String id, OverlayDialogManager dialogManager) async {
    dialogManager.show((setState, close) {
      submit() async {
        await bind.sessionSwitchSides(id: id);
        closeConnection(id: id);
      }

      return CustomAlertDialog(
        content: msgboxContent('info', 'Switch Sides',
            'Please confirm if you want to share your desktop?'),
        actions: [
          dialogButton('Cancel', onPressed: close, isOutline: true),
          dialogButton('OK', onPressed: submit),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  refresh() {
    final pi = ffi.ffiModel.pi;
    final visible = pi.version.isNotEmpty;
    if (!visible) return Offstage();
    return _MenuItemButton(
        child: Text(translate('Refresh')),
        ffi: ffi,
        onPressed: () => bind.sessionRefresh(id: id));
  }
}

class _DisplayMenu extends StatefulWidget {
  final String id;
  final FFI ffi;
  final MenubarState state;
  final Function(bool) setFullscreen;
  _DisplayMenu(
      {Key? key,
      required this.id,
      required this.ffi,
      required this.state,
      required this.setFullscreen})
      : super(key: key);

  @override
  State<_DisplayMenu> createState() => _DisplayMenuState();
}

class _DisplayMenuState extends State<_DisplayMenu> {
  window_size.Screen? _screen;

  bool get isFullscreen => stateGlobal.fullscreen;

  int get windowId => stateGlobal.windowId;

  Map<String, bool> get perms => widget.ffi.ffiModel.permissions;

  PeerInfo get pi => widget.ffi.ffiModel.pi;

  @override
  Widget build(BuildContext context) {
    _updateScreen();
    return _IconSubmenuButton(
        tooltip: 'Display Settings',
        svg: "assets/display.svg",
        ffi: widget.ffi,
        color: _MenubarTheme.blueColor,
        hoverColor: _MenubarTheme.hoverBlueColor,
        menuChildren: [
          adjustWindow(),
          viewStyle(),
          scrollStyle(),
          imageQuality(),
          codec(),
          resolutions(),
          Divider(),
          showRemoteCursor(),
          zoomCursor(),
          showQualityMonitor(),
          mute(),
          fileCopyAndPaste(),
          disableClipboard(),
          lockAfterSessionEnd(),
          privacyMode(),
          swapKey(),
        ]);
  }

  adjustWindow() {
    final visible = _isWindowCanBeAdjusted();
    if (!visible) return Offstage();
    return Column(
      children: [
        _MenuItemButton(
            child: Text(translate('Adjust Window')),
            onPressed: _doAdjustWindow,
            ffi: widget.ffi),
        Divider(),
      ],
    );
  }

  _doAdjustWindow() async {
    await _updateScreen();
    if (_screen != null) {
      widget.setFullscreen(false);
      double scale = _screen!.scaleFactor;
      final wndRect = await WindowController.fromWindowId(windowId).getFrame();
      final mediaSize = MediaQueryData.fromWindow(ui.window).size;
      // On windows, wndRect is equal to GetWindowRect and mediaSize is equal to GetClientRect.
      // https://stackoverflow.com/a/7561083
      double magicWidth =
          wndRect.right - wndRect.left - mediaSize.width * scale;
      double magicHeight =
          wndRect.bottom - wndRect.top - mediaSize.height * scale;

      final canvasModel = widget.ffi.canvasModel;
      final width = (canvasModel.getDisplayWidth() * canvasModel.scale +
                  CanvasModel.leftToEdge +
                  CanvasModel.rightToEdge) *
              scale +
          magicWidth;
      final height = (canvasModel.getDisplayHeight() * canvasModel.scale +
                  CanvasModel.topToEdge +
                  CanvasModel.bottomToEdge) *
              scale +
          magicHeight;
      double left = wndRect.left + (wndRect.width - width) / 2;
      double top = wndRect.top + (wndRect.height - height) / 2;

      Rect frameRect = _screen!.frame;
      if (!isFullscreen) {
        frameRect = _screen!.visibleFrame;
      }
      if (left < frameRect.left) {
        left = frameRect.left;
      }
      if (top < frameRect.top) {
        top = frameRect.top;
      }
      if ((left + width) > frameRect.right) {
        left = frameRect.right - width;
      }
      if ((top + height) > frameRect.bottom) {
        top = frameRect.bottom - height;
      }
      await WindowController.fromWindowId(windowId)
          .setFrame(Rect.fromLTWH(left, top, width, height));
    }
  }

  _updateScreen() async {
    final v = await rustDeskWinManager.call(
        WindowType.Main, kWindowGetWindowInfo, '');
    final String valueStr = v;
    if (valueStr.isEmpty) {
      _screen = null;
    } else {
      final screenMap = jsonDecode(valueStr);
      _screen = window_size.Screen(
          Rect.fromLTRB(screenMap['frame']['l'], screenMap['frame']['t'],
              screenMap['frame']['r'], screenMap['frame']['b']),
          Rect.fromLTRB(
              screenMap['visibleFrame']['l'],
              screenMap['visibleFrame']['t'],
              screenMap['visibleFrame']['r'],
              screenMap['visibleFrame']['b']),
          screenMap['scaleFactor']);
    }
  }

  _isWindowCanBeAdjusted() {
    if (widget.state.viewStyle.value != kRemoteViewStyleOriginal) {
      return false;
    }
    final remoteCount = RemoteCountState.find().value;
    if (remoteCount != 1) {
      return false;
    }
    if (_screen == null) {
      return false;
    }
    final scale = kIgnoreDpi ? 1.0 : _screen!.scaleFactor;
    double selfWidth = _screen!.visibleFrame.width;
    double selfHeight = _screen!.visibleFrame.height;
    if (isFullscreen) {
      selfWidth = _screen!.frame.width;
      selfHeight = _screen!.frame.height;
    }

    final canvasModel = widget.ffi.canvasModel;
    final displayWidth = canvasModel.getDisplayWidth();
    final displayHeight = canvasModel.getDisplayHeight();
    final requiredWidth =
        CanvasModel.leftToEdge + displayWidth + CanvasModel.rightToEdge;
    final requiredHeight =
        CanvasModel.topToEdge + displayHeight + CanvasModel.bottomToEdge;
    return selfWidth > (requiredWidth * scale) &&
        selfHeight > (requiredHeight * scale);
  }

  viewStyle() {
    return futureBuilder(future: () async {
      final viewStyle = await bind.sessionGetViewStyle(id: widget.id) ?? '';
      widget.state.viewStyle.value = viewStyle;
      return viewStyle;
    }(), hasData: (data) {
      final groupValue = data as String;
      onChanged(String? value) async {
        if (value == null) return;
        await bind.sessionSetViewStyle(id: widget.id, value: value);
        widget.state.viewStyle.value = value;
        widget.ffi.canvasModel.updateViewStyle();
      }

      return Column(children: [
        _RadioMenuButton<String>(
          child: Text(translate('Scale original')),
          value: kRemoteViewStyleOriginal,
          groupValue: groupValue,
          onChanged: onChanged,
          ffi: widget.ffi,
        ),
        _RadioMenuButton<String>(
          child: Text(translate('Scale adaptive')),
          value: kRemoteViewStyleAdaptive,
          groupValue: groupValue,
          onChanged: onChanged,
          ffi: widget.ffi,
        ),
        Divider(),
      ]);
    });
  }

  scrollStyle() {
    final visible = widget.state.viewStyle.value == kRemoteViewStyleOriginal;
    if (!visible) return Offstage();
    return futureBuilder(future: () async {
      final scrollStyle = await bind.sessionGetScrollStyle(id: widget.id) ?? '';
      return scrollStyle;
    }(), hasData: (data) {
      final groupValue = data as String;
      onChange(String? value) async {
        if (value == null) return;
        await bind.sessionSetScrollStyle(id: widget.id, value: value);
        widget.ffi.canvasModel.updateScrollStyle();
      }

      final enabled = widget.ffi.canvasModel.imageOverflow.value;
      return Column(children: [
        _RadioMenuButton<String>(
          child: Text(translate('ScrollAuto')),
          value: kRemoteScrollStyleAuto,
          groupValue: groupValue,
          onChanged: enabled ? (value) => onChange(value) : null,
          ffi: widget.ffi,
        ),
        _RadioMenuButton<String>(
          child: Text(translate('Scrollbar')),
          value: kRemoteScrollStyleBar,
          groupValue: groupValue,
          onChanged: enabled ? (value) => onChange(value) : null,
          ffi: widget.ffi,
        ),
        Divider(),
      ]);
    });
  }

  imageQuality() {
    return futureBuilder(future: () async {
      final imageQuality =
          await bind.sessionGetImageQuality(id: widget.id) ?? '';
      return imageQuality;
    }(), hasData: (data) {
      final groupValue = data as String;
      onChanged(String? value) async {
        if (value == null) return;
        await bind.sessionSetImageQuality(id: widget.id, value: value);
      }

      return _SubmenuButton(
        ffi: widget.ffi,
        child: Text(translate('Image Quality')),
        menuChildren: [
          _RadioMenuButton<String>(
            child: Text(translate('Good image quality')),
            value: kRemoteImageQualityBest,
            groupValue: groupValue,
            onChanged: onChanged,
            ffi: widget.ffi,
          ),
          _RadioMenuButton<String>(
            child: Text(translate('Balanced')),
            value: kRemoteImageQualityBalanced,
            groupValue: groupValue,
            onChanged: onChanged,
            ffi: widget.ffi,
          ),
          _RadioMenuButton<String>(
            child: Text(translate('Optimize reaction time')),
            value: kRemoteImageQualityLow,
            groupValue: groupValue,
            onChanged: onChanged,
            ffi: widget.ffi,
          ),
          _RadioMenuButton<String>(
            child: Text(translate('Custom')),
            value: kRemoteImageQualityCustom,
            groupValue: groupValue,
            onChanged: (value) {
              onChanged(value);
              _customImageQualityDialog();
            },
            ffi: widget.ffi,
          ),
        ],
      );
    });
  }

  _customImageQualityDialog() async {
    double qualityInitValue = 50;
    double fpsInitValue = 30;
    bool qualitySet = false;
    bool fpsSet = false;
    setCustomValues({double? quality, double? fps}) async {
      if (quality != null) {
        qualitySet = true;
        await bind.sessionSetCustomImageQuality(
            id: widget.id, value: quality.toInt());
      }
      if (fps != null) {
        fpsSet = true;
        await bind.sessionSetCustomFps(id: widget.id, fps: fps.toInt());
      }
      if (!qualitySet) {
        qualitySet = true;
        await bind.sessionSetCustomImageQuality(
            id: widget.id, value: qualityInitValue.toInt());
      }
      if (!fpsSet) {
        fpsSet = true;
        await bind.sessionSetCustomFps(
            id: widget.id, fps: fpsInitValue.toInt());
      }
    }

    final btnClose = dialogButton('Close', onPressed: () async {
      await setCustomValues();
      widget.ffi.dialogManager.dismissAll();
    });

    // quality
    final quality = await bind.sessionGetCustomImageQuality(id: widget.id);
    qualityInitValue =
        quality != null && quality.isNotEmpty ? quality[0].toDouble() : 50.0;
    const qualityMinValue = 10.0;
    const qualityMaxValue = 100.0;
    if (qualityInitValue < qualityMinValue) {
      qualityInitValue = qualityMinValue;
    }
    if (qualityInitValue > qualityMaxValue) {
      qualityInitValue = qualityMaxValue;
    }
    final RxDouble qualitySliderValue = RxDouble(qualityInitValue);
    final debouncerQuality = Debouncer<double>(
      Duration(milliseconds: 1000),
      onChanged: (double v) {
        setCustomValues(quality: v);
      },
      initialValue: qualityInitValue,
    );
    final qualitySlider = Obx(() => Row(
          children: [
            Slider(
              value: qualitySliderValue.value,
              min: qualityMinValue,
              max: qualityMaxValue,
              divisions: 18,
              onChanged: (double value) {
                qualitySliderValue.value = value;
                debouncerQuality.value = value;
              },
            ),
            SizedBox(
                width: 40,
                child: Text(
                  '${qualitySliderValue.value.round()}%',
                  style: const TextStyle(fontSize: 15),
                )),
            SizedBox(
                width: 50,
                child: Text(
                  translate('Bitrate'),
                  style: const TextStyle(fontSize: 15),
                ))
          ],
        ));
    // fps
    final fpsOption =
        await bind.sessionGetOption(id: widget.id, arg: 'custom-fps');
    fpsInitValue = fpsOption == null ? 30 : double.tryParse(fpsOption) ?? 30;
    if (fpsInitValue < 10 || fpsInitValue > 120) {
      fpsInitValue = 30;
    }
    final RxDouble fpsSliderValue = RxDouble(fpsInitValue);
    final debouncerFps = Debouncer<double>(
      Duration(milliseconds: 1000),
      onChanged: (double v) {
        setCustomValues(fps: v);
      },
      initialValue: qualityInitValue,
    );
    bool? direct;
    try {
      direct = ConnectionTypeState.find(widget.id).direct.value ==
          ConnectionType.strDirect;
    } catch (_) {}
    final fpsSlider = Offstage(
      offstage: (await bind.mainIsUsingPublicServer() && direct != true) ||
          version_cmp(pi.version, '1.2.0') < 0,
      child: Row(
        children: [
          Obx((() => Slider(
                value: fpsSliderValue.value,
                min: 10,
                max: 120,
                divisions: 22,
                onChanged: (double value) {
                  fpsSliderValue.value = value;
                  debouncerFps.value = value;
                },
              ))),
          SizedBox(
              width: 40,
              child: Obx(() => Text(
                    '${fpsSliderValue.value.round()}',
                    style: const TextStyle(fontSize: 15),
                  ))),
          SizedBox(
              width: 50,
              child: Text(
                translate('FPS'),
                style: const TextStyle(fontSize: 15),
              ))
        ],
      ),
    );

    final content = Column(
      children: [qualitySlider, fpsSlider],
    );
    msgBoxCommon(
        widget.ffi.dialogManager, 'Custom Image Quality', content, [btnClose]);
  }

  codec() {
    return futureBuilder(future: () async {
      final supportedHwcodec =
          await bind.sessionSupportedHwcodec(id: widget.id);
      final codecPreference =
          await bind.sessionGetOption(id: widget.id, arg: 'codec-preference') ??
              '';
      return {
        'supportedHwcodec': supportedHwcodec,
        'codecPreference': codecPreference
      };
    }(), hasData: (data) {
      final List<bool> codecs = [];
      try {
        final Map codecsJson = jsonDecode(data['supportedHwcodec']);
        final h264 = codecsJson['h264'] ?? false;
        final h265 = codecsJson['h265'] ?? false;
        codecs.add(h264);
        codecs.add(h265);
      } catch (e) {
        debugPrint("Show Codec Preference err=$e");
      }
      final visible = bind.mainHasHwcodec() &&
          codecs.length == 2 &&
          (codecs[0] || codecs[1]);
      if (!visible) return Offstage();
      final groupValue = data['codecPreference'] as String;
      onChanged(String? value) async {
        if (value == null) return;
        await bind.sessionPeerOption(
            id: widget.id, name: 'codec-preference', value: value);
        bind.sessionChangePreferCodec(id: widget.id);
      }

      return _SubmenuButton(
          ffi: widget.ffi,
          child: Text(translate('Codec')),
          menuChildren: [
            _RadioMenuButton<String>(
              child: Text(translate('Auto')),
              value: 'auto',
              groupValue: groupValue,
              onChanged: onChanged,
              ffi: widget.ffi,
            ),
            _RadioMenuButton<String>(
              child: Text(translate('VP9')),
              value: 'vp9',
              groupValue: groupValue,
              onChanged: onChanged,
              ffi: widget.ffi,
            ),
            _RadioMenuButton<String>(
              child: Text(translate('H264')),
              value: 'h264',
              groupValue: groupValue,
              onChanged: onChanged,
              ffi: widget.ffi,
            ),
            _RadioMenuButton<String>(
              child: Text(translate('H265')),
              value: 'h265',
              groupValue: groupValue,
              onChanged: onChanged,
              ffi: widget.ffi,
            ),
          ]);
    });
  }

  resolutions() {
    final resolutions = widget.ffi.ffiModel.pi.resolutions;
    final visible = widget.ffi.ffiModel.permissions["keyboard"] != false &&
        resolutions.length > 1;
    if (!visible) return Offstage();
    final display = widget.ffi.ffiModel.display;
    final groupValue = "${display.width}x${display.height}";
    onChanged(String? value) async {
      if (value == null) return;
      final list = value.split('x');
      if (list.length == 2) {
        final w = int.tryParse(list[0]);
        final h = int.tryParse(list[1]);
        if (w != null && h != null) {
          await bind.sessionChangeResolution(
              id: widget.id, width: w, height: h);
          Future.delayed(Duration(seconds: 3), () async {
            final display = widget.ffi.ffiModel.display;
            if (w == display.width && h == display.height) {
              if (_isWindowCanBeAdjusted()) {
                _doAdjustWindow();
              }
            }
          });
        }
      }
    }

    return _SubmenuButton(
        ffi: widget.ffi,
        menuChildren: resolutions
            .map((e) => _RadioMenuButton(
                value: '${e.width}x${e.height}',
                groupValue: groupValue,
                onChanged: onChanged,
                ffi: widget.ffi,
                child: Text('${e.width}x${e.height}')))
            .toList(),
        child: Text(translate("Resolution")));
  }

  showRemoteCursor() {
    if (widget.ffi.ffiModel.pi.platform == kPeerPlatformAndroid) {
      return Offstage();
    }
    final visible = !widget.ffi.canvasModel.cursorEmbedded;
    if (!visible) return Offstage();
    final state = ShowRemoteCursorState.find(widget.id);
    final option = 'show-remote-cursor';
    return _CheckboxMenuButton(
        value: state.value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(id: widget.id, value: option);
          state.value =
              bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Show remote cursor')));
  }

  zoomCursor() {
    if (widget.ffi.ffiModel.pi.platform == kPeerPlatformAndroid) {
      return Offstage();
    }
    final visible = widget.state.viewStyle.value != kRemoteViewStyleOriginal;
    if (!visible) return Offstage();
    final option = 'zoom-cursor';
    final peerState = PeerBoolOption.find(widget.id, option);
    return _CheckboxMenuButton(
        value: peerState.value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(id: widget.id, value: option);
          peerState.value =
              bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Zoom cursor')));
  }

  showQualityMonitor() {
    final option = 'show-quality-monitor';
    final value = bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
    return _CheckboxMenuButton(
        value: value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(id: widget.id, value: option);
          widget.ffi.qualityMonitorModel.checkShowQualityMonitor(widget.id);
        },
        ffi: widget.ffi,
        child: Text(translate('Show quality monitor')));
  }

  mute() {
    final visible = perms['audio'] != false;
    if (!visible) return Offstage();
    final option = 'disable-audio';
    final value = bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
    return _CheckboxMenuButton(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(id: widget.id, value: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Mute')));
  }

  fileCopyAndPaste() {
    final visible = Platform.isWindows &&
        pi.platform == kPeerPlatformWindows &&
        perms['file'] != false;
    if (!visible) return Offstage();
    final option = 'enable-file-transfer';
    final value = bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
    return _CheckboxMenuButton(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(id: widget.id, value: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Allow file copy and paste')));
  }

  disableClipboard() {
    final visible = perms['keyboard'] != false && perms['clipboard'] != false;
    if (!visible) return Offstage();
    final option = 'disable-clipboard';
    final value = bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
    return _CheckboxMenuButton(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(id: widget.id, value: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Disable clipboard')));
  }

  lockAfterSessionEnd() {
    final visible = perms['keyboard'] != false;
    if (!visible) return Offstage();
    final option = 'lock-after-session-end';
    final value = bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
    return _CheckboxMenuButton(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(id: widget.id, value: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Lock after session end')));
  }

  privacyMode() {
    bool visible = perms['keyboard'] != false && pi.features.privacyMode;
    if (!visible) return Offstage();
    final option = 'privacy-mode';
    final rxValue = PrivacyModeState.find(widget.id);
    return _CheckboxMenuButton(
        value: rxValue.value,
        onChanged: (value) {
          if (value == null) return;
          if (widget.ffi.ffiModel.pi.currentDisplay != 0) {
            msgBox(
                widget.id,
                'custom-nook-nocancel-hasclose',
                'info',
                'Please switch to Display 1 first',
                '',
                widget.ffi.dialogManager);
            return;
          }
          bind.sessionToggleOption(id: widget.id, value: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Privacy mode')));
  }

  swapKey() {
    final visible = perms['keyboard'] != false &&
        ((Platform.isMacOS && pi.platform != kPeerPlatformMacOS) ||
            (!Platform.isMacOS && pi.platform == kPeerPlatformMacOS));
    if (!visible) return Offstage();
    final option = 'allow_swap_key';
    final value = bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
    return _CheckboxMenuButton(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(id: widget.id, value: option);
        },
        ffi: widget.ffi,
        child: Text(translate('Swap control-command key')));
  }
}

class _KeyboardMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  _KeyboardMenu({
    Key? key,
    required this.id,
    required this.ffi,
  }) : super(key: key);

  PeerInfo get pi => ffi.ffiModel.pi;

  @override
  Widget build(BuildContext context) {
    var ffiModel = Provider.of<FfiModel>(context);
    if (ffiModel.permissions['keyboard'] == false) return Offstage();
    if (stateGlobal.grabKeyboard) {
      if (bind.sessionIsKeyboardModeSupported(id: id, mode: _kKeyMapMode)) {
        bind.sessionSetKeyboardMode(id: id, value: _kKeyMapMode);
      } else if (bind.sessionIsKeyboardModeSupported(
          id: id, mode: _kKeyLegacyMode)) {
        bind.sessionSetKeyboardMode(id: id, value: _kKeyLegacyMode);
      }
      return Offstage();
    }
    return _IconSubmenuButton(
        tooltip: 'Keyboard Settings',
        svg: "assets/keyboard.svg",
        ffi: ffi,
        color: _MenubarTheme.blueColor,
        hoverColor: _MenubarTheme.hoverBlueColor,
        menuChildren: [mode(), localKeyboardType()]);
  }

  mode() {
    return futureBuilder(future: () async {
      return await bind.sessionGetKeyboardMode(id: id) ?? _kKeyLegacyMode;
    }(), hasData: (data) {
      final groupValue = data as String;
      List<KeyboardModeMenu> modes = [
        KeyboardModeMenu(key: _kKeyLegacyMode, menu: 'Legacy mode'),
        KeyboardModeMenu(key: _kKeyMapMode, menu: 'Map mode'),
        KeyboardModeMenu(key: _kKeyTranslateMode, menu: 'Translate mode'),
      ];
      List<_RadioMenuButton> list = [];
      onChanged(String? value) async {
        if (value == null) return;
        await bind.sessionSetKeyboardMode(id: id, value: value);
      }

      for (KeyboardModeMenu mode in modes) {
        if (bind.sessionIsKeyboardModeSupported(id: id, mode: mode.key)) {
          if (mode.key == _kKeyTranslateMode) {
            if (Platform.isLinux || pi.platform == kPeerPlatformLinux) {
              continue;
            }
          }
          var text = translate(mode.menu);
          if (mode.key == _kKeyTranslateMode) {
            text = '$text beta';
          }
          list.add(_RadioMenuButton<String>(
            child: Text(text),
            value: mode.key,
            groupValue: groupValue,
            onChanged: onChanged,
            ffi: ffi,
          ));
        }
      }
      return Column(children: list);
    });
  }

  localKeyboardType() {
    final localPlatform = getLocalPlatformForKBLayoutType(pi.platform);
    final visible = localPlatform != '';
    if (!visible) return Offstage();
    return Column(
      children: [
        Divider(),
        _MenuItemButton(
          child: Text(
              '${translate('Local keyboard type')}: ${KBLayoutType.value}'),
          trailingIcon: const Icon(Icons.settings),
          ffi: ffi,
          onPressed: () =>
              showKBLayoutTypeChooser(localPlatform, ffi.dialogManager),
        )
      ],
    );
  }
}

class _ChatMenu extends StatefulWidget {
  final String id;
  final FFI ffi;
  _ChatMenu({
    Key? key,
    required this.id,
    required this.ffi,
  }) : super(key: key);

  @override
  State<_ChatMenu> createState() => _ChatMenuState();
}

class _ChatMenuState extends State<_ChatMenu> {
  // Using in StatelessWidget got `Looking up a deactivated widget's ancestor is unsafe`.
  final chatButtonKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    return _IconSubmenuButton(
        tooltip: 'Chat',
        key: chatButtonKey,
        svg: 'assets/chat.svg',
        ffi: widget.ffi,
        color: _MenubarTheme.blueColor,
        hoverColor: _MenubarTheme.hoverBlueColor,
        menuChildren: [textChat(), voiceCall()]);
  }

  textChat() {
    return _MenuItemButton(
        child: Text(translate('Text chat')),
        ffi: widget.ffi,
        onPressed: () {
          RenderBox? renderBox =
              chatButtonKey.currentContext?.findRenderObject() as RenderBox?;

          Offset? initPos;
          if (renderBox != null) {
            final pos = renderBox.localToGlobal(Offset.zero);
            initPos = Offset(pos.dx, pos.dy + _MenubarTheme.dividerHeight);
          }

          widget.ffi.chatModel.changeCurrentID(ChatModel.clientModeID);
          widget.ffi.chatModel.toggleChatOverlay(chatInitPos: initPos);
        });
  }

  voiceCall() {
    return _MenuItemButton(
      child: Text(translate('Voice call')),
      ffi: widget.ffi,
      onPressed: () => bind.sessionRequestVoiceCall(id: widget.id),
    );
  }
}

class _VoiceCallMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  _VoiceCallMenu({
    Key? key,
    required this.id,
    required this.ffi,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(
      () {
        final String tooltip;
        final String icon;
        switch (ffi.chatModel.voiceCallStatus.value) {
          case VoiceCallStatus.waitingForResponse:
            tooltip = "Waiting";
            icon = "assets/call_wait.svg";
            break;
          case VoiceCallStatus.connected:
            tooltip = "Disconnect";
            icon = "assets/call_end.svg";
            break;
          default:
            return Offstage();
        }
        return _IconMenuButton(
            assetName: icon,
            tooltip: tooltip,
            onPressed: () => bind.sessionCloseVoiceCall(id: id),
            color: _MenubarTheme.redColor,
            hoverColor: _MenubarTheme.hoverRedColor);
      },
    );
  }
}

class _RecordMenu extends StatelessWidget {
  const _RecordMenu({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var ffi = Provider.of<FfiModel>(context);
    final visible = ffi.permissions['recording'] != false;
    if (!visible) return Offstage();
    return Consumer<RecordingModel>(
      builder: (context, value, child) => _IconMenuButton(
        assetName: 'assets/rec.svg',
        tooltip:
            value.start ? 'Stop session recording' : 'Start session recording',
        onPressed: () => value.toggle(),
        color: value.start ? _MenubarTheme.redColor : _MenubarTheme.blueColor,
        hoverColor: value.start
            ? _MenubarTheme.hoverRedColor
            : _MenubarTheme.hoverBlueColor,
      ),
    );
  }
}

class _CloseMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  const _CloseMenu({Key? key, required this.id, required this.ffi})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _IconMenuButton(
      assetName: 'assets/close.svg',
      tooltip: 'Close',
      onPressed: () => clientClose(id, ffi.dialogManager),
      color: _MenubarTheme.redColor,
      hoverColor: _MenubarTheme.hoverRedColor,
    );
  }
}

class _IconMenuButton extends StatefulWidget {
  final String? assetName;
  final Widget? icon;
  final String? tooltip;
  final Color color;
  final Color hoverColor;
  final VoidCallback? onPressed;
  final double? hMargin;
  final double? vMargin;
  final bool topLevel;
  const _IconMenuButton({
    Key? key,
    this.assetName,
    this.icon,
    this.tooltip,
    required this.color,
    required this.hoverColor,
    required this.onPressed,
    this.hMargin,
    this.vMargin,
    this.topLevel = true,
  }) : super(key: key);

  @override
  State<_IconMenuButton> createState() => _IconMenuButtonState();
}

class _IconMenuButtonState extends State<_IconMenuButton> {
  bool hover = false;

  @override
  Widget build(BuildContext context) {
    assert(widget.assetName != null || widget.icon != null);
    final icon = widget.icon ??
        SvgPicture.asset(
          widget.assetName!,
          color: Colors.white,
          width: _MenubarTheme.buttonSize,
          height: _MenubarTheme.buttonSize,
        );
    final button = SizedBox(
      width: _MenubarTheme.buttonSize,
      height: _MenubarTheme.buttonSize,
      child: MenuItemButton(
        style: ButtonStyle(
            backgroundColor: MaterialStatePropertyAll(Colors.transparent),
            padding: MaterialStatePropertyAll(EdgeInsets.zero),
            overlayColor: MaterialStatePropertyAll(Colors.transparent)),
        onHover: (value) => setState(() {
          hover = value;
        }),
        onPressed: widget.onPressed,
        child: Material(
            type: MaterialType.transparency,
            child: Ink(
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(_MenubarTheme.iconRadius),
                  color: hover ? widget.hoverColor : widget.color,
                ),
                child: icon)),
      ),
    ).marginSymmetric(
        horizontal: widget.hMargin ?? _MenubarTheme.buttonHMargin,
        vertical: widget.vMargin ?? _MenubarTheme.buttonVMargin);
    if (widget.topLevel) {
      return MenuBar(children: [button]);
    } else {
      return button;
    }
  }
}

class _IconSubmenuButton extends StatefulWidget {
  final String tooltip;
  final String? svg;
  final Widget? icon;
  final Color color;
  final Color hoverColor;
  final List<Widget> menuChildren;
  final MenuStyle? menuStyle;
  final FFI ffi;

  _IconSubmenuButton(
      {Key? key,
      this.svg,
      this.icon,
      required this.tooltip,
      required this.color,
      required this.hoverColor,
      required this.menuChildren,
      required this.ffi,
      this.menuStyle})
      : super(key: key);

  @override
  State<_IconSubmenuButton> createState() => _IconSubmenuButtonState();
}

class _IconSubmenuButtonState extends State<_IconSubmenuButton> {
  bool hover = false;

  @override
  Widget build(BuildContext context) {
    assert(widget.svg != null || widget.icon != null);
    final icon = widget.icon ??
        SvgPicture.asset(
          widget.svg!,
          color: Colors.white,
          width: _MenubarTheme.buttonSize,
          height: _MenubarTheme.buttonSize,
        );
    final button = SizedBox(
        width: _MenubarTheme.buttonSize,
        height: _MenubarTheme.buttonSize,
        child: SubmenuButton(
            menuStyle: widget.menuStyle,
            style: ButtonStyle(
                backgroundColor: MaterialStatePropertyAll(Colors.transparent),
                padding: MaterialStatePropertyAll(EdgeInsets.zero),
                overlayColor: MaterialStatePropertyAll(Colors.transparent)),
            onHover: (value) => setState(() {
                  hover = value;
                }),
            child: Material(
                type: MaterialType.transparency,
                child: Ink(
                    decoration: BoxDecoration(
                      borderRadius:
                          BorderRadius.circular(_MenubarTheme.iconRadius),
                      color: hover ? widget.hoverColor : widget.color,
                    ),
                    child: icon)),
            menuChildren: widget.menuChildren
                .map((e) => _buildPointerTrackWidget(e, widget.ffi))
                .toList()));
    return MenuBar(children: [
      button.marginSymmetric(
          horizontal: _MenubarTheme.buttonHMargin,
          vertical: _MenubarTheme.buttonVMargin)
    ]);
  }
}

class _SubmenuButton extends StatelessWidget {
  final List<Widget> menuChildren;
  final Widget? child;
  final FFI ffi;
  const _SubmenuButton({
    Key? key,
    required this.menuChildren,
    required this.child,
    required this.ffi,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return SubmenuButton(
      key: key,
      child: child,
      menuChildren:
          menuChildren.map((e) => _buildPointerTrackWidget(e, ffi)).toList(),
    );
  }
}

class _MenuItemButton extends StatelessWidget {
  final VoidCallback? onPressed;
  final Widget? trailingIcon;
  final Widget? child;
  final FFI ffi;
  _MenuItemButton(
      {Key? key,
      this.onPressed,
      this.trailingIcon,
      required this.child,
      required this.ffi})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MenuItemButton(
        key: key,
        onPressed: onPressed != null
            ? () {
                _menuDismissCallback(ffi);
                onPressed?.call();
              }
            : null,
        trailingIcon: trailingIcon,
        child: child);
  }
}

class _CheckboxMenuButton extends StatelessWidget {
  final bool? value;
  final ValueChanged<bool?>? onChanged;
  final Widget? child;
  final FFI ffi;
  const _CheckboxMenuButton(
      {Key? key,
      required this.value,
      required this.onChanged,
      required this.child,
      required this.ffi})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return CheckboxMenuButton(
      key: key,
      value: value,
      child: child,
      onChanged: onChanged != null
          ? (bool? value) {
              _menuDismissCallback(ffi);
              onChanged?.call(value);
            }
          : null,
    );
  }
}

class _RadioMenuButton<T> extends StatelessWidget {
  final T value;
  final T? groupValue;
  final ValueChanged<T?>? onChanged;
  final Widget? child;
  final FFI ffi;
  const _RadioMenuButton(
      {Key? key,
      required this.value,
      required this.groupValue,
      required this.onChanged,
      required this.child,
      required this.ffi})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return RadioMenuButton(
      value: value,
      groupValue: groupValue,
      child: child,
      onChanged: onChanged != null
          ? (T? value) {
              _menuDismissCallback(ffi);
              onChanged?.call(value);
            }
          : null,
    );
  }
}

class _DraggableShowHide extends StatefulWidget {
  final RxDouble fractionX;
  final RxBool dragging;
  final RxBool show;
  const _DraggableShowHide({
    Key? key,
    required this.fractionX,
    required this.dragging,
    required this.show,
  }) : super(key: key);

  @override
  State<_DraggableShowHide> createState() => _DraggableShowHideState();
}

class _DraggableShowHideState extends State<_DraggableShowHide> {
  Offset position = Offset.zero;
  Size size = Size.zero;

  Widget _buildDraggable(BuildContext context) {
    return Draggable(
      axis: Axis.horizontal,
      child: Icon(
        Icons.drag_indicator,
        size: 20,
        color: MyTheme.color(context).drag_indicator,
      ),
      feedback: widget,
      onDragStarted: (() {
        final RenderObject? renderObj = context.findRenderObject();
        if (renderObj != null) {
          final RenderBox renderBox = renderObj as RenderBox;
          size = renderBox.size;
          position = renderBox.localToGlobal(Offset.zero);
        }
        widget.dragging.value = true;
      }),
      onDragEnd: (details) {
        final mediaSize = MediaQueryData.fromWindow(ui.window).size;
        widget.fractionX.value +=
            (details.offset.dx - position.dx) / (mediaSize.width - size.width);
        if (widget.fractionX.value < 0.35) {
          widget.fractionX.value = 0.35;
        }
        if (widget.fractionX.value > 0.65) {
          widget.fractionX.value = 0.65;
        }
        widget.dragging.value = false;
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final ButtonStyle buttonStyle = ButtonStyle(
      minimumSize: MaterialStateProperty.all(const Size(0, 0)),
      padding: MaterialStateProperty.all(EdgeInsets.zero),
    );
    final child = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _buildDraggable(context),
        TextButton(
          onPressed: () => setState(() {
            widget.show.value = !widget.show.value;
          }),
          child: Obx((() => Icon(
                widget.show.isTrue ? Icons.expand_less : Icons.expand_more,
                size: 20,
              ))),
        ),
      ],
    );
    return TextButtonTheme(
      data: TextButtonThemeData(style: buttonStyle),
      child: Container(
        decoration: BoxDecoration(
          color: Theme.of(context)
              .menuBarTheme
              .style
              ?.backgroundColor
              ?.resolve(MaterialState.values.toSet()),
          borderRadius: BorderRadius.vertical(
            bottom: Radius.circular(5),
          ),
        ),
        child: SizedBox(
          height: 20,
          child: child,
        ),
      ),
    );
  }
}

class KeyboardModeMenu {
  final String key;
  final String menu;

  KeyboardModeMenu({required this.key, required this.menu});
}

_menuDismissCallback(FFI ffi) => ffi.inputModel.refreshMousePos();

Widget _buildPointerTrackWidget(Widget child, FFI ffi) {
  return Listener(
    onPointerHover: (PointerHoverEvent e) =>
        ffi.inputModel.lastMousePos = e.position,
    child: MouseRegion(
      child: child,
    ),
  );
}
