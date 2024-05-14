import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/toolbar.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/mobile/widgets/gesture_help.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import '../../common.dart';
import '../../common/widgets/overlay.dart';
import '../../common/widgets/dialog.dart';
import '../../common/widgets/remote_input.dart';
import '../../models/input_model.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../utils/image.dart';
import '../widgets/dialog.dart';

final initText = '1' * 1024;

class RemotePage extends StatefulWidget {
  RemotePage({Key? key, required this.id, this.password, this.isSharedPassword})
      : super(key: key);

  final String id;
  final String? password;
  final bool? isSharedPassword;

  @override
  State<RemotePage> createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  Timer? _timer;
  bool _showBar = !isWebDesktop;
  bool _showGestureHelp = false;
  String _value = '';
  Orientation? _currentOrientation;

  final _blockableOverlayState = BlockableOverlayState();

  final keyboardVisibilityController = KeyboardVisibilityController();
  late final StreamSubscription<bool> keyboardSubscription;
  final FocusNode _mobileFocusNode = FocusNode();
  final FocusNode _physicalFocusNode = FocusNode();
  var _showEdit = false; // use soft keyboard

  InputModel get inputModel => gFFI.inputModel;
  SessionID get sessionId => gFFI.sessionId;

  @override
  void initState() {
    super.initState();
    gFFI.ffiModel.updateEventListener(sessionId, widget.id);
    gFFI.start(
      widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    if (!isWeb) {
      WakelockPlus.enable();
    }
    _physicalFocusNode.requestFocus();
    gFFI.inputModel.listenToMouse(true);
    gFFI.qualityMonitorModel.checkShowQualityMonitor(sessionId);
    keyboardSubscription =
        keyboardVisibilityController.onChange.listen(onSoftKeyboardChanged);
    initSharedStates(widget.id);
    gFFI.chatModel
        .changeCurrentKey(MessageKey(widget.id, ChatModel.clientModeID));
    gFFI.chatModel.voiceCallStatus.value = VoiceCallStatus.notStarted;
    _blockableOverlayState.applyFfi(gFFI);
  }

  @override
  Future<void> dispose() async {
    // https://github.com/flutter/flutter/issues/64935
    super.dispose();
    gFFI.dialogManager.hideMobileActionsOverlay();
    gFFI.inputModel.listenToMouse(false);
    await gFFI.invokeMethod("enable_soft_keyboard", true);
    _mobileFocusNode.dispose();
    _physicalFocusNode.dispose();
    await gFFI.close();
    _timer?.cancel();
    gFFI.dialogManager.dismissAll();
    await SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    if (!isWeb) {
      await WakelockPlus.disable();
    }
    await keyboardSubscription.cancel();
    removeSharedStates(widget.id);
    if (isAndroid) {
      // Only one client is considered here for now.
      // TODO: take into account the case where there are multiple clients
      gFFI.invokeMethod("on_voice_call_closed");
    }
  }

  // to-do: It should be better to use transparent color instead of the bgColor.
  // But for now, the transparent color will cause the canvas to be white.
  // I'm sure that the white color is caused by the Overlay widget in BlockableOverlay.
  // But I don't know why and how to fix it.
  Widget emptyOverlay(Color bgColor) => BlockableOverlay(
        /// the Overlay key will be set with _blockableOverlayState in BlockableOverlay
        /// see override build() in [BlockableOverlay]
        state: _blockableOverlayState,
        underlying: Container(
          color: bgColor,
        ),
      );

  void onSoftKeyboardChanged(bool visible) {
    if (!visible) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      // [pi.version.isNotEmpty] -> check ready or not, avoid login without soft-keyboard
      if (gFFI.chatModel.chatWindowOverlayEntry == null &&
          gFFI.ffiModel.pi.version.isNotEmpty) {
        gFFI.invokeMethod("enable_soft_keyboard", false);
      }
    } else {
      _timer?.cancel();
      _timer = Timer(kMobileDelaySoftKeyboardFocus, () {
        SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
            overlays: SystemUiOverlay.values);
        _mobileFocusNode.requestFocus();
      });
    }
    // update for Scaffold
    setState(() {});
  }

  // handle mobile virtual keyboard
  void handleSoftKeyboardInput(String newValue) {
    var oldValue = _value;
    _value = newValue;
    if (isIOS) {
      var i = newValue.length - 1;
      for (; i >= 0 && newValue[i] != '\1'; --i) {}
      var j = oldValue.length - 1;
      for (; j >= 0 && oldValue[j] != '\1'; --j) {}
      if (i < j) j = i;
      newValue = newValue.substring(j + 1);
      oldValue = oldValue.substring(j + 1);
      var common = 0;
      for (;
          common < oldValue.length &&
              common < newValue.length &&
              newValue[common] == oldValue[common];
          ++common) {}
      for (i = 0; i < oldValue.length - common; ++i) {
        inputModel.inputKey('VK_BACK');
      }
      if (newValue.length > common) {
        var s = newValue.substring(common);
        if (s.length > 1) {
          bind.sessionInputString(sessionId: sessionId, value: s);
        } else {
          inputChar(s);
        }
      }
      return;
    }
    if (oldValue.isNotEmpty &&
        newValue.isNotEmpty &&
        oldValue[0] == '\1' &&
        newValue[0] != '\1') {
      // clipboard
      oldValue = '';
    }
    if (newValue.length == oldValue.length) {
      // ?
    } else if (newValue.length < oldValue.length) {
      final char = 'VK_BACK';
      inputModel.inputKey(char);
    } else {
      final content = newValue.substring(oldValue.length);
      if (content.length > 1) {
        if (oldValue != '' &&
            content.length == 2 &&
            (content == '""' ||
                content == '()' ||
                content == '[]' ||
                content == '<>' ||
                content == "{}" ||
                content == '”“' ||
                content == '《》' ||
                content == '（）' ||
                content == '【】')) {
          // can not only input content[0], because when input ], [ are also auo insert, which cause ] never be input
          bind.sessionInputString(sessionId: sessionId, value: content);
          openKeyboard();
          return;
        }
        bind.sessionInputString(sessionId: sessionId, value: content);
      } else {
        inputChar(content);
      }
    }
  }

  void inputChar(String char) {
    if (char == '\n') {
      char = 'VK_RETURN';
    } else if (char == ' ') {
      char = 'VK_SPACE';
    }
    inputModel.inputKey(char);
  }

  void openKeyboard() {
    gFFI.invokeMethod("enable_soft_keyboard", true);
    // destroy first, so that our _value trick can work
    _value = initText;
    setState(() => _showEdit = false);
    _timer?.cancel();
    _timer = Timer(kMobileDelaySoftKeyboard, () {
      // show now, and sleep a while to requestFocus to
      // make sure edit ready, so that keyboard won't show/hide/show/hide happen
      setState(() => _showEdit = true);
      _timer?.cancel();
      _timer = Timer(kMobileDelaySoftKeyboardFocus, () {
        SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
            overlays: SystemUiOverlay.values);
        _mobileFocusNode.requestFocus();
      });
    });
  }

  bool get keyboard => gFFI.ffiModel.permissions['keyboard'] != false;

  Widget _bottomWidget() => _showGestureHelp
      ? getGestureHelp()
      : (_showBar && gFFI.ffiModel.pi.displays.isNotEmpty
          ? getBottomAppBar(keyboard)
          : Offstage());

  @override
  Widget build(BuildContext context) {
    final keyboardIsVisible =
        keyboardVisibilityController.isVisible && _showEdit;
    final showActionButton = !_showBar || keyboardIsVisible || _showGestureHelp;

    return WillPopScope(
      onWillPop: () async {
        clientClose(sessionId, gFFI.dialogManager);
        return false;
      },
      child: Scaffold(
          // workaround for https://github.com/rustdesk/rustdesk/issues/3131
          floatingActionButtonLocation: keyboardIsVisible
              ? FABLocation(FloatingActionButtonLocation.endFloat, 0, -35)
              : null,
          floatingActionButton: !showActionButton
              ? null
              : FloatingActionButton(
                  mini: !keyboardIsVisible,
                  child: Icon(
                    (keyboardIsVisible || _showGestureHelp)
                        ? Icons.expand_more
                        : Icons.expand_less,
                    color: Colors.white,
                  ),
                  backgroundColor: MyTheme.accent,
                  onPressed: () {
                    setState(() {
                      if (keyboardIsVisible) {
                        _showEdit = false;
                        gFFI.invokeMethod("enable_soft_keyboard", false);
                        _mobileFocusNode.unfocus();
                        _physicalFocusNode.requestFocus();
                      } else if (_showGestureHelp) {
                        _showGestureHelp = false;
                      } else {
                        _showBar = !_showBar;
                      }
                    });
                  }),
          bottomNavigationBar: Obx(() => Stack(
                alignment: Alignment.bottomCenter,
                children: [
                  gFFI.ffiModel.pi.isSet.isTrue &&
                          gFFI.ffiModel.waitForFirstImage.isTrue
                      ? emptyOverlay(MyTheme.canvasColor)
                      : () {
                          gFFI.ffiModel.tryShowAndroidActionsOverlay();
                          return Offstage();
                        }(),
                  _bottomWidget(),
                  gFFI.ffiModel.pi.isSet.isFalse
                      ? emptyOverlay(MyTheme.canvasColor)
                      : Offstage(),
                ],
              )),
          body: Obx(
            () => getRawPointerAndKeyBody(Overlay(
              initialEntries: [
                OverlayEntry(builder: (context) {
                  return Container(
                    color: Colors.black,
                    child: isWebDesktop
                        ? getBodyForDesktopWithListener(keyboard)
                        : SafeArea(
                            child:
                                OrientationBuilder(builder: (ctx, orientation) {
                              if (_currentOrientation != orientation) {
                                Timer(const Duration(milliseconds: 200), () {
                                  gFFI.dialogManager
                                      .resetMobileActionsOverlay(ffi: gFFI);
                                  _currentOrientation = orientation;
                                  gFFI.canvasModel.updateViewStyle();
                                });
                              }
                              return Container(
                                color: MyTheme.canvasColor,
                                child: inputModel.isPhysicalMouse.value
                                    ? getBodyForMobile()
                                    : RawTouchGestureDetectorRegion(
                                        child: getBodyForMobile(),
                                        ffi: gFFI,
                                      ),
                              );
                            }),
                          ),
                  );
                })
              ],
            )),
          )),
    );
  }

  Widget getRawPointerAndKeyBody(Widget child) {
    final keyboard = gFFI.ffiModel.permissions['keyboard'] != false;
    return RawPointerMouseRegion(
      cursor: keyboard ? SystemMouseCursors.none : MouseCursor.defer,
      inputModel: inputModel,
      // Disable RawKeyFocusScope before the connecting is established.
      // The "Delete" key on the soft keyboard may be grabbed when inputting the password dialog.
      child: gFFI.ffiModel.pi.isSet.isTrue
          ? RawKeyFocusScope(
              focusNode: _physicalFocusNode,
              inputModel: inputModel,
              child: child)
          : child,
    );
  }

  Widget getBottomAppBar(bool keyboard) {
    return BottomAppBar(
      elevation: 10,
      color: MyTheme.accent,
      child: Row(
        mainAxisSize: MainAxisSize.max,
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: <Widget>[
          Row(
              children: <Widget>[
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.clear),
                      onPressed: () {
                        clientClose(sessionId, gFFI.dialogManager);
                      },
                    ),
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.tv),
                      onPressed: () {
                        setState(() => _showEdit = false);
                        showOptions(context, widget.id, gFFI.dialogManager);
                      },
                    )
                  ] +
                  (isWebDesktop
                      ? []
                      : gFFI.ffiModel.isPeerAndroid
                          ? [
                              IconButton(
                                  color: Colors.white,
                                  icon: Icon(Icons.keyboard),
                                  onPressed: openKeyboard),
                              IconButton(
                                color: Colors.white,
                                icon: const Icon(Icons.build),
                                onPressed: () => gFFI.dialogManager
                                    .toggleMobileActionsOverlay(ffi: gFFI),
                              )
                            ]
                          : [
                              IconButton(
                                  color: Colors.white,
                                  icon: Icon(Icons.keyboard),
                                  onPressed: openKeyboard),
                              IconButton(
                                color: Colors.white,
                                icon: Icon(gFFI.ffiModel.touchMode
                                    ? Icons.touch_app
                                    : Icons.mouse),
                                onPressed: () => setState(
                                    () => _showGestureHelp = !_showGestureHelp),
                              ),
                            ]) +
                  (isWeb
                      ? []
                      : <Widget>[
                          IconButton(
                            color: Colors.white,
                            icon: isAndroid
                                ? SvgPicture.asset('assets/chat.svg',
                                    colorFilter: ColorFilter.mode(
                                        Colors.white, BlendMode.srcIn))
                                : Icon(Icons.message),
                            onPressed: () => isAndroid
                                ? showChatOptions(widget.id)
                                : onPressedTextChat(widget.id),
                          )
                        ]) +
                  [
                    IconButton(
                      color: Colors.white,
                      icon: Icon(Icons.more_vert),
                      onPressed: () {
                        setState(() => _showEdit = false);
                        showActions(widget.id);
                      },
                    ),
                  ]),
          Obx(() => IconButton(
                color: Colors.white,
                icon: Icon(Icons.expand_more),
                onPressed: gFFI.ffiModel.waitForFirstImage.isTrue
                    ? null
                    : () {
                        setState(() => _showBar = !_showBar);
                      },
              )),
        ],
      ),
    );
  }

  bool get showCursorPaint =>
      !gFFI.ffiModel.isPeerAndroid && !gFFI.canvasModel.cursorEmbedded;

  Widget getBodyForMobile() {
    final keyboardIsVisible = keyboardVisibilityController.isVisible;
    return Container(
        color: MyTheme.canvasColor,
        child: Stack(children: () {
          final paints = [
            ImagePaint(),
            Positioned(
              top: 10,
              right: 10,
              child: QualityMonitor(gFFI.qualityMonitorModel),
            ),
            KeyHelpTools(requestShow: (keyboardIsVisible || _showGestureHelp)),
            SizedBox(
              width: 0,
              height: 0,
              child: !_showEdit
                  ? Container()
                  : TextFormField(
                      textInputAction: TextInputAction.newline,
                      autocorrect: false,
                      enableSuggestions: false,
                      autofocus: true,
                      focusNode: _mobileFocusNode,
                      maxLines: null,
                      initialValue: _value,
                      // trick way to make backspace work always
                      keyboardType: TextInputType.multiline,
                      onChanged: handleSoftKeyboardInput,
                    ),
            ),
          ];
          if (showCursorPaint) {
            paints.add(CursorPaint());
          }
          return paints;
        }()));
  }

  Widget getBodyForDesktopWithListener(bool keyboard) {
    var paints = <Widget>[ImagePaint()];
    if (showCursorPaint) {
      final cursor = bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: 'show-remote-cursor');
      if (keyboard || cursor) {
        paints.add(CursorPaint());
      }
    }
    return Container(
        color: MyTheme.canvasColor, child: Stack(children: paints));
  }

  void showActions(String id) async {
    final size = MediaQuery.of(context).size;
    final x = 120.0;
    final y = size.height;
    final menus = toolbarControls(context, id, gFFI);
    getChild(TTextMenu menu) {
      if (menu.trailingIcon != null) {
        return Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              menu.child,
              menu.trailingIcon!,
            ]);
      } else {
        return menu.child;
      }
    }

    final more = menus
        .asMap()
        .entries
        .map((e) => PopupMenuItem<int>(child: getChild(e.value), value: e.key))
        .toList();
    () async {
      var index = await showMenu(
        context: context,
        position: RelativeRect.fromLTRB(x, y, x, y),
        items: more,
        elevation: 8,
      );
      if (index != null && index < menus.length) {
        menus[index].onPressed.call();
      }
    }();
  }

  onPressedTextChat(String id) {
    gFFI.chatModel.changeCurrentKey(MessageKey(id, ChatModel.clientModeID));
    gFFI.chatModel.toggleChatOverlay();
  }

  showChatOptions(String id) async {
    onPressVoiceCall() => bind.sessionRequestVoiceCall(sessionId: sessionId);
    onPressEndVoiceCall() => bind.sessionCloseVoiceCall(sessionId: sessionId);

    makeTextMenu(String label, Widget icon, VoidCallback onPressed,
            {TextStyle? labelStyle}) =>
        TTextMenu(
          child: Text(translate(label), style: labelStyle),
          trailingIcon: Transform.scale(
            scale: (isDesktop || isWebDesktop) ? 0.8 : 1,
            child: IconButton(
              onPressed: onPressed,
              icon: icon,
            ),
          ),
          onPressed: onPressed,
        );

    final isInVoice = [
      VoiceCallStatus.waitingForResponse,
      VoiceCallStatus.connected
    ].contains(gFFI.chatModel.voiceCallStatus.value);
    final menus = [
      makeTextMenu('Text chat', Icon(Icons.message, color: MyTheme.accent),
          () => onPressedTextChat(widget.id)),
      isInVoice
          ? makeTextMenu(
              'End voice call',
              SvgPicture.asset(
                'assets/call_wait.svg',
                colorFilter:
                    ColorFilter.mode(Colors.redAccent, BlendMode.srcIn),
              ),
              onPressEndVoiceCall,
              labelStyle: TextStyle(color: Colors.redAccent))
          : makeTextMenu(
              'Voice call',
              SvgPicture.asset(
                'assets/call_wait.svg',
                colorFilter: ColorFilter.mode(MyTheme.accent, BlendMode.srcIn),
              ),
              onPressVoiceCall),
    ];
    getChild(TTextMenu menu) {
      if (menu.trailingIcon != null) {
        return Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              menu.child,
              menu.trailingIcon!,
            ]);
      } else {
        return menu.child;
      }
    }

    final menuItems = menus
        .asMap()
        .entries
        .map((e) => PopupMenuItem<int>(child: getChild(e.value), value: e.key))
        .toList();
    Future.delayed(Duration.zero, () async {
      final size = MediaQuery.of(context).size;
      final x = 120.0;
      final y = size.height;
      var index = await showMenu(
        context: context,
        position: RelativeRect.fromLTRB(x, y, x, y),
        items: menuItems,
        elevation: 8,
      );
      if (index != null && index < menus.length) {
        menus[index].onPressed.call();
      }
    });
  }

  /// aka changeTouchMode
  BottomAppBar getGestureHelp() {
    return BottomAppBar(
        child: SingleChildScrollView(
            controller: ScrollController(),
            padding: EdgeInsets.symmetric(vertical: 10),
            child: GestureHelp(
                touchMode: gFFI.ffiModel.touchMode,
                onTouchModeChange: (t) {
                  gFFI.ffiModel.toggleTouchMode();
                  final v = gFFI.ffiModel.touchMode ? 'Y' : '';
                  bind.sessionPeerOption(
                      sessionId: sessionId, name: "touch-mode", value: v);
                })));
  }

  // * Currently mobile does not enable map mode
  // void changePhysicalKeyboardInputMode() async {
  //   var current = await bind.sessionGetKeyboardMode(id: widget.id) ?? "legacy";
  //   gFFI.dialogManager.show((setState, close) {
  //     void setMode(String? v) async {
  //       await bind.sessionSetKeyboardMode(id: widget.id, value: v ?? "");
  //       setState(() => current = v ?? '');
  //       Future.delayed(Duration(milliseconds: 300), close);
  //     }
  //
  //     return CustomAlertDialog(
  //         title: Text(translate('Physical Keyboard Input Mode')),
  //         content: Column(mainAxisSize: MainAxisSize.min, children: [
  //           getRadio('Legacy mode', 'legacy', current, setMode),
  //           getRadio('Map mode', 'map', current, setMode),
  //         ]));
  //   }, clickMaskDismiss: true);
  // }
}

