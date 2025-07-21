import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:get/get.dart';

import '../../models/platform_model.dart';
import 'terminal_page.dart';
import 'terminal_connection_manager.dart';
import '../widgets/material_mod_popup_menu.dart' as mod_menu;
import '../widgets/popup_menu.dart';
import 'package:bot_toast/bot_toast.dart';

class TerminalTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const TerminalTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<TerminalTabPage> createState() => _TerminalTabPageState(params);
}

class _TerminalTabPageState extends State<TerminalTabPage> {
  DesktopTabController get tabController => Get.find<DesktopTabController>();

  static const IconData selectedIcon = Icons.terminal;
  static const IconData unselectedIcon = Icons.terminal_outlined;
  int _nextTerminalId = 1;

  _TerminalTabPageState(Map<String, dynamic> params) {
    Get.put(DesktopTabController(tabType: DesktopTabType.terminal));
    tabController.onSelected = (id) {
      WindowController.fromWindowId(windowId())
          .setTitle(getWindowNameWithId(id));
    };
    tabController.onRemoved = (_, id) => onRemoveId(id);
    final terminalId = params['terminalId'] ?? _nextTerminalId++;
    tabController.add(_createTerminalTab(
      peerId: params['id'],
      terminalId: terminalId,
      password: params['password'],
      isSharedPassword: params['isSharedPassword'],
      forceRelay: params['forceRelay'],
      connToken: params['connToken'],
    ));
  }

  TabInfo _createTerminalTab({
    required String peerId,
    required int terminalId,
    String? password,
    bool? isSharedPassword,
    bool? forceRelay,
    String? connToken,
  }) {
    final tabKey = '${peerId}_$terminalId';
    return TabInfo(
      key: tabKey,
      label: '$peerId #$terminalId',
      selectedIcon: selectedIcon,
      unselectedIcon: unselectedIcon,
      onTabCloseButton: () async {
        // Close the terminal session first
        final ffi = TerminalConnectionManager.getExistingConnection(peerId);
        if (ffi != null) {
          final terminalModel = ffi.terminalModels[terminalId];
          if (terminalModel != null) {
            await terminalModel.closeTerminal();
          }
        }
        // Then close the tab
        tabController.closeBy(tabKey);
      },
      page: TerminalPage(
        key: ValueKey(tabKey),
        id: peerId,
        terminalId: terminalId,
        password: password,
        isSharedPassword: isSharedPassword,
        tabController: tabController,
        forceRelay: forceRelay,
        connToken: connToken,
      ),
    );
  }

