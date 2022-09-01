import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:get/get.dart';
import 'package:rxdart/rxdart.dart' as rxdart;

import '../../common.dart';
import '../../mobile/widgets/dialog.dart';
import '../../mobile/widgets/overlay.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../../common/shared_state.dart';
import './popup_menu.dart';
import './material_mod_popup_menu.dart' as mod_menu;

class _MenubarTheme {
  static const Color commonColor = MyTheme.accent;
  // kMinInteractiveDimension
  static const double height = 24.0;
  static const double dividerHeight = 12.0;
}

class RemoteMenubar extends StatefulWidget {
  final String id;
  final FFI ffi;

  const RemoteMenubar({
    Key? key,
    required this.id,
    required this.ffi,
  }) : super(key: key);

  @override
  State<RemoteMenubar> createState() => _RemoteMenubarState();
}

class _RemoteMenubarState extends State<RemoteMenubar> {
  final RxBool _show = false.obs;
  final Rx<Color> _hideColor = Colors.white12.obs;

  bool get isFullscreen => Get.find<RxBool>(tag: 'fullscreen').isTrue;
  void setFullscreen(bool v) {
    Get.find<RxBool>(tag: 'fullscreen').value = v;
  }

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.topCenter,
      child: Obx(
          () => _show.value ? _buildMenubar(context) : _buildShowHide(context)),
    );
  }

  Widget _buildShowHide(BuildContext context) {
    return Obx(() => Tooltip(
          message: translate(_show.value ? "Hide Menubar" : "Show Menubar"),
          child: SizedBox(
              width: 100,
              height: 5,
              child: TextButton(
                  onHover: (bool v) {
                    _hideColor.value = v ? Colors.white60 : Colors.white24;
                  },
                  onPressed: () {
                    _show.value = !_show.value;
                  },
                  child: Obx(() => Container(
                        color: _hideColor.value,
                      )))),
        ));
  }

  Widget _buildMenubar(BuildContext context) {
    final List<Widget> menubarItems = [];
    if (!isWebDesktop) {
      menubarItems.add(_buildFullscreen(context));
      if (widget.ffi.ffiModel.isPeerAndroid) {
        menubarItems.add(IconButton(
          tooltip: translate('Mobile Actions'),
          color: _MenubarTheme.commonColor,
          icon: const Icon(Icons.build),
          onPressed: () {
            if (mobileActionsOverlayEntry == null) {
              showMobileActionsOverlay();
            } else {
              hideMobileActionsOverlay();
            }
          },
        ));
      }
    }
    menubarItems.add(_buildMonitor(context));
    menubarItems.add(_buildControl(context));
    menubarItems.add(_buildDisplay(context));
    if (!isWeb) {
      menubarItems.add(_buildChat(context));
    }
    menubarItems.add(_buildClose(context));
    return PopupMenuTheme(
        data: const PopupMenuThemeData(
            textStyle: TextStyle(color: _MenubarTheme.commonColor)),
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          Container(
              color: Colors.white,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: menubarItems,
              )),
          _buildShowHide(context),
        ]));
  }

  Widget _buildFullscreen(BuildContext context) {
    return IconButton(
      tooltip: translate(isFullscreen ? 'Exit Fullscreen' : 'Fullscreen'),
      onPressed: () {
        setFullscreen(!isFullscreen);
      },
      icon: Obx(() => isFullscreen
          ? const Icon(
              Icons.fullscreen_exit,
              color: _MenubarTheme.commonColor,
            )
          : const Icon(
              Icons.fullscreen,
              color: _MenubarTheme.commonColor,
            )),
    );
  }

  Widget _buildChat(BuildContext context) {
    return IconButton(
      tooltip: translate('Chat'),
      onPressed: () {
        widget.ffi.chatModel.changeCurrentID(ChatModel.clientModeID);
        widget.ffi.chatModel.toggleChatOverlay();
      },
      icon: const Icon(
        Icons.message,
        color: _MenubarTheme.commonColor,
      ),
    );
  }

  Widget _buildMonitor(BuildContext context) {
    final pi = widget.ffi.ffiModel.pi;
    return mod_menu.PopupMenuButton(
      tooltip: translate('Select Monitor'),
      padding: EdgeInsets.zero,
      position: mod_menu.PopupMenuPosition.under,
      icon: Stack(
        alignment: Alignment.center,
        children: [
          const Icon(
            Icons.personal_video,
            color: _MenubarTheme.commonColor,
          ),
          Padding(
            padding: const EdgeInsets.only(bottom: 3.9),
            child: Obx(() {
              RxInt display = CurrentDisplayState.find(widget.id);
              return Text(
                "${display.value + 1}/${pi.displays.length}",
                style: const TextStyle(
                    color: _MenubarTheme.commonColor, fontSize: 8),
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
                const Icon(
                  Icons.personal_video,
                  color: _MenubarTheme.commonColor,
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
                          style:
                              const TextStyle(color: _MenubarTheme.commonColor),
                        ),
                      )),
                  onPressed: () {
                    RxInt display = CurrentDisplayState.find(widget.id);
                    if (display.value != i) {
                      bind.sessionSwitchDisplay(id: widget.id, value: i);
                      pi.currentDisplay = i;
                      display.value = i;
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
            child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: rowChildren),
          )
        ];
      },
    );
  }

  Widget _buildControl(BuildContext context) {
    return mod_menu.PopupMenuButton(
      padding: EdgeInsets.zero,
      icon: const Icon(
        Icons.bolt,
        color: _MenubarTheme.commonColor,
      ),
      tooltip: translate('Control Actions'),
      position: mod_menu.PopupMenuPosition.under,
      itemBuilder: (BuildContext context) => _getControlMenu()
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenubarTheme.commonColor,
                height: _MenubarTheme.height,
                dividerHeight: _MenubarTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  Widget _buildDisplay(BuildContext context) {
    return mod_menu.PopupMenuButton(
      padding: EdgeInsets.zero,
      icon: const Icon(
        Icons.tv,
        color: _MenubarTheme.commonColor,
      ),
      tooltip: translate('Display Settings'),
      position: mod_menu.PopupMenuPosition.under,
      onSelected: (String item) {},
      itemBuilder: (BuildContext context) => _getDisplayMenu()
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenubarTheme.commonColor,
                height: _MenubarTheme.height,
                dividerHeight: _MenubarTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  Widget _buildClose(BuildContext context) {
    return IconButton(
      tooltip: translate('Close'),
      onPressed: () {
        clientClose(widget.ffi.dialogManager);
      },
      icon: const Icon(
        Icons.close,
        color: _MenubarTheme.commonColor,
      ),
    );
  }

  List<MenuEntryBase<String>> _getControlMenu() {
    final pi = widget.ffi.ffiModel.pi;
    final perms = widget.ffi.ffiModel.permissions;

    final List<MenuEntryBase<String>> displayMenu = [];

    if (pi.version.isNotEmpty) {
      displayMenu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Refresh'),
          style: style,
        ),
        proc: () {
          bind.sessionRefresh(id: widget.id);
        },
        dismissOnClicked: true,
      ));
    }
    displayMenu.add(MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('OS Password'),
        style: style,
      ),
      proc: () {
        showSetOSPassword(widget.id, false, widget.ffi.dialogManager);
      },
      dismissOnClicked: true,
    ));

    if (!isWebDesktop) {
      if (perms['keyboard'] != false && perms['clipboard'] != false) {
        displayMenu.add(MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Text(
            translate('Paste'),
            style: style,
          ),
          proc: () {
            () async {
              ClipboardData? data =
                  await Clipboard.getData(Clipboard.kTextPlain);
              if (data != null && data.text != null) {
                bind.sessionInputString(id: widget.id, value: data.text ?? "");
              }
            }();
          },
          dismissOnClicked: true,
        ));
      }

      displayMenu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Reset canvas'),
          style: style,
        ),
        proc: () {
          widget.ffi.cursorModel.reset();
        },
        dismissOnClicked: true,
      ));
    }

    if (perms['keyboard'] != false) {
      if (pi.platform == 'Linux' || pi.sasEnabled) {
        displayMenu.add(MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Text(
            '${translate("Insert")} Ctrl + Alt + Del',
            style: style,
          ),
          proc: () {
            bind.sessionCtrlAltDel(id: widget.id);
          },
          dismissOnClicked: true,
        ));
      }

      displayMenu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Insert Lock'),
          style: style,
        ),
        proc: () {
          bind.sessionLockScreen(id: widget.id);
        },
        dismissOnClicked: true,
      ));

      if (pi.platform == 'Windows') {
        displayMenu.add(MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Obx(() => Text(
                translate(
                    '${BlockInputState.find(widget.id).value ? "Unb" : "B"}lock user input'),
                style: style,
              )),
          proc: () {
            RxBool blockInput = BlockInputState.find(widget.id);
            bind.sessionToggleOption(
                id: widget.id,
                value: '${blockInput.value ? "un" : ""}block-input');
            blockInput.value = !blockInput.value;
          },
          dismissOnClicked: true,
        ));
      }
    }

    if (gFFI.ffiModel.permissions["restart"] != false &&
        (pi.platform == "Linux" ||
            pi.platform == "Windows" ||
            pi.platform == "Mac OS")) {
      displayMenu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Restart Remote Device'),
          style: style,
        ),
        proc: () {
          showRestartRemoteDevice(pi, widget.id, gFFI.dialogManager);
        },
        dismissOnClicked: true,
      ));
    }

    return displayMenu;
  }

  List<MenuEntryBase<String>> _getDisplayMenu() {
    final displayMenu = [
      MenuEntryRadios<String>(
          text: translate('Ratio'),
          optionsGetter: () => [
                MenuEntryRadioOption(
                    text: translate('Scale original'), value: 'original'),
                MenuEntryRadioOption(
                    text: translate('Scale adaptive'), value: 'adaptive'),
              ],
          curOptionGetter: () async {
            return await bind.sessionGetOption(
                    id: widget.id, arg: 'view-style') ??
                'adaptive';
          },
          optionSetter: (String oldValue, String newValue) async {
            await bind.sessionPeerOption(
                id: widget.id, name: "view-style", value: newValue);
            widget.ffi.canvasModel.updateViewStyle();
          }),
      MenuEntryDivider<String>(),
      MenuEntryRadios<String>(
          text: translate('Scroll Style'),
          optionsGetter: () => [
                MenuEntryRadioOption(
                    text: translate('ScrollAuto'), value: 'scrollauto'),
                MenuEntryRadioOption(
                    text: translate('Scrollbar'), value: 'scrollbar'),
              ],
          curOptionGetter: () async {
            return await bind.sessionGetOption(
                    id: widget.id, arg: 'scroll-style') ??
                '';
          },
          optionSetter: (String oldValue, String newValue) async {
            await bind.sessionPeerOption(
                id: widget.id, name: "scroll-style", value: newValue);
            widget.ffi.canvasModel.updateScrollStyle();
          }),
      MenuEntryDivider<String>(),
      MenuEntryRadios<String>(
          text: translate('Image Quality'),
          optionsGetter: () => [
                MenuEntryRadioOption(
                    text: translate('Good image quality'), value: 'best'),
                MenuEntryRadioOption(
                    text: translate('Balanced'), value: 'balanced'),
                MenuEntryRadioOption(
                    text: translate('Optimize reaction time'), value: 'low'),
                MenuEntryRadioOption(
                    text: translate('Custom'),
                    value: 'custom',
                    dismissOnClicked: true),
              ],
          curOptionGetter: () async {
            String quality =
                await bind.sessionGetImageQuality(id: widget.id) ?? 'balanced';
            if (quality == '') quality = 'balanced';
            return quality;
          },
          optionSetter: (String oldValue, String newValue) async {
            if (oldValue != newValue) {
              await bind.sessionSetImageQuality(id: widget.id, value: newValue);
            }

            if (newValue == 'custom') {
              final btnCancel = msgBoxButton(translate('Close'), () {
                widget.ffi.dialogManager.dismissAll();
              });
              final quality =
                  await bind.sessionGetCustomImageQuality(id: widget.id);
              final double initValue = quality != null && quality.isNotEmpty
                  ? quality[0].toDouble()
                  : 50.0;
              final RxDouble sliderValue = RxDouble(initValue);
              final rxReplay = rxdart.ReplaySubject<double>();
              rxReplay
                  .throttleTime(const Duration(milliseconds: 1000),
                      trailing: true, leading: false)
                  .listen((double v) {
                () async {
                  await bind.sessionSetCustomImageQuality(
                      id: widget.id, value: v.toInt());
                }();
              });
              final slider = Obx(() {
                return Slider(
                  value: sliderValue.value,
                  max: 100,
                  divisions: 100,
                  label: sliderValue.value.round().toString(),
                  onChanged: (double value) {
                    sliderValue.value = value;
                    rxReplay.add(value);
                  },
                );
              });
              msgBoxCommon(widget.ffi.dialogManager, 'Custom Image Quality',
                  slider, [btnCancel]);
            }
          }),
      MenuEntryDivider<String>(),
      MenuEntrySwitch<String>(
          text: translate('Show remote cursor'),
          getter: () async {
            return bind.sessionGetToggleOptionSync(
                id: widget.id, arg: 'show-remote-cursor');
          },
          setter: (bool v) async {
            await bind.sessionToggleOption(
                id: widget.id, value: 'show-remote-cursor');
          }),
      MenuEntrySwitch<String>(
          text: translate('Show quality monitor'),
          getter: () async {
            return bind.sessionGetToggleOptionSync(
                id: widget.id, arg: 'show-quality-monitor');
          },
          setter: (bool v) async {
            await bind.sessionToggleOption(
                id: widget.id, value: 'show-quality-monitor');
            widget.ffi.qualityMonitorModel.checkShowQualityMonitor(widget.id);
          }),
    ];

    final perms = widget.ffi.ffiModel.permissions;
    final pi = widget.ffi.ffiModel.pi;

    if (perms['audio'] != false) {
      displayMenu.add(_createSwitchMenuEntry('Mute', 'disable-audio'));
    }
    if (perms['keyboard'] != false) {
      if (perms['clipboard'] != false) {
        displayMenu.add(
            _createSwitchMenuEntry('Disable clipboard', 'disable-clipboard'));
      }
      displayMenu.add(_createSwitchMenuEntry(
          'Lock after session end', 'lock-after-session-end'));
      if (pi.platform == 'Windows') {
        displayMenu.add(MenuEntrySwitch2<String>(
            text: translate('Privacy mode'),
            getter: () {
              return PrivacyModeState.find(widget.id);
            },
            setter: (bool v) async {
              Navigator.pop(context);
              await bind.sessionToggleOption(
                  id: widget.id, value: 'privacy-mode');
            }));
      }
    }
    return displayMenu;
  }

  MenuEntrySwitch<String> _createSwitchMenuEntry(String text, String option) {
    return MenuEntrySwitch<String>(
        text: translate(text),
        getter: () async {
          return bind.sessionGetToggleOptionSync(id: widget.id, arg: option);
        },
        setter: (bool v) async {
          await bind.sessionToggleOption(id: widget.id, value: option);
        });
  }
}

void showSetOSPassword(
    String id, bool login, OverlayDialogManager dialogManager) async {
  final controller = TextEditingController();
  var password = await bind.sessionGetOption(id: id, arg: "os-password") ?? "";
  var autoLogin = await bind.sessionGetOption(id: id, arg: "auto-login") != "";
  controller.text = password;
  dialogManager.show((setState, close) {
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
          TextButton(
            style: flatButtonStyle,
            onPressed: () {
              close();
            },
            child: Text(translate('Cancel')),
          ),
          TextButton(
            style: flatButtonStyle,
            onPressed: () {
              var text = controller.text.trim();
              bind.sessionPeerOption(id: id, name: "os-password", value: text);
              bind.sessionPeerOption(
                  id: id, name: "auto-login", value: autoLogin ? 'Y' : '');
              if (text != "" && login) {
                bind.sessionInputOsPassword(id: id, value: text);
              }
              close();
            },
            child: Text(translate('OK')),
          ),
        ]);
  });
}