class KeyHelpTools extends StatefulWidget {
  /// need to show by external request, etc [keyboardIsVisible] or [changeTouchMode]
  final bool requestShow;

  KeyHelpTools({required this.requestShow});

  @override
  State<KeyHelpTools> createState() => _KeyHelpToolsState();
}

class _KeyHelpToolsState extends State<KeyHelpTools> {
  var _more = true;
  var _fn = false;
  var _pin = false;
  final _keyboardVisibilityController = KeyboardVisibilityController();

  InputModel get inputModel => gFFI.inputModel;

  Widget wrap(String text, void Function() onPressed,
      {bool? active, IconData? icon}) {
    return TextButton(
        style: TextButton.styleFrom(
          minimumSize: Size(0, 0),
          padding: EdgeInsets.symmetric(vertical: 10, horizontal: 9.75),
          //adds padding inside the button
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          //limits the touch area to the button area
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(5.0),
          ),
          backgroundColor: active == true ? MyTheme.accent80 : null,
        ),
        child: icon != null
            ? Icon(icon, size: 14, color: Colors.white)
            : Text(translate(text),
                style: TextStyle(color: Colors.white, fontSize: 11)),
        onPressed: onPressed);
  }

  @override
  Widget build(BuildContext context) {
    final hasModifierOn = inputModel.ctrl ||
        inputModel.alt ||
        inputModel.shift ||
        inputModel.command;

    if (!_pin && !hasModifierOn && !widget.requestShow) {
      return Offstage();
    }
    final size = MediaQuery.of(context).size;

    final pi = gFFI.ffiModel.pi;
    final isMac = pi.platform == kPeerPlatformMacOS;
    final modifiers = <Widget>[
      wrap('Ctrl ', () {
        setState(() => inputModel.ctrl = !inputModel.ctrl);
      }, active: inputModel.ctrl),
      wrap(' Alt ', () {
        setState(() => inputModel.alt = !inputModel.alt);
      }, active: inputModel.alt),
      wrap('Shift', () {
        setState(() => inputModel.shift = !inputModel.shift);
      }, active: inputModel.shift),
      wrap(isMac ? ' Cmd ' : ' Win ', () {
        setState(() => inputModel.command = !inputModel.command);
      }, active: inputModel.command),
    ];
    final keys = <Widget>[
      wrap(
          ' Fn ',
          () => setState(
                () {
                  _fn = !_fn;
                  if (_fn) {
                    _more = false;
                  }
                },
              ),
          active: _fn),
      wrap(
          '',
          () => setState(
                () => _pin = !_pin,
              ),
          active: _pin,
          icon: Icons.push_pin),
      wrap(
          ' ... ',
          () => setState(
                () {
                  _more = !_more;
                  if (_more) {
                    _fn = false;
                  }
                },
              ),
          active: _more),
    ];
    final fn = <Widget>[
      SizedBox(width: 9999),
    ];
    for (var i = 1; i <= 12; ++i) {
      final name = 'F$i';
      fn.add(wrap(name, () {
        inputModel.inputKey('VK_$name');
      }));
    }
    final more = <Widget>[
      SizedBox(width: 9999),
      wrap('Esc', () {
        inputModel.inputKey('VK_ESCAPE');
      }),
      wrap('Tab', () {
        inputModel.inputKey('VK_TAB');
      }),
      wrap('Home', () {
        inputModel.inputKey('VK_HOME');
      }),
      wrap('End', () {
        inputModel.inputKey('VK_END');
      }),
      wrap('Ins', () {
        inputModel.inputKey('VK_INSERT');
      }),
      wrap('Del', () {
        inputModel.inputKey('VK_DELETE');
      }),
      wrap('PgUp', () {
        inputModel.inputKey('VK_PRIOR');
      }),
      wrap('PgDn', () {
        inputModel.inputKey('VK_NEXT');
      }),
      SizedBox(width: 9999),
      wrap('', () {
        inputModel.inputKey('VK_LEFT');
      }, icon: Icons.keyboard_arrow_left),
      wrap('', () {
        inputModel.inputKey('VK_UP');
      }, icon: Icons.keyboard_arrow_up),
      wrap('', () {
        inputModel.inputKey('VK_DOWN');
      }, icon: Icons.keyboard_arrow_down),
      wrap('', () {
        inputModel.inputKey('VK_RIGHT');
      }, icon: Icons.keyboard_arrow_right),
      wrap(isMac ? 'Cmd+C' : 'Ctrl+C', () {
        sendPrompt(isMac, 'VK_C');
      }),
      wrap(isMac ? 'Cmd+V' : 'Ctrl+V', () {
        sendPrompt(isMac, 'VK_V');
      }),
      wrap(isMac ? 'Cmd+S' : 'Ctrl+S', () {
        sendPrompt(isMac, 'VK_S');
      }),
    ];
    final space = size.width > 320 ? 4.0 : 2.0;
    return Container(
        color: Color(0xAA000000),
        padding: EdgeInsets.only(
            top: _keyboardVisibilityController.isVisible ? 24 : 4, bottom: 8),
        child: Wrap(
          spacing: space,
          runSpacing: space,
          children: <Widget>[SizedBox(width: 9999)] +
              modifiers +
              keys +
              (_fn ? fn : []) +
              (_more ? more : []),
        ));
  }
}

class ImagePaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    final c = Provider.of<CanvasModel>(context);
    final adjust = gFFI.cursorModel.adjustForKeyboard();
    var s = c.scale;
    return CustomPaint(
      painter: ImagePainter(
          image: m.image, x: c.x / s, y: (c.y - adjust) / s, scale: s),
    );
  }
}

class CursorPaint extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    final adjust = gFFI.cursorModel.adjustForKeyboard();
    var s = c.scale;
    double hotx = m.hotx;
    double hoty = m.hoty;
    if (m.image == null) {
      if (preDefaultCursor.image != null) {
        hotx = preDefaultCursor.image!.width / 2;
        hoty = preDefaultCursor.image!.height / 2;
      }
    }
    return CustomPaint(
      painter: ImagePainter(
          image: m.image ?? preDefaultCursor.image,
          x: m.x * s - hotx + c.x,
          y: m.y * s - hoty + c.y - adjust,
          scale: 1),
    );
  }
}

void showOptions(
    BuildContext context, String id, OverlayDialogManager dialogManager) async {
  var displays = <Widget>[];
  final pi = gFFI.ffiModel.pi;
  final image = gFFI.ffiModel.getConnectionImage();
  if (image != null) {
    displays.add(Padding(padding: const EdgeInsets.only(top: 8), child: image));
  }
  if (pi.displays.length > 1 && pi.currentDisplay != kAllDisplayValue) {
    final cur = pi.currentDisplay;
    final children = <Widget>[];
    for (var i = 0; i < pi.displays.length; ++i) {
      children.add(InkWell(
          onTap: () {
            if (i == cur) return;
            openMonitorInTheSameTab(i, gFFI, pi);
            gFFI.dialogManager.dismissAll();
          },
          child: Ink(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                  border: Border.all(color: Theme.of(context).hintColor),
                  borderRadius: BorderRadius.circular(2),
                  color: i == cur
                      ? Theme.of(context).toggleableActiveColor.withOpacity(0.6)
                      : null),
              child: Center(
                  child: Text((i + 1).toString(),
                      style: TextStyle(
                          color: i == cur ? Colors.white : Colors.black87,
                          fontWeight: FontWeight.bold))))));
    }
    displays.add(Padding(
        padding: const EdgeInsets.only(top: 8),
        child: Wrap(
          alignment: WrapAlignment.center,
          spacing: 8,
          children: children,
        )));
  }
  if (displays.isNotEmpty) {
    displays.add(const Divider(color: MyTheme.border));
  }

  List<TRadioMenu<String>> viewStyleRadios =
      await toolbarViewStyle(context, id, gFFI);
  List<TRadioMenu<String>> imageQualityRadios =
      await toolbarImageQuality(context, id, gFFI);
  List<TRadioMenu<String>> codecRadios = await toolbarCodec(context, id, gFFI);
  List<TToggleMenu> cursorToggles = await toolbarCursor(context, id, gFFI);
  List<TToggleMenu> displayToggles =
      await toolbarDisplayToggle(context, id, gFFI);

  List<TToggleMenu> privacyModeList = [];
  // privacy mode
  final privacyModeState = PrivacyModeState.find(id);
  if (gFFI.ffiModel.keyboard && gFFI.ffiModel.pi.features.privacyMode) {
    privacyModeList = toolbarPrivacyMode(privacyModeState, context, id, gFFI);
    if (privacyModeList.length == 1) {
      displayToggles.add(privacyModeList[0]);
    }
  }

  dialogManager.show((setState, close, context) {
    var viewStyle =
        (viewStyleRadios.isNotEmpty ? viewStyleRadios[0].groupValue : '').obs;
    var imageQuality =
        (imageQualityRadios.isNotEmpty ? imageQualityRadios[0].groupValue : '')
            .obs;
    var codec = (codecRadios.isNotEmpty ? codecRadios[0].groupValue : '').obs;
    final radios = [
      for (var e in viewStyleRadios)
        Obx(() => getRadio<String>(e.child, e.value, viewStyle.value, (v) {
              e.onChanged?.call(v);
              if (v != null) viewStyle.value = v;
            })),
      const Divider(color: MyTheme.border),
      for (var e in imageQualityRadios)
        Obx(() => getRadio<String>(e.child, e.value, imageQuality.value, (v) {
              e.onChanged?.call(v);
              if (v != null) imageQuality.value = v;
            })),
      const Divider(color: MyTheme.border),
      for (var e in codecRadios)
        Obx(() => getRadio<String>(e.child, e.value, codec.value, (v) {
              e.onChanged?.call(v);
              if (v != null) codec.value = v;
            })),
      if (codecRadios.isNotEmpty) const Divider(color: MyTheme.border),
    ];
    final rxCursorToggleValues = cursorToggles.map((e) => e.value.obs).toList();
    final cursorTogglesList = cursorToggles
        .asMap()
        .entries
        .map((e) => Obx(() => CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            visualDensity: VisualDensity.compact,
            value: rxCursorToggleValues[e.key].value,
            onChanged: (v) {
              e.value.onChanged?.call(v);
              if (v != null) rxCursorToggleValues[e.key].value = v;
            },
            title: e.value.child)))
        .toList();

    final rxToggleValues = displayToggles.map((e) => e.value.obs).toList();
    final displayTogglesList = displayToggles
        .asMap()
        .entries
        .map((e) => Obx(() => CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            visualDensity: VisualDensity.compact,
            value: rxToggleValues[e.key].value,
            onChanged: (v) {
              e.value.onChanged?.call(v);
              if (v != null) rxToggleValues[e.key].value = v;
            },
            title: e.value.child)))
        .toList();
    final toggles = [
      ...cursorTogglesList,
      if (cursorToggles.isNotEmpty) const Divider(color: MyTheme.border),
      ...displayTogglesList,
    ];

    Widget privacyModeWidget = Offstage();
    if (privacyModeList.length > 1) {
      privacyModeWidget = ListTile(
        contentPadding: EdgeInsets.zero,
        visualDensity: VisualDensity.compact,
        title: Text(translate('Privacy mode')),
        onTap: () => setPrivacyModeDialog(
            dialogManager, privacyModeList, privacyModeState),
      );
    }

    return CustomAlertDialog(
      content: Column(
          mainAxisSize: MainAxisSize.min,
          children: displays + radios + toggles + [privacyModeWidget]),
    );
  }, clickMaskDismiss: true, backDismiss: true);
}

void sendPrompt(bool isMac, String key) {
  final old = isMac ? gFFI.inputModel.command : gFFI.inputModel.ctrl;
  if (isMac) {
    gFFI.inputModel.command = true;
  } else {
    gFFI.inputModel.ctrl = true;
  }
  gFFI.inputModel.inputKey(key);
  if (isMac) {
    gFFI.inputModel.command = old;
  } else {
    gFFI.inputModel.ctrl = old;
  }
}

class FABLocation extends FloatingActionButtonLocation {
  FloatingActionButtonLocation location;
  double offsetX;
  double offsetY;
  FABLocation(this.location, this.offsetX, this.offsetY);

  @override
  Offset getOffset(ScaffoldPrelayoutGeometry scaffoldGeometry) {
    final offset = location.getOffset(scaffoldGeometry);
    return Offset(offset.dx + offsetX, offset.dy + offsetY);
  }
}