  Widget _tabMenuBuilder(String peerId, CancelFunc cancelFunc) {
    final List<MenuEntryBase<String>> menu = [];
    const EdgeInsets padding = EdgeInsets.only(left: 8.0, right: 5.0);

    // New tab menu item
    menu.add(MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('New tab'),
        style: style,
      ),
      proc: () {
        _addNewTerminal(peerId);
        cancelFunc();
        // Also try to close any BotToast overlays
        BotToast.cleanAll();
      },
      padding: padding,
    ));

    menu.add(MenuEntryDivider());

    menu.add(MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Keep terminal sessions on disconnect'),
      getter: () async {
        final ffi = Get.find<FFI>(tag: 'terminal_$peerId');
        return bind.sessionGetToggleOptionSync(
          sessionId: ffi.sessionId,
          arg: kOptionTerminalPersistent,
        );
      },
      setter: (bool v) async {
        final ffi = Get.find<FFI>(tag: 'terminal_$peerId');
        await bind.sessionToggleOption(
          sessionId: ffi.sessionId,
          value: kOptionTerminalPersistent,
        );
      },
      padding: padding,
    ));

    return mod_menu.PopupMenu<String>(
      items: menu
          .map((e) => e.build(
                context,
                const MenuConfig(
                  commonColor: CustomPopupMenuTheme.commonColor,
                  height: CustomPopupMenuTheme.height,
                  dividerHeight: CustomPopupMenuTheme.dividerHeight,
                ),
              ))
          .expand((i) => i)
          .toList(),
    );
  }

  @override
  void initState() {
    super.initState();

    // Add keyboard shortcut handler
    HardwareKeyboard.instance.addHandler(_handleKeyEvent);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "[Remote Terminal] call ${call.method} with args ${call.arguments} from window $fromWindowId");
      if (call.method == kWindowEventNewTerminal) {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        windowOnTop(windowId());
        // Allow multiple terminals for the same connection
        final terminalId = args['terminalId'] ?? _nextTerminalId++;
        tabController.add(_createTerminalTab(
          peerId: id,
          terminalId: terminalId,
          password: args['password'],
          isSharedPassword: args['isSharedPassword'],
          forceRelay: args['forceRelay'],
          connToken: args['connToken'],
        ));
      } else if (call.method == kWindowEventRestoreTerminalSessions) {
        _restoreSessions(call.arguments);
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.Terminal, windowId: windowId());
    });
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    super.dispose();
  }

  Future<void> _restoreSessions(String arguments) async {
    Map<String, dynamic>? args;
    try {
      args = jsonDecode(arguments) as Map<String, dynamic>;
    } catch (e) {
      debugPrint("Error parsing JSON arguments in _restoreSessions: $e");
      return;
    }
    final persistentSessions =
        args['persistent_sessions'] as List<dynamic>? ?? [];
    final sortedSessions = persistentSessions.whereType<int>().toList()..sort();
    for (final terminalId in sortedSessions) {
      _addNewTerminalForCurrentPeer(terminalId: terminalId);
      // A delay is required to ensure the UI has sufficient time to update
      // before adding the next terminal. Without this delay, `_TerminalPageState::dispose()`
      // may be called prematurely while the tab widget is still in the tab controller.
      // This behavior is likely due to a race condition between the UI rendering lifecycle
      // and the addition of new tabs. Attempts to use `_TerminalPageState::addPostFrameCallback()`
      // to wait for the previous page to be ready were unsuccessful, as the observed call sequence is:
      // `initState() 2 -> dispose() 2 -> postFrameCallback() 2`, followed by `initState() 3`.
      // The `Future.delayed` approach mitigates this issue by introducing a buffer period,
      // allowing the UI to stabilize before proceeding.
      await Future.delayed(const Duration(milliseconds: 300));
    }
  }

  bool _handleKeyEvent(KeyEvent event) {
    if (event is KeyDownEvent) {
      // Use Cmd+T on macOS, Ctrl+Shift+T on other platforms
      if (event.logicalKey == LogicalKeyboardKey.keyT) {
        if (isMacOS &&
            HardwareKeyboard.instance.isMetaPressed &&
            !HardwareKeyboard.instance.isShiftPressed) {
          // macOS: Cmd+T (standard for new tab)
          _addNewTerminalForCurrentPeer();
          return true;
        } else if (!isMacOS &&
            HardwareKeyboard.instance.isControlPressed &&
            HardwareKeyboard.instance.isShiftPressed) {
          // Other platforms: Ctrl+Shift+T (to avoid conflict with Ctrl+T in terminal)
          _addNewTerminalForCurrentPeer();
          return true;
        }
      }

      // Use Cmd+W on macOS, Ctrl+Shift+W on other platforms
      if (event.logicalKey == LogicalKeyboardKey.keyW) {
        if (isMacOS &&
            HardwareKeyboard.instance.isMetaPressed &&
            !HardwareKeyboard.instance.isShiftPressed) {
          // macOS: Cmd+W (standard for close tab)
          final currentTab = tabController.state.value.selectedTabInfo;
          if (tabController.state.value.tabs.length > 1) {
            tabController.closeBy(currentTab.key);
            return true;
          }
        } else if (!isMacOS &&
            HardwareKeyboard.instance.isControlPressed &&
            HardwareKeyboard.instance.isShiftPressed) {
          // Other platforms: Ctrl+Shift+W (to avoid conflict with Ctrl+W word delete)
          final currentTab = tabController.state.value.selectedTabInfo;
          if (tabController.state.value.tabs.length > 1) {
            tabController.closeBy(currentTab.key);
            return true;
          }
        }
      }

      // Use Alt+Left/Right for tab navigation (avoids conflicts)
      if (HardwareKeyboard.instance.isAltPressed) {
        if (event.logicalKey == LogicalKeyboardKey.arrowLeft) {
          // Previous tab
          final currentIndex = tabController.state.value.selected;
          if (currentIndex > 0) {
            tabController.jumpTo(currentIndex - 1);
          }
          return true;
        } else if (event.logicalKey == LogicalKeyboardKey.arrowRight) {
          // Next tab
          final currentIndex = tabController.state.value.selected;
          if (currentIndex < tabController.length - 1) {
            tabController.jumpTo(currentIndex + 1);
          }
          return true;
        }
      }

      // Check for Cmd/Ctrl + Number (switch to specific tab)
      final numberKeys = [
        LogicalKeyboardKey.digit1,
        LogicalKeyboardKey.digit2,
        LogicalKeyboardKey.digit3,
        LogicalKeyboardKey.digit4,
        LogicalKeyboardKey.digit5,
        LogicalKeyboardKey.digit6,
        LogicalKeyboardKey.digit7,
        LogicalKeyboardKey.digit8,
        LogicalKeyboardKey.digit9,
      ];

      for (int i = 0; i < numberKeys.length; i++) {
        if (event.logicalKey == numberKeys[i] &&
            ((isMacOS && HardwareKeyboard.instance.isMetaPressed) ||
                (!isMacOS && HardwareKeyboard.instance.isControlPressed))) {
          if (i < tabController.length) {
            tabController.jumpTo(i);
            return true;
          }
        }
      }
    }
    return false;
  }

  void _addNewTerminal(String peerId, {int? terminalId}) {
    // Find first tab for this peer to get connection parameters
    final firstTab = tabController.state.value.tabs.firstWhere(
      (tab) => tab.key.startsWith('$peerId\_'),
    );
    if (firstTab.page is TerminalPage) {
      final page = firstTab.page as TerminalPage;
      final newTerminalId = terminalId ?? _nextTerminalId++;
      if (terminalId != null && terminalId >= _nextTerminalId) {
        _nextTerminalId = terminalId + 1;
      }
      tabController.add(_createTerminalTab(
        peerId: peerId,
        terminalId: newTerminalId,
        password: page.password,
        isSharedPassword: page.isSharedPassword,
        forceRelay: page.forceRelay,
        connToken: page.connToken,
      ));
    }
  }

  void _addNewTerminalForCurrentPeer({int? terminalId}) {
    final currentTab = tabController.state.value.selectedTabInfo;
    final parts = currentTab.key.split('_');
    if (parts.isNotEmpty) {
      final peerId = parts[0];
      _addNewTerminal(peerId, terminalId: terminalId);
    }
  }

  @override
  Widget build(BuildContext context) {
    final child = Scaffold(
        backgroundColor: Theme.of(context).cardColor,
        body: DesktopTab(
          controller: tabController,
          onWindowCloseButton: handleWindowCloseButton,
          tail: _buildAddButton(),
          selectedBorderColor: MyTheme.accent,
          labelGetter: DesktopTab.tablabelGetter,
          tabMenuBuilder: (key) {
            // Extract peerId from tab key (format: "peerId_terminalId")
            final parts = key.split('_');
            if (parts.isEmpty) return Container();
            final peerId = parts[0];
            return _tabMenuBuilder(peerId, () {});
          },
        ));
    final tabWidget = isLinux
        ? buildVirtualWindowFrame(context, child)
        : workaroundWindowBorder(
            context,
            Container(
              decoration: BoxDecoration(
                  border: Border.all(color: MyTheme.color(context).border!)),
              child: child,
            ));
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : SubWindowDragToResizeArea(
            child: tabWidget,
            resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
            enableResizeEdges: subWindowManagerEnableResizeEdges,
            windowId: stateGlobal.windowId,
          );
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }

  Widget _buildAddButton() {
    return ActionIcon(
      message: 'New tab',
      icon: IconFont.add,
      onTap: () {
        _addNewTerminalForCurrentPeer();
      },
      isClose: false,
    );
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.state.value.tabs.length;
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
}
