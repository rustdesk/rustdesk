import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/widgets/menu_button.dart';
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
import './material_mod_popup_menu.dart' as mod_menu;
import './kb_layout_type_chooser.dart';

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

  const RemoteMenubar({
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
  window_size.Screen? _screen;
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
    _updateScreen();
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
          child: _DraggableShowHide(
            dragging: _dragging,
            fractionX: _fractionX,
            show: show,
          ),
        ),
      );
    });
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

  Widget _buildPointerTrackWidget(Widget child) {
    return Listener(
      onPointerHover: (PointerHoverEvent e) =>
          widget.ffi.inputModel.lastMousePos = e.position,
      child: MouseRegion(
        child: child,
      ),
    );
  }

  _menuDismissCallback() => widget.ffi.inputModel.refreshMousePos();

  Widget _buildMenubar(BuildContext context) {
    final List<Widget> menubarItems = [];
    if (!isWebDesktop) {
      menubarItems.add(_buildPinMenubar(context));
      menubarItems.add(_buildFullscreen(context));
      if (widget.ffi.ffiModel.isPeerAndroid) {
        menubarItems.add(IconButton(
          tooltip: translate('Mobile Actions'),
          color: _MenubarTheme.blueColor,
          icon: const Icon(Icons.build),
          onPressed: () {
            widget.ffi.dialogManager
                .toggleMobileActionsOverlay(ffi: widget.ffi);
          },
        ));
      }
    }
    menubarItems.add(_buildMonitor(context));
    menubarItems.add(_buildControl(context));
    menubarItems.add(_buildDisplay(context));
    menubarItems.add(_buildKeyboard(context));
    if (!isWeb) {
      menubarItems.add(_buildChat(context));
      menubarItems.add(_buildVoiceCall(context));
    }
    menubarItems.add(_buildRecording(context));
    menubarItems.add(_buildClose(context));
    return PopupMenuTheme(
      data: const PopupMenuThemeData(
          textStyle: TextStyle(color: _MenubarTheme.blueColor)),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.vertical(
                bottom: Radius.circular(10),
              ),
            ),
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SizedBox(width: 3),
                  ...menubarItems,
                  SizedBox(width: 3)
                ],
              ),
            ),
          ),
          _buildDraggableShowHide(context),
        ],
      ),
    );
  }

  Widget _buildPinMenubar(BuildContext context) {
    return Obx(
      () => MenuButton(
        tooltip: translate(pin ? 'Unpin menubar' : 'Pin menubar'),
        onPressed: () {
          widget.state.switchPin();
        },
        icon: SvgPicture.asset(
          pin ? "assets/pinned.svg" : "assets/unpinned.svg",
          color: Colors.white,
        ),
        color: pin ? _MenubarTheme.blueColor : Colors.grey[800]!,
        hoverColor: pin ? _MenubarTheme.hoverBlueColor : Colors.grey[850]!,
      ),
    );
  }

  Widget _buildFullscreen(BuildContext context) {
    return MenuButton(
      tooltip: translate(isFullscreen ? 'Exit Fullscreen' : 'Fullscreen'),
      onPressed: () {
        _setFullscreen(!isFullscreen);
      },
      icon: SvgPicture.asset(
        isFullscreen ? "assets/fullscreen_exit.svg" : "assets/fullscreen.svg",
        color: Colors.white,
      ),
      color: _MenubarTheme.blueColor,
      hoverColor: _MenubarTheme.hoverBlueColor,
    );
  }

  Widget _buildMonitor(BuildContext context) {
    final pi = widget.ffi.ffiModel.pi;
    return mod_menu.PopupMenuButton(
      tooltip: translate('Select Monitor'),
      position: mod_menu.PopupMenuPosition.under,
      icon: Stack(
        alignment: Alignment.center,
        children: [
          SvgPicture.asset(
            "assets/display.svg",
            color: Colors.white,
          ),
          Padding(
            padding: const EdgeInsets.only(bottom: 3.9),
            child: Obx(() {
              RxInt display = CurrentDisplayState.find(widget.id);
              return Text(
                '${display.value + 1}/${pi.displays.length}',
                style: const TextStyle(color: Colors.white, fontSize: 8),
              );
            }),
          )
        ],
      ),
      itemBuilder: (BuildContext context) {
        final List<Widget> rowChildren = [];
        for (int i = 0; i < pi.displays.length; i++) {
          rowChildren.add(
            Stack(
              alignment: Alignment.center,
              children: [
                SvgPicture.asset(
                  "assets/display.svg",
                  color: Colors.white,
                ),
                TextButton(
                  child: Container(
                    alignment: AlignmentDirectional.center,
                    constraints:
                        const BoxConstraints(minHeight: _MenubarTheme.height),
                    child: Padding(
                      padding: const EdgeInsets.only(bottom: 2.5),
                      child: Text(
                        (i + 1).toString(),
                        style: TextStyle(
                          color: Colors.white,
                        ),
                      ),
                    ),
                  ),
                  onPressed: () {
                    if (Navigator.canPop(context)) {
                      Navigator.pop(context);
                      _menuDismissCallback();
                    }
                    RxInt display = CurrentDisplayState.find(widget.id);
                    if (display.value != i) {
                      bind.sessionSwitchDisplay(id: widget.id, value: i);
                    }
                  },
                )
              ],
            ),
          );
        }
        return <mod_menu.PopupMenuEntry<String>>[
          mod_menu.PopupMenuItem<String>(
            height: _MenubarTheme.height,
            padding: EdgeInsets.zero,
            child: _buildPointerTrackWidget(
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: rowChildren,
              ),
            ),
          )
        ];
      },
    );
  }

  Widget _buildControl(BuildContext context) {
    return mod_menu.PopupMenuButton(
      padding: EdgeInsets.zero,
      icon: SvgPicture.asset(
        "assets/actions.svg",
        color: Colors.white,
      ),
      tooltip: translate('Control Actions'),
      position: mod_menu.PopupMenuPosition.under,
      itemBuilder: (BuildContext context) => _getControlMenu(context)
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenubarTheme.blueColor,
                height: _MenubarTheme.height,
                dividerHeight: _MenubarTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  Widget _buildDisplay(BuildContext context) {
    return FutureBuilder(future: () async {
      widget.state.viewStyle.value =
          await bind.sessionGetViewStyle(id: widget.id) ?? '';
      final supportedHwcodec =
          await bind.sessionSupportedHwcodec(id: widget.id);
      return {'supportedHwcodec': supportedHwcodec};
    }(), builder: (context, snapshot) {
      if (snapshot.hasData) {
        return Obx(() {
          final remoteCount = RemoteCountState.find().value;
          return mod_menu.PopupMenuButton(
            padding: EdgeInsets.zero,
            icon: SvgPicture.asset(
              "assets/display.svg",
              color: Colors.white,
            ),
            tooltip: translate('Display Settings'),
            position: mod_menu.PopupMenuPosition.under,
            menuWrapper: _buildPointerTrackWidget,
            itemBuilder: (BuildContext context) =>
                _getDisplayMenu(snapshot.data!, remoteCount)
                    .map((entry) => entry.build(
                        context,
                        const MenuConfig(
                          commonColor: _MenubarTheme.blueColor,
                          height: _MenubarTheme.height,
                          dividerHeight: _MenubarTheme.dividerHeight,
                        )))
                    .expand((i) => i)
                    .toList(),
          );
        });
      } else {
        return const Offstage();
      }
    });
  }

  Widget _buildKeyboard(BuildContext context) {
    FfiModel ffiModel = Provider.of<FfiModel>(context);
    if (ffiModel.permissions['keyboard'] == false) {
      return Offstage();
    }
    return mod_menu.PopupMenuButton(
      padding: EdgeInsets.zero,
      icon: SvgPicture.asset(
        "assets/keyboard.svg",
        color: Colors.white,
      ),
      tooltip: translate('Keyboard Settings'),
      position: mod_menu.PopupMenuPosition.under,
      itemBuilder: (BuildContext context) => _getKeyboardMenu()
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenubarTheme.blueColor,
                height: _MenubarTheme.height,
                dividerHeight: _MenubarTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  Widget _buildRecording(BuildContext context) {
    return Consumer<FfiModel>(builder: ((context, value, child) {
      if (value.permissions['recording'] != false) {
        return Consumer<RecordingModel>(
          builder: (context, value, child) => MenuButton(
            tooltip: value.start
                ? translate('Stop session recording')
                : translate('Start session recording'),
            onPressed: () => value.toggle(),
            icon: SvgPicture.asset(
              "assets/rec.svg",
              color: Colors.white,
            ),
            color:
                value.start ? _MenubarTheme.redColor : _MenubarTheme.blueColor,
            hoverColor: value.start
                ? _MenubarTheme.hoverRedColor
                : _MenubarTheme.hoverBlueColor,
          ),
        );
      } else {
        return Offstage();
      }
    }));
  }

  Widget _buildClose(BuildContext context) {
    return MenuButton(
      tooltip: translate('Close'),
      onPressed: () {
        clientClose(widget.id, widget.ffi.dialogManager);
      },
      icon: SvgPicture.asset(
        "assets/close.svg",
        color: Colors.white,
      ),
      color: _MenubarTheme.redColor,
      hoverColor: _MenubarTheme.hoverRedColor,
    );
  }

  final _chatButtonKey = GlobalKey();
  Widget _buildChat(BuildContext context) {
    FfiModel ffiModel = Provider.of<FfiModel>(context);
    return mod_menu.PopupMenuButton(
      key: _chatButtonKey,
      padding: EdgeInsets.zero,
      icon: SvgPicture.asset(
        "assets/chat.svg",
        color: Colors.white,
      ),
      tooltip: translate('Chat'),
      position: mod_menu.PopupMenuPosition.under,
      itemBuilder: (BuildContext context) => _getChatMenu(context)
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenubarTheme.blueColor,
                height: _MenubarTheme.height,
                dividerHeight: _MenubarTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  Widget _getVoiceCallIcon() {
    switch (widget.ffi.chatModel.voiceCallStatus.value) {
      case VoiceCallStatus.waitingForResponse:
        return SvgPicture.asset(
          "assets/call_wait.svg",
          color: Colors.white,
        );

      case VoiceCallStatus.connected:
        return SvgPicture.asset(
          "assets/call_end.svg",
          color: Colors.white,
        );
      default:
        return const Offstage();
    }
  }

  String? _getVoiceCallTooltip() {
    switch (widget.ffi.chatModel.voiceCallStatus.value) {
      case VoiceCallStatus.waitingForResponse:
        return "Waiting";
      case VoiceCallStatus.connected:
        return "Disconnect";
      default:
        return null;
    }
  }

  Widget _buildVoiceCall(BuildContext context) {
    return Obx(
      () {
        final tooltipText = _getVoiceCallTooltip();
        return tooltipText == null
            ? const Offstage()
            : MenuButton(
                icon: _getVoiceCallIcon(),
                tooltip: translate(tooltipText),
                onPressed: () => bind.sessionCloseVoiceCall(id: widget.id),
                color: _MenubarTheme.redColor,
                hoverColor: _MenubarTheme.hoverRedColor,
              );
      },
    );
  }

  List<MenuEntryBase<String>> _getChatMenu(BuildContext context) {
    final List<MenuEntryBase<String>> chatMenu = [];
    const EdgeInsets padding = EdgeInsets.only(left: 14.0, right: 5.0);
    chatMenu.addAll([
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Text chat'),
          style: style,
        ),
        proc: () {
          RenderBox? renderBox =
              _chatButtonKey.currentContext?.findRenderObject() as RenderBox?;

          Offset? initPos;
          if (renderBox != null) {
            final pos = renderBox.localToGlobal(Offset.zero);
            initPos = Offset(pos.dx, pos.dy + _MenubarTheme.dividerHeight);
          }

          widget.ffi.chatModel.changeCurrentID(ChatModel.clientModeID);
          widget.ffi.chatModel.toggleChatOverlay(chatInitPos: initPos);
        },
        padding: padding,
        dismissOnClicked: true,
      ),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Voice call'),
          style: style,
        ),
        proc: () {
          // Request a voice call.
          bind.sessionRequestVoiceCall(id: widget.id);
        },
        padding: padding,
        dismissOnClicked: true,
      ),
    ]);
    return chatMenu;
  }

  List<MenuEntryBase<String>> _getControlMenu(BuildContext context) {
    final pi = widget.ffi.ffiModel.pi;
    final perms = widget.ffi.ffiModel.permissions;
    final peer_version = widget.ffi.ffiModel.pi.version;
    const EdgeInsets padding = EdgeInsets.only(left: 14.0, right: 5.0);
    final List<MenuEntryBase<String>> displayMenu = [];
    displayMenu.addAll([
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Container(
            alignment: AlignmentDirectional.center,
            height: _MenubarTheme.height,
            child: Row(
              children: [
                Text(
                  translate('OS Password'),
                  style: style,
                ),
                Expanded(
                    child: Align(
                  alignment: Alignment.centerRight,
                  child: Transform.scale(
                      scale: 0.8,
                      child: IconButton(
                          padding: EdgeInsets.zero,
                          icon: const Icon(Icons.edit),
                          onPressed: () {
                            if (Navigator.canPop(context)) {
                              Navigator.pop(context);
                              _menuDismissCallback();
                            }
                            showSetOSPassword(
                                widget.id, false, widget.ffi.dialogManager);
                          })),
                ))
              ],
            )),
        proc: () {
          showSetOSPassword(widget.id, false, widget.ffi.dialogManager);
        },
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: _menuDismissCallback,
      ),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Transfer File'),
          style: style,
        ),
        proc: () {
          connect(context, widget.id, isFileTransfer: true);
        },
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: _menuDismissCallback,
      ),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('TCP Tunneling'),
          style: style,
        ),
        padding: padding,
        proc: () {
          connect(context, widget.id, isTcpTunneling: true);
        },
        dismissOnClicked: true,
        dismissCallback: _menuDismissCallback,
      ),
    ]);
    // {handler.get_audit_server() && <li #note>{translate('Note')}</li>}
    final auditServer =
        bind.sessionGetAuditServerSync(id: widget.id, typ: "conn");
    if (auditServer.isNotEmpty) {
      displayMenu.add(
        MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Text(
            translate('Note'),
            style: style,
          ),
          proc: () {
            showAuditDialog(widget.id, widget.ffi.dialogManager);
          },
          padding: padding,
          dismissOnClicked: true,
          dismissCallback: _menuDismissCallback,
        ),
      );
    }
    displayMenu.add(MenuEntryDivider());
    if (perms['keyboard'] != false) {
      if (pi.platform == kPeerPlatformLinux || pi.sasEnabled) {
        displayMenu.add(RemoteMenuEntry.insertCtrlAltDel(widget.id, padding,
            dismissCallback: _menuDismissCallback));
      }
    }
    if (perms['restart'] != false &&
        (pi.platform == kPeerPlatformLinux ||
            pi.platform == kPeerPlatformWindows ||
            pi.platform == kPeerPlatformMacOS)) {
      displayMenu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Restart Remote Device'),
          style: style,
        ),
        proc: () {
          showRestartRemoteDevice(pi, widget.id, gFFI.dialogManager);
        },
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: _menuDismissCallback,
      ));
    }

    if (perms['keyboard'] != false) {
      displayMenu.add(RemoteMenuEntry.insertLock(widget.id, padding,
          dismissCallback: _menuDismissCallback));

      if (pi.platform == kPeerPlatformWindows) {
        displayMenu.add(MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Obx(() => Text(
                translate(
                    '${BlockInputState.find(widget.id).value ? 'Unb' : 'B'}lock user input'),
                style: style,
              )),
          proc: () {
            RxBool blockInput = BlockInputState.find(widget.id);
            bind.sessionToggleOption(
                id: widget.id,
                value: '${blockInput.value ? 'un' : ''}block-input');
            blockInput.value = !blockInput.value;
          },
          padding: padding,
          dismissOnClicked: true,
          dismissCallback: _menuDismissCallback,
        ));
      }
      if (pi.platform != kPeerPlatformAndroid &&
          pi.platform != kPeerPlatformMacOS && // unsupport yet
          version_cmp(peer_version, '1.2.0') >= 0) {
        displayMenu.add(MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Text(
            translate('Switch Sides'),
            style: style,
          ),
          proc: () =>
              showConfirmSwitchSidesDialog(widget.id, widget.ffi.dialogManager),
          padding: padding,
          dismissOnClicked: true,
          dismissCallback: _menuDismissCallback,
        ));
      }
    }

    if (pi.version.isNotEmpty) {
      displayMenu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Refresh'),
          style: style,
        ),
        proc: () {
          bind.sessionRefresh(id: widget.id);
        },
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: _menuDismissCallback,
      ));
    }

    if (!isWebDesktop) {
      //   if (perms['keyboard'] != false && perms['clipboard'] != false) {
      //     displayMenu.add(MenuEntryButton<String>(
      //       childBuilder: (TextStyle? style) => Text(
      //         translate('Paste'),
      //         style: style,
      //       ),
      //       proc: () {
      //         () async {
      //           ClipboardData? data =
      //               await Clipboard.getData(Clipboard.kTextPlain);
      //           if (data != null && data.text != null) {
      //             bind.sessionInputString(id: widget.id, value: data.text ?? '');
      //           }
      //         }();
      //       },
      //       padding: padding,
      //       dismissOnClicked: true,
      //       dismissCallback: _menuDismissCallback,
      //     ));
      //   }
    }
    return displayMenu;
  }

  bool _isWindowCanBeAdjusted(int remoteCount) {
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
    final requiredWidth = displayWidth +
        (canvasModel.tabBarHeight + canvasModel.windowBorderWidth * 2);
    final requiredHeight = displayHeight +
        (canvasModel.tabBarHeight + canvasModel.windowBorderWidth * 2);
    return selfWidth > (requiredWidth * scale) &&
        selfHeight > (requiredHeight * scale);
  }

  List<MenuEntryBase<String>> _getDisplayMenu(
      dynamic futureData, int remoteCount) {
    const EdgeInsets padding = EdgeInsets.only(left: 18.0, right: 8.0);
    final peer_version = widget.ffi.ffiModel.pi.version;
    final displayMenu = [
      RemoteMenuEntry.viewStyle(
        widget.id,
        widget.ffi,
        padding,
        dismissCallback: _menuDismissCallback,
        rxViewStyle: widget.state.viewStyle,
      ),
      MenuEntryDivider<String>(),
      MenuEntryRadios<String>(
        text: translate('Image Quality'),
        optionsGetter: () => [
          MenuEntryRadioOption(
            text: translate('Good image quality'),
            value: kRemoteImageQualityBest,
            dismissOnClicked: true,
            dismissCallback: _menuDismissCallback,
          ),
          MenuEntryRadioOption(
            text: translate('Balanced'),
            value: kRemoteImageQualityBalanced,
            dismissOnClicked: true,
            dismissCallback: _menuDismissCallback,
          ),
          MenuEntryRadioOption(
            text: translate('Optimize reaction time'),
            value: kRemoteImageQualityLow,
            dismissOnClicked: true,
            dismissCallback: _menuDismissCallback,
          ),
          MenuEntryRadioOption(
            text: translate('Custom'),
            value: kRemoteImageQualityCustom,
            dismissOnClicked: true,
            dismissCallback: _menuDismissCallback,
          ),
        ],
        curOptionGetter: () async =>
            // null means peer id is not found, which there's no need to care about
            await bind.sessionGetImageQuality(id: widget.id) ?? '',
        optionSetter: (String oldValue, String newValue) async {
          if (oldValue != newValue) {
            await bind.sessionSetImageQuality(id: widget.id, value: newValue);
          }

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

          if (newValue == kRemoteImageQualityCustom) {
            final btnClose = dialogButton('Close', onPressed: () async {
              await setCustomValues();
              widget.ffi.dialogManager.dismissAll();
            });

            // quality
            final quality =
                await bind.sessionGetCustomImageQuality(id: widget.id);
            qualityInitValue = quality != null && quality.isNotEmpty
                ? quality[0].toDouble()
                : 50.0;
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
            fpsInitValue =
                fpsOption == null ? 30 : double.tryParse(fpsOption) ?? 30;
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
              offstage:
                  (await bind.mainIsUsingPublicServer() && direct != true) ||
                      version_cmp(peer_version, '1.2.0') < 0,
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
            msgBoxCommon(widget.ffi.dialogManager, 'Custom Image Quality',
                content, [btnClose]);
          }
        },
        padding: padding,
      ),
      MenuEntryDivider<String>(),
    ];

    if (widget.state.viewStyle.value == kRemoteViewStyleOriginal) {
      displayMenu.insert(
          2,
          MenuEntryRadios<String>(
            text: translate('Scroll Style'),
            optionsGetter: () => [
              MenuEntryRadioOption(
                text: translate('ScrollAuto'),
                value: kRemoteScrollStyleAuto,
                dismissOnClicked: true,
                dismissCallback: _menuDismissCallback,
                enabled: widget.ffi.canvasModel.imageOverflow,
              ),
              MenuEntryRadioOption(
                text: translate('Scrollbar'),
                value: kRemoteScrollStyleBar,
                dismissOnClicked: true,
                dismissCallback: _menuDismissCallback,
                enabled: widget.ffi.canvasModel.imageOverflow,
              ),
            ],
            curOptionGetter: () async =>
                // null means peer id is not found, which there's no need to care about
                await bind.sessionGetScrollStyle(id: widget.id) ?? '',
            optionSetter: (String oldValue, String newValue) async {
              await bind.sessionSetScrollStyle(id: widget.id, value: newValue);
              widget.ffi.canvasModel.updateScrollStyle();
            },
            padding: padding,
            dismissOnClicked: true,
            dismissCallback: _menuDismissCallback,
          ));
      displayMenu.insert(3, MenuEntryDivider<String>());

      if (_isWindowCanBeAdjusted(remoteCount)) {
        displayMenu.insert(
          0,
          MenuEntryDivider<String>(),
        );
        displayMenu.insert(
          0,
          MenuEntryButton<String>(
            childBuilder: (TextStyle? style) => Container(
                child: Text(
              translate('Adjust Window'),
              style: style,
            )),
            proc: () {
              () async {
                await _updateScreen();
                if (_screen != null) {
                  _setFullscreen(false);
                  double scale = _screen!.scaleFactor;
                  final wndRect =
                      await WindowController.fromWindowId(windowId).getFrame();
                  final mediaSize = MediaQueryData.fromWindow(ui.window).size;
                  // On windows, wndRect is equal to GetWindowRect and mediaSize is equal to GetClientRect.
                  // https://stackoverflow.com/a/7561083
                  double magicWidth =
                      wndRect.right - wndRect.left - mediaSize.width * scale;
                  double magicHeight =
                      wndRect.bottom - wndRect.top - mediaSize.height * scale;

                  final canvasModel = widget.ffi.canvasModel;
                  final width =
                      (canvasModel.getDisplayWidth() * canvasModel.scale +
                                  canvasModel.windowBorderWidth * 2) *
                              scale +
                          magicWidth;
                  final height =
                      (canvasModel.getDisplayHeight() * canvasModel.scale +
                                  canvasModel.tabBarHeight +
                                  canvasModel.windowBorderWidth * 2) *
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
              }();
            },
            padding: padding,
            dismissOnClicked: true,
            dismissCallback: _menuDismissCallback,
          ),
        );
      }
    }

    /// Show Codec Preference
    if (bind.mainHasHwcodec()) {
      final List<bool> codecs = [];
      try {
        final Map codecsJson = jsonDecode(futureData['supportedHwcodec']);
        final h264 = codecsJson['h264'] ?? false;
        final h265 = codecsJson['h265'] ?? false;
        codecs.add(h264);
        codecs.add(h265);
      } catch (e) {
        debugPrint("Show Codec Preference err=$e");
      }
      if (codecs.length == 2 && (codecs[0] || codecs[1])) {
        displayMenu.add(MenuEntryRadios<String>(
          text: translate('Codec Preference'),
          optionsGetter: () {
            final list = [
              MenuEntryRadioOption(
                text: translate('Auto'),
                value: 'auto',
                dismissOnClicked: true,
                dismissCallback: _menuDismissCallback,
              ),
              MenuEntryRadioOption(
                text: 'VP9',
                value: 'vp9',
                dismissOnClicked: true,
                dismissCallback: _menuDismissCallback,
              ),
            ];
            if (codecs[0]) {
              list.add(MenuEntryRadioOption(
                text: 'H264',
                value: 'h264',
                dismissOnClicked: true,
                dismissCallback: _menuDismissCallback,
              ));
            }
            if (codecs[1]) {
              list.add(MenuEntryRadioOption(
                text: 'H265',
                value: 'h265',
                dismissOnClicked: true,
                dismissCallback: _menuDismissCallback,
              ));
            }
            return list;
          },
          curOptionGetter: () async =>
              // null means peer id is not found, which there's no need to care about
              await bind.sessionGetOption(
                  id: widget.id, arg: 'codec-preference') ??
              '',
          optionSetter: (String oldValue, String newValue) async {
            await bind.sessionPeerOption(
                id: widget.id, name: 'codec-preference', value: newValue);
            bind.sessionChangePreferCodec(id: widget.id);
          },
          padding: padding,
          dismissOnClicked: true,
          dismissCallback: _menuDismissCallback,
        ));
      }
    }
    displayMenu.add(MenuEntryDivider());

    /// Show remote cursor
    if (!widget.ffi.canvasModel.cursorEmbedded) {
      displayMenu.add(RemoteMenuEntry.showRemoteCursor(
        widget.id,
        padding,
        dismissCallback: _menuDismissCallback,
      ));
    }

    /// Show remote cursor scaling with image
    if (widget.state.viewStyle.value != kRemoteViewStyleOriginal) {
      displayMenu.add(() {
        final opt = 'zoom-cursor';
        final state = PeerBoolOption.find(widget.id, opt);
        return MenuEntrySwitch2<String>(
          switchType: SwitchType.scheckbox,
          text: translate('Zoom cursor'),
          getter: () {
            return state;
          },
          setter: (bool v) async {
            await bind.sessionToggleOption(id: widget.id, value: opt);
            state.value =
                bind.sessionGetToggleOptionSync(id: widget.id, arg: opt);
          },
          padding: padding,
          dismissOnClicked: true,
          dismissCallback: _menuDismissCallback,
        );
      }());
    }

    /// Show quality monitor
    displayMenu.add(MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Show quality monitor'),
      getter: () async {
        return bind.sessionGetToggleOptionSync(
            id: widget.id, arg: 'show-quality-monitor');
      },
      setter: (bool v) async {
        await bind.sessionToggleOption(
            id: widget.id, value: 'show-quality-monitor');
        widget.ffi.qualityMonitorModel.checkShowQualityMonitor(widget.id);
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: _menuDismissCallback,
    ));

    final perms = widget.ffi.ffiModel.permissions;
    final pi = widget.ffi.ffiModel.pi;

    if (perms['audio'] != false) {
      displayMenu
          .add(_createSwitchMenuEntry('Mute', 'disable-audio', padding, true));
    }

    if (Platform.isWindows &&
        pi.platform == kPeerPlatformWindows &&
        perms['file'] != false) {
      displayMenu.add(_createSwitchMenuEntry(
          'Allow file copy and paste', 'enable-file-transfer', padding, true));
    }

    if (perms['keyboard'] != false) {
      if (perms['clipboard'] != false) {
        displayMenu.add(RemoteMenuEntry.disableClipboard(
          widget.id,
          padding,
          dismissCallback: _menuDismissCallback,
        ));
      }
      displayMenu.add(_createSwitchMenuEntry(
          'Lock after session end', 'lock-after-session-end', padding, true));
      if (pi.features.privacyMode) {
        displayMenu.add(MenuEntrySwitch2<String>(
          switchType: SwitchType.scheckbox,
          text: translate('Privacy mode'),
          getter: () {
            return PrivacyModeState.find(widget.id);
          },
          setter: (bool v) async {
            await bind.sessionToggleOption(
                id: widget.id, value: 'privacy-mode');
          },
          padding: padding,
          dismissOnClicked: true,
          dismissCallback: _menuDismissCallback,
        ));
      }
    }
    return displayMenu;
  }

  List<MenuEntryBase<String>> _getKeyboardMenu() {
    final List<MenuEntryBase<String>> keyboardMenu = [
      MenuEntryRadios<String>(
        text: translate('Ratio'),
        optionsGetter: () {
          List<MenuEntryRadioOption> list = [];
          List<KeyboardModeMenu> modes = [
            KeyboardModeMenu(key: 'legacy', menu: 'Legacy mode'),
            KeyboardModeMenu(key: 'map', menu: 'Map mode'),
            KeyboardModeMenu(key: 'translate', menu: 'Translate mode'),
          ];

          for (KeyboardModeMenu mode in modes) {
            if (bind.sessionIsKeyboardModeSupported(
                id: widget.id, mode: mode.key)) {
              if (mode.key == 'translate') {
                if (Platform.isLinux ||
                    widget.ffi.ffiModel.pi.platform == kPeerPlatformLinux) {
                  continue;
                }
              }
              var text = translate(mode.menu);
              if (mode.key == 'translate') {
                text = '$text beta';
              }
              list.add(MenuEntryRadioOption(text: text, value: mode.key));
            }
          }
          return list;
        },
        curOptionGetter: () async {
          return await bind.sessionGetKeyboardMode(id: widget.id) ?? 'legacy';
        },
        optionSetter: (String oldValue, String newValue) async {
          await bind.sessionSetKeyboardMode(id: widget.id, value: newValue);
        },
      )
    ];
    final localPlatform =
        getLocalPlatformForKBLayoutType(widget.ffi.ffiModel.pi.platform);
    if (localPlatform != '') {
      keyboardMenu.add(MenuEntryDivider());
      keyboardMenu.add(
        MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Container(
              alignment: AlignmentDirectional.center,
              height: _MenubarTheme.height,
              child: Row(
                children: [
                  Obx(() => RichText(
                        text: TextSpan(
                          text: '${translate('Local keyboard type')}: ',
                          style: DefaultTextStyle.of(context).style,
                          children: <TextSpan>[
                            TextSpan(
                              text: KBLayoutType.value,
                              style: TextStyle(fontWeight: FontWeight.bold),
                            ),
                          ],
                        ),
                      )),
                  Expanded(
                      child: Align(
                    alignment: Alignment.centerRight,
                    child: Transform.scale(
                      scale: 0.8,
                      child: IconButton(
                        padding: EdgeInsets.zero,
                        icon: const Icon(Icons.settings),
                        onPressed: () {
                          if (Navigator.canPop(context)) {
                            Navigator.pop(context);
                            _menuDismissCallback();
                          }
                          showKBLayoutTypeChooser(
                              localPlatform, widget.ffi.dialogManager);
                        },
                      ),
                    ),
                  ))
                ],
              )),
          proc: () {},
          padding: EdgeInsets.zero,
          dismissOnClicked: false,
          dismissCallback: _menuDismissCallback,
        ),
      );
    }
    return keyboardMenu;
  }

  MenuEntrySwitch<String> _createSwitchMenuEntry(
      String text, String option, EdgeInsets? padding, bool dismissOnClicked) {
    return RemoteMenuEntry.createSwitchMenuEntry(
        widget.id, text, option, padding, dismissOnClicked,
        dismissCallback: _menuDismissCallback);
  }
}

void showSetOSPassword(
    String id, bool login, OverlayDialogManager dialogManager) async {
  final controller = TextEditingController();
  var password = await bind.sessionGetOption(id: id, arg: 'os-password') ?? '';
  var autoLogin = await bind.sessionGetOption(id: id, arg: 'auto-login') != '';
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
      title: Text(translate('OS Password')),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
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
      ]),
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showAuditDialog(String id, dialogManager) async {
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
            // inputFormatters: [
            //   LengthLimitingTextInputFormatter(16),
            //   // FilteringTextInputFormatter(RegExp(r'[a-zA-z][a-zA-z0-9\_]*'), allow: true)
            // ],
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

void showConfirmSwitchSidesDialog(
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
        color: Colors.grey[800],
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
          color: Colors.white,
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
