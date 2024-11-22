import 'dart:convert';
import 'dart:async';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:bot_toast/bot_toast.dart';

import '../../common/widgets/dialog.dart';
import '../../models/platform_model.dart';

class _MenuTheme {
  static const Color blueColor = MyTheme.button;
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
  final tabController =
      Get.put(DesktopTabController(tabType: DesktopTabType.remoteScreen));
  final contentKey = UniqueKey();
  static const IconData selectedIcon = Icons.desktop_windows_sharp;
  static const IconData unselectedIcon = Icons.desktop_windows_outlined;

  String? peerId;
  bool _isScreenRectSet = false;
  int? _display;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    RemoteCountState.init();
    peerId = params['id'];
    final sessionId = params['session_id'];
    final tabWindowId = params['tab_window_id'];
    final display = params['display'];
    final displays = params['displays'];
    final screenRect = parseParamScreenRect(params);
    _isScreenRectSet = screenRect != null;
    _display = display as int?;
    tryMoveToScreenAndSetFullscreen(screenRect);
    if (peerId != null) {
      ConnectionTypeState.init(peerId!);
      tabController.onSelected = (id) {
        final remotePage = tabController.widget(id);
        if (remotePage is RemotePage) {
          final ffi = remotePage.ffi;
          bind.setCurSessionId(sessionId: ffi.sessionId);
        }
        WindowController.fromWindowId(params['windowId'])
            .setTitle(getWindowNameWithId(id));
        UnreadChatCountState.find(id).value = 0;
      };
      tabController.add(TabInfo(
        key: peerId!,
        label: peerId!,
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        onTabCloseButton: () => tabController.closeBy(peerId),
        page: RemotePage(
          key: ValueKey(peerId),
          id: peerId!,
          sessionId: sessionId == null ? null : SessionID(sessionId),
          tabWindowId: tabWindowId,
          display: display,
          displays: displays?.cast<int>(),
          password: params['password'],
          toolbarState: ToolbarState(),
          tabController: tabController,
          switchUuid: params['switch_uuid'],
          forceRelay: params['forceRelay'],
          isSharedPassword: params['isSharedPassword'],
        ),
      ));
      _update_remote_count();
    }
    tabController.onRemoved = (_, id) => onRemoveId(id);
    rustDeskWinManager.setMethodHandler(_remoteMethodHandler);
  }

  @override
  void initState() {
    super.initState();

    if (!_isScreenRectSet) {
      Future.delayed(Duration.zero, () {
        restoreWindowPosition(
          WindowType.RemoteDesktop,
          windowId: windowId(),
          peerId: tabController.state.value.tabs.isEmpty
              ? null
              : tabController.state.value.tabs[0].key,
          display: _display,
        );
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final child = Scaffold(
      backgroundColor: Theme.of(context).colorScheme.background,
      body: DesktopTab(
        controller: tabController,
        onWindowCloseButton: handleWindowCloseButton,
        tail: const AddButton(),
        selectedBorderColor: MyTheme.accent,
        pageViewBuilder: (pageView) => pageView,
        labelGetter: DesktopTab.tablabelGetter,
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
            bool secure =
                connectionType.secure.value == ConnectionType.strSecure;
            bool direct =
                connectionType.direct.value == ConnectionType.strDirect;
            String msgConn;
            if (secure && direct) {
              msgConn = translate("Direct and encrypted connection");
            } else if (secure && !direct) {
              msgConn = translate("Relayed and encrypted connection");
            } else if (!secure && direct) {
              msgConn = translate("Direct and unencrypted connection");
            } else {
              msgConn = translate("Relayed and unencrypted connection");
            }
            var msgFingerprint = '${translate('Fingerprint')}:\n';
            var fingerprint = FingerprintState.find(key).value;
            if (fingerprint.isEmpty) {
              fingerprint = 'N/A';
            }
            if (fingerprint.length > 5 * 8) {
              var first = fingerprint.substring(0, 39);
              var second = fingerprint.substring(40);
              msgFingerprint += '$first\n$second';
            } else {
              msgFingerprint += fingerprint;
            }

            final tab = Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                icon,
                Tooltip(
                  message: '$msgConn\n$msgFingerprint',
                  child: SvgPicture.asset(
                    'assets/${connectionType.secure.value}${connectionType.direct.value}.svg',
                    width: themeConf.iconSize,
                    height: themeConf.iconSize,
                  ).paddingOnly(right: 5),
                ),
                label,
                unreadMessageCountBuilder(UnreadChatCountState.find(key))
                    .marginOnly(left: 4),
              ],
            );

            return Listener(
              onPointerDown: (e) {
                if (e.kind != ui.PointerDeviceKind.mouse) {
                  return;
                }
                final remotePage = tabController.state.value.tabs
                    .firstWhere((tab) => tab.key == key)
                    .page as RemotePage;
                if (remotePage.ffi.ffiModel.pi.isSet.isTrue && e.buttons == 2) {
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
    );
    final tabWidget = isLinux
        ? buildVirtualWindowFrame(context, child)
        : Obx(() => Container(
              decoration: BoxDecoration(
                border: Border.all(
                    color: MyTheme.color(context).border!,
                    width: stateGlobal.windowBorderWidth.value),
              ),
              child: child,
            ));
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : Obx(() => SubWindowDragToResizeArea(
              key: contentKey,
              child: tabWidget,
              // Specially configured for a better resize area and remote control.
              childPadding: kDragToResizeAreaPadding,
              resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
              enableResizeEdges: subWindowManagerEnableResizeEdges,
              windowId: stateGlobal.windowId,
            ));
  }

  // Note: Some dup code to ../widgets/remote_toolbar
  Widget _tabMenuBuilder(String key, CancelFunc cancelFunc) {
    final List<MenuEntryBase<String>> menu = [];
    const EdgeInsets padding = EdgeInsets.only(left: 8.0, right: 5.0);
    final remotePage = tabController.state.value.tabs
        .firstWhere((tab) => tab.key == key)
        .page as RemotePage;
    final ffi = remotePage.ffi;
    final pi = ffi.ffiModel.pi;
    final perms = ffi.ffiModel.permissions;
    final sessionId = ffi.sessionId;
    final toolbarState = remotePage.toolbarState;
    menu.addAll([
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Obx(() => Text(
              translate(
                  toolbarState.show.isTrue ? 'Hide Toolbar' : 'Show Toolbar'),
              style: style,
            )),
        proc: () {
          toolbarState.switchShow(sessionId);
          cancelFunc();
        },
        padding: padding,
      ),
    ]);

    if (tabController.state.value.tabs.length > 1) {
      final splitAction = MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Move tab to new window'),
          style: style,
        ),
        proc: () async {
          await DesktopMultiWindow.invokeMethod(kMainWindowId,
              kWindowEventMoveTabToNewWindow, '${windowId()},$key,$sessionId');
          cancelFunc();
        },
        padding: padding,
      );
      menu.insert(1, splitAction);
    }

    if (perms['restart'] != false &&
        (pi.platform == kPeerPlatformLinux ||
            pi.platform == kPeerPlatformWindows ||
            pi.platform == kPeerPlatformMacOS)) {
      menu.add(MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Restart remote device'),
          style: style,
        ),
        proc: () => showRestartRemoteDevice(
            pi, peerId ?? '', sessionId, ffi.dialogManager),
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: cancelFunc,
      ));
    }

    if (perms['keyboard'] != false && !ffi.ffiModel.viewOnly) {
      menu.add(RemoteMenuEntry.insertLock(sessionId, padding,
          dismissFunc: cancelFunc));

      if (pi.platform == kPeerPlatformLinux || pi.sasEnabled) {
        menu.add(RemoteMenuEntry.insertCtrlAltDel(sessionId, padding,
            dismissFunc: cancelFunc));
      }
    }

    menu.addAll([
      MenuEntryDivider<String>(),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Copy Fingerprint'),
          style: style,
        ),
        proc: () => onCopyFingerprint(FingerprintState.find(key).value),
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: cancelFunc,
      ),
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
      )
    ]);

    return mod_menu.PopupMenu<String>(
      items: menu
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenuTheme.blueColor,
                height: _MenuTheme.height,
                dividerHeight: _MenuTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  void onRemoveId(String id) async {
    if (tabController.state.value.tabs.isEmpty) {
      // Keep calling until the window status is hidden.
      //
      // Workaround for Windows:
      // If you click other buttons and close in msgbox within a very short period of time, the close may fail.
      // `await WindowController.fromWindowId(windowId()).close();`.
      Future<void> loopCloseWindow() async {
        int c = 0;
        final windowController = WindowController.fromWindowId(windowId());
        while (c < 20 &&
            tabController.state.value.tabs.isEmpty &&
            (!await windowController.isHidden())) {
          await windowController.close();
          await Future.delayed(Duration(milliseconds: 100));
          c++;
        }
      }

      loopCloseWindow();
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
      final bool res;
      if (!option2bool(kOptionEnableConfirmClosingTabs,
          bind.mainGetLocalOption(key: kOptionEnableConfirmClosingTabs))) {
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

  Future<dynamic> _remoteMethodHandler(call, fromWindowId) async {
    debugPrint(
        "[Remote Page] call ${call.method} with args ${call.arguments} from window $fromWindowId");

    dynamic returnValue;
    // for simplify, just replace connectionId
    if (call.method == kWindowEventNewRemoteDesktop) {
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final switchUuid = args['switch_uuid'];
      final sessionId = args['session_id'];
      final tabWindowId = args['tab_window_id'];
      final display = args['display'];
      final displays = args['displays'];
      final screenRect = parseParamScreenRect(args);
      final prePeerCount = tabController.length;
      Future.delayed(Duration.zero, () async {
        if (stateGlobal.fullscreen.isTrue) {
          await WindowController.fromWindowId(windowId()).setFullscreen(false);
          stateGlobal.setFullscreen(false, procWnd: false);
        }
        await setNewConnectWindowFrame(
            windowId(), id!, prePeerCount, display, screenRect);
        Future.delayed(Duration(milliseconds: isWindows ? 100 : 0), () async {
          await windowOnTop(windowId());
        });
      });
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
          sessionId: sessionId == null ? null : SessionID(sessionId),
          tabWindowId: tabWindowId,
          display: display,
          displays: displays?.cast<int>(),
          password: args['password'],
          toolbarState: ToolbarState(),
          tabController: tabController,
          switchUuid: switchUuid,
          forceRelay: args['forceRelay'],
          isSharedPassword: args['isSharedPassword'],
        ),
      ));
    } else if (call.method == kWindowDisableGrabKeyboard) {
      // ???
    } else if (call.method == "onDestroy") {
      tabController.clear();
    } else if (call.method == kWindowActionRebuild) {
      reloadCurrentWindow();
    } else if (call.method == kWindowEventActiveSession) {
      final jumpOk = tabController.jumpToByKey(call.arguments);
      if (jumpOk) {
        windowOnTop(windowId());
      }
      return jumpOk;
    } else if (call.method == kWindowEventActiveDisplaySession) {
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final display = args['display'];
      final jumpOk = tabController.jumpToByKeyAndDisplay(id, display);
      if (jumpOk) {
        windowOnTop(windowId());
      }
      return jumpOk;
    } else if (call.method == kWindowEventGetRemoteList) {
      return tabController.state.value.tabs
          .map((e) => e.key)
          .toList()
          .join(',');
    } else if (call.method == kWindowEventGetSessionIdList) {
      return tabController.state.value.tabs
          .map((e) => '${e.key},${(e.page as RemotePage).ffi.sessionId}')
          .toList()
          .join(';');
    } else if (call.method == kWindowEventGetCachedSessionData) {
      // Ready to show new window and close old tab.
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final close = args['close'];
      try {
        final remotePage = tabController.state.value.tabs
            .firstWhere((tab) => tab.key == id)
            .page as RemotePage;
        returnValue = remotePage.ffi.ffiModel.cachedPeerData.toString();
      } catch (e) {
        debugPrint('Failed to get cached session data: $e');
      }
      if (close && returnValue != null) {
        closeSessionOnDispose[id] = false;
        tabController.closeBy(id);
      }
    } else if (call.method == kWindowEventRemoteWindowCoords) {
      final remotePage =
          tabController.state.value.selectedTabInfo.page as RemotePage;
      final ffi = remotePage.ffi;
      final displayRect = ffi.ffiModel.displaysRect();
      if (displayRect != null) {
        final wc = WindowController.fromWindowId(windowId());
        Rect? frame;
        try {
          frame = await wc.getFrame();
        } catch (e) {
          debugPrint(
              "Failed to get frame of window $windowId, it may be hidden");
        }
        if (frame != null) {
          ffi.cursorModel.moveLocal(0, 0);
          final coords = RemoteWindowCoords(
              frame,
              CanvasCoords.fromCanvasModel(ffi.canvasModel),
              CursorCoords.fromCursorModel(ffi.cursorModel),
              displayRect);
          returnValue = jsonEncode(coords.toJson());
        }
      }
    } else if (call.method == kWindowEventSetFullscreen) {
      stateGlobal.setFullscreen(call.arguments == 'true');
    }
    _update_remote_count();
    return returnValue;
  }
}
