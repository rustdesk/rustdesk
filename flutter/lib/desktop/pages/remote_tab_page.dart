import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/remote_menubar.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:bot_toast/bot_toast.dart';

import '../../models/platform_model.dart';

class _MenuTheme {
  static const Color commonColor = MyTheme.accent;
  // kMinInteractiveDimension
  static const double height = 20.0;
  static const double dividerHeight = 12.0;
}

class ConnectionTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const ConnectionTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<ConnectionTabPage> createState() => _ConnectionTabPageState(params);
}

class _ConnectionTabPageState extends State<ConnectionTabPage> {
  final tabController = Get.put(DesktopTabController(
      tabType: DesktopTabType.remoteScreen,
      onSelected: (_, id) => bind.setCurSessionId(id: id)));
  static const IconData selectedIcon = Icons.desktop_windows_sharp;
  static const IconData unselectedIcon = Icons.desktop_windows_outlined;

  late MenubarState _menubarState;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    _menubarState = MenubarState();
    RemoteCountState.init();
    final peerId = params['id'];
    if (peerId != null) {
      ConnectionTypeState.init(peerId);
      tabController.add(TabInfo(
        key: peerId,
        label: peerId,
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        onTabCloseButton: () => tabController.closeBy(peerId),
        page: RemotePage(
          key: ValueKey(peerId),
          id: peerId,
          menubarState: _menubarState,
        ),
      ));
      _update_remote_count();
    }
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemoved = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "[Remote Page] call ${call.method} with args ${call.arguments} from window $fromWindowId");

      // for simplify, just replace connectionId
      if (call.method == "new_remote_desktop") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        ConnectionTypeState.init(id);
        window_on_top(windowId());
        ConnectionTypeState.init(id);
        tabController.add(TabInfo(
          key: id,
          label: id,
          selectedIcon: selectedIcon,
          unselectedIcon: unselectedIcon,
          onTabCloseButton: () => tabController.closeBy(id),
          page: RemotePage(
            key: ValueKey(id),
            id: id,
            menubarState: _menubarState,
          ),
        ));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
      _update_remote_count();
    });
  }

  @override
  void dispose() {
    super.dispose();
    _menubarState.save();
  }

  @override
  Widget build(BuildContext context) {
    final tabWidget = Obx(
      () => Container(
        decoration: BoxDecoration(
          border: Border.all(
              color: MyTheme.color(context).border!,
              width: stateGlobal.windowBorderWidth.value),
        ),
        child: Scaffold(
          backgroundColor: Theme.of(context).backgroundColor,
          body: DesktopTab(
            controller: tabController,
            onWindowCloseButton: handleWindowCloseButton,
            tail: const AddButton().paddingOnly(left: 10),
            pageViewBuilder: (pageView) => pageView,
            labelGetter: DesktopTab.labelGetterAlias,
            tabBuilder: (key, icon, label, themeConf) => Obx(() {
              final connectionType = ConnectionTypeState.find(key);
              if (!connectionType.isValid()) {
                return Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    icon,
                    label,
                  ],
                );
              } else {
                final msgDirect = translate(
                    connectionType.direct.value == ConnectionType.strDirect
                        ? 'Direct Connection'
                        : 'Relay Connection');
                final msgSecure = translate(
                    connectionType.secure.value == ConnectionType.strSecure
                        ? 'Secure Connection'
                        : 'Insecure Connection');
                final tab = Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    icon,
                    Tooltip(
                      message: '$msgDirect\n$msgSecure',
                      child: SvgPicture.asset(
                        'assets/${connectionType.secure.value}${connectionType.direct.value}.svg',
                        width: themeConf.iconSize,
                        height: themeConf.iconSize,
                      ).paddingOnly(right: 5),
                    ),
                    label,
                  ],
                );

                return Listener(
                  onPointerDown: (e) {
                    if (e.kind != ui.PointerDeviceKind.mouse) {
                      return;
                    }
                    if (e.buttons == 2) {
                      showRightMenu(
                        (CancelFunc cancelFunc) {
                          return _tabMenuBuilder(key, cancelFunc);
                        },
                        target: e.position,
                      );
                    }
                  },
                  child: tab,
                );
              }
            }),
          ),
        ),
      ),
    );
    return Platform.isMacOS
        ? tabWidget
        : SubWindowDragToResizeArea(
            child: tabWidget,
            resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
            windowId: stateGlobal.windowId,
          );
  }

  // Note: Some dup code to ../widgets/remote_menubar
  Widget _tabMenuBuilder(String key, CancelFunc cancelFunc) {
    final List<MenuEntryBase<String>> menu = [];
    const EdgeInsets padding = EdgeInsets.only(left: 8.0, right: 5.0);
    final remotePage = tabController.state.value.tabs
        .firstWhere((tab) => tab.key == key)
        .page as RemotePage;
    final ffi = remotePage.ffi;
    final pi = ffi.ffiModel.pi;
    final perms = ffi.ffiModel.permissions;
    menu.addAll([
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Close'),
          style: style,
        ),
        proc: () {
          tabController.closeBy(key);
          cancelFunc();
        },
        padding: padding,
      ),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Obx(() => Text(
              translate(
                  _menubarState.show.isTrue ? 'Hide Menubar' : 'Show Menubar'),
              style: style,
            )),
        proc: () {
          _menubarState.switchShow();
          cancelFunc();
        },
        padding: padding,
      ),
      MenuEntryDivider<String>(),
      MenuEntryRadios<String>(
        text: translate('Ratio'),
        optionsGetter: () => [
          MenuEntryRadioOption(
            text: translate('Scale original'),
            value: 'original',
            dismissOnClicked: true,
          ),
          MenuEntryRadioOption(
            text: translate('Scale adaptive'),
            value: 'adaptive',
            dismissOnClicked: true,
          ),
        ],
        curOptionGetter: () async =>
            // null means peer id is not found, which there's no need to care about
            await bind.sessionGetViewStyle(id: key) ?? '',
        optionSetter: (String oldValue, String newValue) async {
          await bind.sessionSetViewStyle(
              id: key, value: newValue);
          ffi.canvasModel.updateViewStyle();
          cancelFunc();
        },
        padding: padding,
      ),
      MenuEntryDivider<String>(),
      () {
        final state = ShowRemoteCursorState.find(key);
        return MenuEntrySwitch2<String>(
          switchType: SwitchType.scheckbox,
          text: translate('Show remote cursor'),
          getter: () {
            return state;
          },
          setter: (bool v) async {
            state.value = v;
            await bind.sessionToggleOption(
                id: key, value: 'show-remote-cursor');
            cancelFunc();
          },
          padding: padding,
        );
      }()
    ]);

    if (perms['keyboard'] != false) {
      if (perms['clipboard'] != false) {
        menu.add(MenuEntrySwitch<String>(
          switchType: SwitchType.scheckbox,
          text: translate('Disable clipboard'),
          getter: () async {
            return bind.sessionGetToggleOptionSync(
                id: key, arg: 'disable-clipboard');
          },
          setter: (bool v) async {
            await bind.sessionToggleOption(id: key, value: 'disable-clipboard');
            cancelFunc();
          },
          padding: padding,
        ));
      }

      menu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Insert Lock'),
          style: style,
        ),
        proc: () {
          bind.sessionLockScreen(id: key);
          cancelFunc();
        },
        padding: padding,
        dismissOnClicked: true,
      ));

      if (pi.platform == 'Linux' || pi.sasEnabled) {
        menu.add(MenuEntryButton<String>(
          childBuilder: (TextStyle? style) => Text(
            '${translate("Insert")} Ctrl + Alt + Del',
            style: style,
          ),
          proc: () {
            bind.sessionCtrlAltDel(id: key);
            cancelFunc();
          },
          padding: padding,
          dismissOnClicked: true,
        ));
      }
    }

    return mod_menu.PopupMenu<String>(
      items: menu
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenuTheme.commonColor,
                height: _MenuTheme.height,
                dividerHeight: _MenuTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  void onRemoveId(String id) async {
    if (tabController.state.value.tabs.isEmpty) {
      await WindowController.fromWindowId(windowId()).close();
    }
    ConnectionTypeState.delete(id);
    _update_remote_count();
  }

  int windowId() {
    return widget.params["windowId"];
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.length;
    if (connLength <= 1) {
      tabController.clear();
      return true;
    } else {
      final opt = "enable-confirm-closing-tabs";
      final bool res;
      if (!option2bool(opt, await bind.mainGetOption(key: opt))) {
        res = true;
      } else {
        res = await closeConfirmDialog();
      }
      if (res) {
        tabController.clear();
      }
      return res;
    }
  }

  _update_remote_count() =>
      RemoteCountState.find().value = tabController.length;
}
