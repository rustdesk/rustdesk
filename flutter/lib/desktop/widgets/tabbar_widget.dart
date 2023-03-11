import 'dart:async';
import 'dart:io';
import 'dart:math';
import 'dart:ui' as ui;

import 'package:bot_toast/bot_toast.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart' hide TabBarTheme;
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:get/get_rx/src/rx_workers/utils/debouncer.dart';
import 'package:scroll_pos/scroll_pos.dart';
import 'package:window_manager/window_manager.dart';

import '../../utils/multi_window_manager.dart';

const double _kTabBarHeight = kDesktopRemoteTabBarHeight;
const double _kIconSize = 18;
const double _kDividerIndent = 10;
const double _kActionIconSize = 12;

class TabInfo {
  final String key;
  final String label;
  final IconData? selectedIcon;
  final IconData? unselectedIcon;
  final bool closable;
  final VoidCallback? onTabCloseButton;
  final VoidCallback? onTap;
  final Widget page;

  TabInfo(
      {required this.key,
      required this.label,
      this.selectedIcon,
      this.unselectedIcon,
      this.closable = true,
      this.onTabCloseButton,
      this.onTap,
      required this.page});
}

enum DesktopTabType {
  main,
  cm,
  remoteScreen,
  fileTransfer,
  portForward,
  install,
}

class DesktopTabState {
  final List<TabInfo> tabs = [];
  final ScrollPosController scrollController =
      ScrollPosController(itemCount: 0);
  final PageController pageController = PageController();
  int selected = 0;

  TabInfo get selectedTabInfo => tabs[selected];

  DesktopTabState() {
    scrollController.itemCount = tabs.length;
  }
}

CancelFunc showRightMenu(ToastBuilder builder,
    {BuildContext? context, Offset? target}) {
  return BotToast.showAttachedWidget(
    target: target,
    targetContext: context,
    verticalOffset: 0,
    horizontalOffset: 0,
    duration: Duration(seconds: 4),
    animationDuration: Duration(milliseconds: 0),
    animationReverseDuration: Duration(milliseconds: 0),
    preferDirection: PreferDirection.rightTop,
    ignoreContentClick: false,
    onlyOne: true,
    allowClick: true,
    enableSafeArea: true,
    backgroundColor: Color(0x00000000),
    attachedBuilder: builder,
  );
}

class DesktopTabController {
  final state = DesktopTabState().obs;
  final DesktopTabType tabType;

  /// index, key
  Function(int, String)? onRemoved;
  Function(int, String)? onSelected;

  DesktopTabController(
      {required this.tabType, this.onRemoved, this.onSelected});

  int get length => state.value.tabs.length;

  void add(TabInfo tab) {
    if (!isDesktop) return;
    final index = state.value.tabs.indexWhere((e) => e.key == tab.key);
    int toIndex;
    if (index >= 0) {
      toIndex = index;
    } else {
      state.update((val) {
        val!.tabs.add(tab);
      });
      state.value.scrollController.itemCount = state.value.tabs.length;
      toIndex = state.value.tabs.length - 1;
      assert(toIndex >= 0);
    }
    try {
      jumpTo(toIndex);
    } catch (e) {
      // call before binding controller will throw
      debugPrint("Failed to jumpTo: $e");
    }
  }

  void remove(int index) {
    if (!isDesktop) return;
    final len = state.value.tabs.length;
    if (index < 0 || index > len - 1) return;
    final key = state.value.tabs[index].key;
    final currentSelected = state.value.selected;
    int toIndex = 0;
    if (index == len - 1) {
      toIndex = max(0, currentSelected - 1);
    } else if (index < len - 1 && index < currentSelected) {
      toIndex = max(0, currentSelected - 1);
    }
    state.value.tabs.removeAt(index);
    state.value.scrollController.itemCount = state.value.tabs.length;
    jumpTo(toIndex);
    onRemoved?.call(index, key);
  }

  void jumpTo(int index) {
    if (!isDesktop || index < 0) return;
    state.update((val) {
      val!.selected = index;
      Future.delayed(Duration(milliseconds: 100), (() {
        if (val.pageController.hasClients) {
          val.pageController.jumpToPage(index);
        }
        val.scrollController.itemCount = val.tabs.length;
        if (val.scrollController.hasClients &&
            val.scrollController.itemCount > index) {
          val.scrollController
              .scrollToItem(index, center: false, animate: true);
        }
      }));
    });
    if (state.value.tabs.length > index) {
      final key = state.value.tabs[index].key;
      onSelected?.call(index, key);
    }
  }

  void jumpBy(String key) {
    if (!isDesktop) return;
    final index = state.value.tabs.indexWhere((tab) => tab.key == key);
    jumpTo(index);
  }

  void closeBy(String? key) {
    if (!isDesktop) return;
    assert(onRemoved != null);
    if (key == null) {
      if (state.value.selected < state.value.tabs.length) {
        remove(state.value.selected);
      }
    } else {
      final index = state.value.tabs.indexWhere((tab) => tab.key == key);
      remove(index);
    }
  }

  void clear() {
    state.value.tabs.clear();
    state.refresh();
  }
}

class TabThemeConf {
  double iconSize;

  TabThemeConf({required this.iconSize});
}

typedef TabBuilder = Widget Function(
    String key, Widget icon, Widget label, TabThemeConf themeConf);
typedef TabMenuBuilder = Widget Function(String key);
typedef LabelGetter = Rx<String> Function(String key);

/// [_lastClickTime], help to handle double click
int _lastClickTime =
    DateTime.now().millisecondsSinceEpoch - bind.getDoubleClickTime() - 1000;

class DesktopTab extends StatelessWidget {
  final bool showLogo;
  final bool showTitle;
  final bool showMinimize;
  final bool showMaximize;
  final bool showClose;
  final Widget Function(Widget pageView)? pageViewBuilder;
  // Right click tab menu
  final TabMenuBuilder? tabMenuBuilder;
  final Widget? tail;
  final Future<bool> Function()? onWindowCloseButton;
  final TabBuilder? tabBuilder;
  final LabelGetter? labelGetter;
  final double? maxLabelWidth;
  final Color? selectedTabBackgroundColor;
  final Color? unSelectedTabBackgroundColor;

  final DesktopTabController controller;

  Rx<DesktopTabState> get state => controller.state;
  final isMaximized = false.obs;
  final _scrollDebounce = Debouncer(delay: Duration(milliseconds: 50));

  late final DesktopTabType tabType;
  late final bool isMainWindow;

  DesktopTab({
    Key? key,
    required this.controller,
    this.showLogo = true,
    this.showTitle = false,
    this.showMinimize = true,
    this.showMaximize = true,
    this.showClose = true,
    this.pageViewBuilder,
    this.tabMenuBuilder,
    this.tail,
    this.onWindowCloseButton,
    this.tabBuilder,
    this.labelGetter,
    this.maxLabelWidth,
    this.selectedTabBackgroundColor,
    this.unSelectedTabBackgroundColor,
  }) : super(key: key) {
    tabType = controller.tabType;
    isMainWindow = tabType == DesktopTabType.main ||
        tabType == DesktopTabType.cm ||
        tabType == DesktopTabType.install;
  }

  static RxString labelGetterAlias(String peerId) {
    final opt = 'alias';
    PeerStringOption.init(peerId, opt, () {
      final alias = bind.mainGetPeerOptionSync(id: peerId, key: opt);
      return alias.isEmpty ? peerId : alias;
    });
    return PeerStringOption.find(peerId, opt);
  }

  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Obx(() => Offstage(
          offstage: !stateGlobal.showTabBar.isTrue ||
              (kUseCompatibleUiMode && isHideSingleItem()),
          child: SizedBox(
            height: _kTabBarHeight,
            child: Column(
              children: [
                SizedBox(
                  height: _kTabBarHeight - 1,
                  child: _buildBar(),
                ),
                const Divider(
                  height: 1,
                ),
              ],
            ),
          ))),
      Expanded(
          child: pageViewBuilder != null
              ? pageViewBuilder!(_buildPageView())
              : _buildPageView())
    ]);
  }

  Widget _buildBlock({required Widget child}) {
    if (tabType != DesktopTabType.main) {
      return child;
    }
    var block = false.obs;
    return Obx(() => MouseRegion(
          onEnter: (_) async {
            var access_mode = await bind.mainGetOption(key: 'access-mode');
            var option = option2bool(
                'allow-remote-config-modification',
                await bind.mainGetOption(
                    key: 'allow-remote-config-modification'));
            if (access_mode == 'view' || (access_mode.isEmpty && !option)) {
              var time0 = DateTime.now().millisecondsSinceEpoch;
              await bind.mainCheckMouseTime();
              Timer(const Duration(milliseconds: 120), () async {
                var d = time0 - await bind.mainGetMouseTime();
                if (d < 120) {
                  block.value = true;
                }
              });
            }
          },
          onExit: (_) => block.value = false,
          child: Stack(
            children: [
              child,
              Offstage(
                  offstage: !block.value,
                  child: Container(
                    color: Colors.black.withOpacity(0.5),
                  )),
            ],
          ),
        ));
  }

  List<Widget> _tabWidgets = [];
  Widget _buildPageView() {
    return _buildBlock(
        child: Obx(() => PageView(
            controller: state.value.pageController,
            physics: NeverScrollableScrollPhysics(),
            children: () {
              /// to-do refactor, separate connection state and UI state for remote session.
              /// [workaround] PageView children need an immutable list, after it has been passed into PageView
              final tabLen = state.value.tabs.length;
              if (tabLen == _tabWidgets.length) {
                return _tabWidgets;
              } else if (_tabWidgets.isNotEmpty &&
                  tabLen == _tabWidgets.length + 1) {
                /// On add. Use the previous list(pointer) to prevent item's state init twice.
                /// *[_tabWidgets.isNotEmpty] means TabsWindow(remote_tab_page or file_manager_tab_page) opened before, but was hidden. In this case, we have to reload, otherwise the child can't be built.
                _tabWidgets.add(state.value.tabs.last.page);
                return _tabWidgets;
              } else {
                /// On remove or change. Use new list(pointer) to reload list children so that items loading order is normal.
                /// the Widgets in list must enable [AutomaticKeepAliveClientMixin]
                final newList = state.value.tabs.map((v) => v.page).toList();
                _tabWidgets = newList;
                return newList;
              }
            }())));
  }

  /// Check whether to show ListView
  ///
  /// Conditions:
  /// - hide single item when only has one item (home) on [DesktopTabPage].
  bool isHideSingleItem() {
    return state.value.tabs.length == 1 &&
        (controller.tabType == DesktopTabType.main ||
            controller.tabType == DesktopTabType.install);
  }

  Widget _buildBar() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Expanded(
            child: GestureDetector(
                // custom double tap handler
                onTap: showMaximize
                    ? () {
                        final current = DateTime.now().millisecondsSinceEpoch;
                        final elapsed = current - _lastClickTime;
                        _lastClickTime = current;
                        if (elapsed < bind.getDoubleClickTime()) {
                          // onDoubleTap
                          toggleMaximize(isMainWindow)
                              .then((value) => isMaximized.value = value);
                        }
                      }
                    : null,
                onPanStart: (_) => startDragging(isMainWindow),
                child: Row(
                  children: [
                    Offstage(
                        offstage: !Platform.isMacOS,
                        child: const SizedBox(
                          width: 78,
                        )),
                    Offstage(
                      offstage: kUseCompatibleUiMode || Platform.isMacOS,
                      child: Row(children: [
                        Offstage(
                            offstage: !showLogo,
                            child: SvgPicture.asset(
                              'assets/logo.svg',
                              width: 16,
                              height: 16,
                            )),
                        Offstage(
                            offstage: !showTitle,
                            child: const Text(
                              "RustDesk",
                              style: TextStyle(fontSize: 13),
                            ).marginOnly(left: 2))
                      ]).marginOnly(
                        left: 5,
                        right: 10,
                      ),
                    ),
                    Expanded(
                        child: Listener(
                            // handle mouse wheel
                            onPointerSignal: (e) {
                              if (e is PointerScrollEvent) {
                                final sc =
                                    controller.state.value.scrollController;
                                if (!sc.canScroll) return;
                                _scrollDebounce.call(() {
                                  sc.animateTo(sc.offset + e.scrollDelta.dy,
                                      duration: Duration(milliseconds: 200),
                                      curve: Curves.ease);
                                });
                              }
                            },
                            child: _ListView(
                                controller: controller,
                                tabBuilder: tabBuilder,
                                tabMenuBuilder: tabMenuBuilder,
                                labelGetter: labelGetter,
                                maxLabelWidth: maxLabelWidth,
                                selectedTabBackgroundColor:
                                    selectedTabBackgroundColor,
                                unSelectedTabBackgroundColor:
                                    unSelectedTabBackgroundColor))),
                  ],
                ))),
        // hide simulated action buttons when we in compatible ui mode, because of reusing system title bar.
        WindowActionPanel(
          isMainWindow: isMainWindow,
          tabType: tabType,
          state: state,
          tail: tail,
          isMaximized: isMaximized,
          showMinimize: showMinimize,
          showMaximize: showMaximize,
          showClose: showClose,
          onClose: onWindowCloseButton,
        )
      ],
    );
  }
}

class WindowActionPanel extends StatefulWidget {
  final bool isMainWindow;
  final DesktopTabType tabType;
  final Rx<DesktopTabState> state;
  final RxBool isMaximized;

  final bool showMinimize;
  final bool showMaximize;
  final bool showClose;
  final Widget? tail;
  final Future<bool> Function()? onClose;

  const WindowActionPanel(
      {Key? key,
      required this.isMainWindow,
      required this.tabType,
      required this.state,
      required this.isMaximized,
      this.tail,
      this.showMinimize = true,
      this.showMaximize = true,
      this.showClose = true,
      this.onClose})
      : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return WindowActionPanelState();
  }
}

class WindowActionPanelState extends State<WindowActionPanel>
    with MultiWindowListener, WindowListener {
  @override
  void initState() {
    super.initState();
    DesktopMultiWindow.addListener(this);
    windowManager.addListener(this);

    Future.delayed(Duration(milliseconds: 500), () {
      if (widget.isMainWindow) {
        windowManager.isMaximized().then((maximized) {
          if (widget.isMaximized.value != maximized) {
            WidgetsBinding.instance.addPostFrameCallback(
                (_) => setState(() => widget.isMaximized.value = maximized));
          }
        });
      } else {
        final wc = WindowController.fromWindowId(kWindowId!);
        wc.isMaximized().then((maximized) {
          debugPrint("isMaximized $maximized");
          if (widget.isMaximized.value != maximized) {
            WidgetsBinding.instance.addPostFrameCallback(
                (_) => setState(() => widget.isMaximized.value = maximized));
          }
        });
      }
    });
  }

  @override
  void dispose() {
    DesktopMultiWindow.removeListener(this);
    windowManager.removeListener(this);
    super.dispose();
  }

  void _setMaximize(bool maximize) {
    stateGlobal.setMaximize(maximize);
    setState(() {});
  }

  @override
  void onWindowMaximize() {
    // catch maximize from system
    if (!widget.isMaximized.value) {
      widget.isMaximized.value = true;
    }
    _setMaximize(true);
    super.onWindowMaximize();
  }

  @override
  void onWindowUnmaximize() {
    // catch unmaximize from system
    if (widget.isMaximized.value) {
      widget.isMaximized.value = false;
    }
    _setMaximize(false);
    super.onWindowUnmaximize();
  }

  @override
  void onWindowClose() async {
    // hide window on close
    if (widget.isMainWindow) {
      if (rustDeskWinManager.getActiveWindows().contains(kMainWindowId)) {
        await rustDeskWinManager.unregisterActiveWindow(kMainWindowId);
      }
      // macOS specific workaround, the window is not hiding when in fullscreen.
      if (Platform.isMacOS && await windowManager.isFullScreen()) {
        await windowManager.setFullScreen(false);
        await Future.delayed(Duration(seconds: 1));
      }
      await windowManager.hide();
    } else {
      // it's safe to hide the subwindow
      final controller = WindowController.fromWindowId(kWindowId!);
      if (Platform.isMacOS && await controller.isFullScreen()) {
        await controller.setFullscreen(false);
        await Future.delayed(Duration(seconds: 1));
      }
      await controller.hide();
      await Future.wait([
        rustDeskWinManager
            .call(WindowType.Main, kWindowEventHide, {"id": kWindowId!}),
        widget.onClose?.call() ?? Future.microtask(() => null)
      ]);
    }
    super.onWindowClose();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Offstage(offstage: widget.tail == null, child: widget.tail),
        Offstage(
          offstage: kUseCompatibleUiMode,
          child: Row(
            children: [
              Offstage(
                  offstage: !widget.showMinimize || Platform.isMacOS,
                  child: ActionIcon(
                    message: 'Minimize',
                    icon: IconFont.min,
                    onTap: () {
                      if (widget.isMainWindow) {
                        windowManager.minimize();
                      } else {
                        WindowController.fromWindowId(kWindowId!).minimize();
                      }
                    },
                    isClose: false,
                  )),
              Offstage(
                  offstage: !widget.showMaximize || Platform.isMacOS,
                  child: Obx(() => ActionIcon(
                        message:
                            widget.isMaximized.value ? 'Restore' : 'Maximize',
                        icon: widget.isMaximized.value
                            ? IconFont.restore
                            : IconFont.max,
                        onTap: _toggleMaximize,
                        isClose: false,
                      ))),
              Offstage(
                  offstage: !widget.showClose || Platform.isMacOS,
                  child: ActionIcon(
                    message: 'Close',
                    icon: IconFont.close,
                    onTap: () async {
                      final res = await widget.onClose?.call() ?? true;
                      if (res) {
                        // hide for all window
                        // note: the main window can be restored by tray icon
                        Future.delayed(Duration.zero, () async {
                          if (widget.isMainWindow) {
                            await windowManager.close();
                          } else {
                            await WindowController.fromWindowId(kWindowId!)
                                .close();
                          }
                        });
                      }
                    },
                    isClose: true,
                  ))
            ],
          ),
        ),
      ],
    );
  }

  void _toggleMaximize() {
    toggleMaximize(widget.isMainWindow).then((maximize) {
      if (widget.isMaximized.value != maximize) {
        // update state for sub window, wc.unmaximize/maximize() will not invoke onWindowMaximize/Unmaximize
        widget.isMaximized.value = maximize;
      }
    });
  }
}

void startDragging(bool isMainWindow) {
  if (isMainWindow) {
    windowManager.startDragging();
  } else {
    WindowController.fromWindowId(kWindowId!).startDragging();
  }
}

/// return true -> window will be maximize
/// return false -> window will be unmaximize
Future<bool> toggleMaximize(bool isMainWindow) async {
  if (isMainWindow) {
    if (await windowManager.isMaximized()) {
      windowManager.unmaximize();
      return false;
    } else {
      windowManager.maximize();
      return true;
    }
  } else {
    final wc = WindowController.fromWindowId(kWindowId!);
    if (await wc.isMaximized()) {
      wc.unmaximize();
      return false;
    } else {
      wc.maximize();
      return true;
    }
  }
}

Future<bool> closeConfirmDialog() async {
  var confirm = true;
  final res = await gFFI.dialogManager.show<bool>((setState, close) {
    submit() {
      final opt = "enable-confirm-closing-tabs";
      String value = bool2option(opt, confirm);
      bind.mainSetOption(key: opt, value: value);
      close(true);
    }

    return CustomAlertDialog(
      title: Row(children: [
        const Icon(Icons.warning_amber_sharp,
            color: Colors.redAccent, size: 28),
        const SizedBox(width: 10),
        Text(translate("Warning")),
      ]),
      content: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("Disconnect all devices?")),
            CheckboxListTile(
              contentPadding: const EdgeInsets.all(0),
              dense: true,
              controlAffinity: ListTileControlAffinity.leading,
              title: Text(
                translate("Confirm before closing multiple tabs"),
              ),
              value: confirm,
              onChanged: (v) {
                if (v == null) return;
                setState(() => confirm = v);
              },
            )
          ]),
      // confirm checkbox
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
  return res == true;
}

class _ListView extends StatelessWidget {
  final DesktopTabController controller;

  final TabBuilder? tabBuilder;
  final TabMenuBuilder? tabMenuBuilder;
  final LabelGetter? labelGetter;
  final double? maxLabelWidth;
  final Color? selectedTabBackgroundColor;
  final Color? unSelectedTabBackgroundColor;

  Rx<DesktopTabState> get state => controller.state;

  const _ListView({
    required this.controller,
    this.tabBuilder,
    this.tabMenuBuilder,
    this.labelGetter,
    this.maxLabelWidth,
    this.selectedTabBackgroundColor,
    this.unSelectedTabBackgroundColor,
  });

  /// Check whether to show ListView
  ///
  /// Conditions:
  /// - hide single item when only has one item (home) on [DesktopTabPage].
  bool isHideSingleItem() {
    return state.value.tabs.length == 1 &&
            controller.tabType == DesktopTabType.main ||
        controller.tabType == DesktopTabType.install;
  }

  @override
  Widget build(BuildContext context) {
    return Obx(() => ListView(
        controller: state.value.scrollController,
        scrollDirection: Axis.horizontal,
        shrinkWrap: true,
        physics: const BouncingScrollPhysics(),
        children: isHideSingleItem()
            ? List.empty()
            : state.value.tabs.asMap().entries.map((e) {
                final index = e.key;
                final tab = e.value;
                return _Tab(
                  key: ValueKey(tab.key),
                  index: index,
                  tabInfoKey: tab.key,
                  label: labelGetter == null
                      ? Rx<String>(tab.label)
                      : labelGetter!(tab.label),
                  selectedIcon: tab.selectedIcon,
                  unselectedIcon: tab.unselectedIcon,
                  closable: tab.closable,
                  selected: state.value.selected,
                  onClose: () {
                    if (tab.onTabCloseButton != null) {
                      tab.onTabCloseButton!();
                    } else {
                      controller.remove(index);
                    }
                  },
                  onTap: () {
                    controller.jumpTo(index);
                    tab.onTap?.call();
                  },
                  tabBuilder: tabBuilder,
                  tabMenuBuilder: tabMenuBuilder,
                  maxLabelWidth: maxLabelWidth,
                  selectedTabBackgroundColor: selectedTabBackgroundColor ??
                      MyTheme.tabbar(context).selectedTabBackgroundColor,
                  unSelectedTabBackgroundColor: unSelectedTabBackgroundColor,
                );
              }).toList()));
  }
}

class _Tab extends StatefulWidget {
  final int index;
  final String tabInfoKey;
  final Rx<String> label;
  final IconData? selectedIcon;
  final IconData? unselectedIcon;
  final bool closable;
  final int selected;
  final Function() onClose;
  final Function() onTap;
  final TabBuilder? tabBuilder;
  final TabMenuBuilder? tabMenuBuilder;
  final double? maxLabelWidth;
  final Color? selectedTabBackgroundColor;
  final Color? unSelectedTabBackgroundColor;

  const _Tab({
    Key? key,
    required this.index,
    required this.tabInfoKey,
    required this.label,
    this.selectedIcon,
    this.unselectedIcon,
    this.tabBuilder,
    this.tabMenuBuilder,
    required this.closable,
    required this.selected,
    required this.onClose,
    required this.onTap,
    this.maxLabelWidth,
    this.selectedTabBackgroundColor,
    this.unSelectedTabBackgroundColor,
  }) : super(key: key);

  @override
  State<_Tab> createState() => _TabState();
}

class _TabState extends State<_Tab> with RestorationMixin {
  final RestorableBool restoreHover = RestorableBool(false);

  Widget _buildTabContent() {
    bool showIcon =
        widget.selectedIcon != null && widget.unselectedIcon != null;
    bool isSelected = widget.index == widget.selected;

    final icon = Offstage(
        offstage: !showIcon,
        child: Icon(
          isSelected ? widget.selectedIcon : widget.unselectedIcon,
          size: _kIconSize,
          color: isSelected
              ? MyTheme.tabbar(context).selectedTabIconColor
              : MyTheme.tabbar(context).unSelectedTabIconColor,
        ).paddingOnly(right: 5));
    final labelWidget = Obx(() {
      return ConstrainedBox(
          constraints: BoxConstraints(maxWidth: widget.maxLabelWidth ?? 200),
          child: Text(
            widget.label.value,
            textAlign: TextAlign.center,
            style: TextStyle(
                color: isSelected
                    ? MyTheme.tabbar(context).selectedTextColor
                    : MyTheme.tabbar(context).unSelectedTextColor),
            overflow: TextOverflow.ellipsis,
          ));
    });

    Widget getWidgetWithBuilder() {
      if (widget.tabBuilder == null) {
        return Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            icon,
            labelWidget,
          ],
        );
      } else {
        return widget.tabBuilder!(
          widget.tabInfoKey,
          icon,
          labelWidget,
          TabThemeConf(iconSize: _kIconSize),
        );
      }
    }

    return Listener(
      onPointerDown: (e) {
        if (e.kind != ui.PointerDeviceKind.mouse) {
          return;
        }
        if (e.buttons == 2) {
          if (widget.tabMenuBuilder != null) {
            showRightMenu(
              (cacel) {
                return widget.tabMenuBuilder!(widget.tabInfoKey);
              },
              target: e.position,
            );
          }
        }
      },
      child: getWidgetWithBuilder(),
    );
  }

  @override
  Widget build(BuildContext context) {
    bool isSelected = widget.index == widget.selected;
    bool showDivider =
        widget.index != widget.selected - 1 && widget.index != widget.selected;
    RxBool hover = restoreHover.value.obs;
    return Ink(
      child: InkWell(
        onHover: (value) {
          hover.value = value;
          restoreHover.value = value;
        },
        onTap: () => widget.onTap(),
        child: Container(
          color: isSelected
              ? widget.selectedTabBackgroundColor
              : widget.unSelectedTabBackgroundColor,
          child: Row(
            children: [
              SizedBox(
                  height: _kTabBarHeight,
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        _buildTabContent(),
                        Obx((() => _CloseButton(
                              visible: hover.value && widget.closable,
                              tabSelected: isSelected,
                              onClose: () => widget.onClose(),
                            )))
                      ])).paddingOnly(left: 10, right: 5),
              Offstage(
                offstage: !showDivider,
                child: VerticalDivider(
                  width: 1,
                  indent: _kDividerIndent,
                  endIndent: _kDividerIndent,
                  color: MyTheme.tabbar(context).dividerColor,
                ),
              )
            ],
          ),
        ),
      ),
    );
  }

  @override
  String? get restorationId => "_Tab${widget.label.value}";

  @override
  void restoreState(RestorationBucket? oldBucket, bool initialRestore) {
    registerForRestoration(restoreHover, 'restoreHover');
  }
}

class _CloseButton extends StatelessWidget {
  final bool visible;
  final bool tabSelected;
  final Function onClose;

  const _CloseButton({
    Key? key,
    required this.visible,
    required this.tabSelected,
    required this.onClose,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return SizedBox(
        width: _kIconSize,
        child: Offstage(
          offstage: !visible,
          child: InkWell(
            hoverColor: MyTheme.tabbar(context).closeHoverColor,
            customBorder: const CircleBorder(),
            onTap: () => onClose(),
            child: Icon(
              Icons.close,
              size: _kIconSize,
              color: tabSelected
                  ? MyTheme.tabbar(context).selectedIconColor
                  : MyTheme.tabbar(context).unSelectedIconColor,
            ),
          ),
        )).paddingOnly(left: 10);
  }
}

class ActionIcon extends StatefulWidget {
  final String? message;
  final IconData icon;
  final Function() onTap;
  final bool isClose;
  final double iconSize;
  final double boxSize;

  const ActionIcon(
      {Key? key,
      this.message,
      required this.icon,
      required this.onTap,
      this.isClose = false,
      this.iconSize = _kActionIconSize,
      this.boxSize = _kTabBarHeight - 1})
      : super(key: key);

  @override
  State<ActionIcon> createState() => _ActionIconState();
}

class _ActionIconState extends State<ActionIcon> {
  var hover = false.obs;

  @override
  void initState() {
    super.initState();
    hover.value = false;
  }

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: widget.message != null ? translate(widget.message!) : "",
      waitDuration: const Duration(seconds: 1),
      child: Obx(
        () => InkWell(
          hoverColor: widget.isClose
              ? const Color.fromARGB(255, 196, 43, 28)
              : MyTheme.tabbar(context).hoverColor,
          onHover: (value) => hover.value = value,
          onTap: widget.onTap,
          child: SizedBox(
            height: widget.boxSize,
            width: widget.boxSize,
            child: Icon(
              widget.icon,
              color: hover.value && widget.isClose
                  ? Colors.white
                  : MyTheme.tabbar(context).unSelectedIconColor,
              size: widget.iconSize,
            ),
          ),
        ),
      ),
    );
  }
}

class AddButton extends StatelessWidget {
  const AddButton({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ActionIcon(
        message: 'New Connection',
        icon: IconFont.add,
        onTap: () => rustDeskWinManager.call(
            WindowType.Main, kWindowMainWindowOnTop, ""),
        isClose: false);
  }
}

class TabbarTheme extends ThemeExtension<TabbarTheme> {
  final Color? selectedTabIconColor;
  final Color? unSelectedTabIconColor;
  final Color? selectedTextColor;
  final Color? unSelectedTextColor;
  final Color? selectedIconColor;
  final Color? unSelectedIconColor;
  final Color? dividerColor;
  final Color? hoverColor;
  final Color? closeHoverColor;
  final Color? selectedTabBackgroundColor;

  const TabbarTheme(
      {required this.selectedTabIconColor,
      required this.unSelectedTabIconColor,
      required this.selectedTextColor,
      required this.unSelectedTextColor,
      required this.selectedIconColor,
      required this.unSelectedIconColor,
      required this.dividerColor,
      required this.hoverColor,
      required this.closeHoverColor,
      required this.selectedTabBackgroundColor});

  static const light = TabbarTheme(
      selectedTabIconColor: MyTheme.accent,
      unSelectedTabIconColor: Color.fromARGB(255, 162, 203, 241),
      selectedTextColor: Colors.black,
      unSelectedTextColor: Color.fromARGB(255, 112, 112, 112),
      selectedIconColor: Color.fromARGB(255, 26, 26, 26),
      unSelectedIconColor: Color.fromARGB(255, 96, 96, 96),
      dividerColor: Color.fromARGB(255, 238, 238, 238),
      hoverColor: Color.fromARGB(51, 158, 158, 158),
      closeHoverColor: Color.fromARGB(255, 224, 224, 224),
      selectedTabBackgroundColor: Color.fromARGB(255, 240, 240, 240));

  static const dark = TabbarTheme(
      selectedTabIconColor: MyTheme.accent,
      unSelectedTabIconColor: Color.fromARGB(255, 30, 65, 98),
      selectedTextColor: Color.fromARGB(255, 255, 255, 255),
      unSelectedTextColor: Color.fromARGB(255, 192, 192, 192),
      selectedIconColor: Color.fromARGB(255, 192, 192, 192),
      unSelectedIconColor: Color.fromARGB(255, 255, 255, 255),
      dividerColor: Color.fromARGB(255, 64, 64, 64),
      hoverColor: Colors.black26,
      closeHoverColor: Colors.black,
      selectedTabBackgroundColor: Colors.black26);

  @override
  ThemeExtension<TabbarTheme> copyWith({
    Color? selectedTabIconColor,
    Color? unSelectedTabIconColor,
    Color? selectedTextColor,
    Color? unSelectedTextColor,
    Color? selectedIconColor,
    Color? unSelectedIconColor,
    Color? dividerColor,
    Color? hoverColor,
    Color? closeHoverColor,
    Color? selectedTabBackgroundColor,
  }) {
    return TabbarTheme(
      selectedTabIconColor: selectedTabIconColor ?? this.selectedTabIconColor,
      unSelectedTabIconColor:
          unSelectedTabIconColor ?? this.unSelectedTabIconColor,
      selectedTextColor: selectedTextColor ?? this.selectedTextColor,
      unSelectedTextColor: unSelectedTextColor ?? this.unSelectedTextColor,
      selectedIconColor: selectedIconColor ?? this.selectedIconColor,
      unSelectedIconColor: unSelectedIconColor ?? this.unSelectedIconColor,
      dividerColor: dividerColor ?? this.dividerColor,
      hoverColor: hoverColor ?? this.hoverColor,
      closeHoverColor: closeHoverColor ?? this.closeHoverColor,
      selectedTabBackgroundColor:
          selectedTabBackgroundColor ?? this.selectedTabBackgroundColor,
    );
  }

  @override
  ThemeExtension<TabbarTheme> lerp(
      ThemeExtension<TabbarTheme>? other, double t) {
    if (other is! TabbarTheme) {
      return this;
    }
    return TabbarTheme(
      selectedTabIconColor:
          Color.lerp(selectedTabIconColor, other.selectedTabIconColor, t),
      unSelectedTabIconColor:
          Color.lerp(unSelectedTabIconColor, other.unSelectedTabIconColor, t),
      selectedTextColor:
          Color.lerp(selectedTextColor, other.selectedTextColor, t),
      unSelectedTextColor:
          Color.lerp(unSelectedTextColor, other.unSelectedTextColor, t),
      selectedIconColor:
          Color.lerp(selectedIconColor, other.selectedIconColor, t),
      unSelectedIconColor:
          Color.lerp(unSelectedIconColor, other.unSelectedIconColor, t),
      dividerColor: Color.lerp(dividerColor, other.dividerColor, t),
      hoverColor: Color.lerp(hoverColor, other.hoverColor, t),
      closeHoverColor: Color.lerp(closeHoverColor, other.closeHoverColor, t),
      selectedTabBackgroundColor: Color.lerp(
          selectedTabBackgroundColor, other.selectedTabBackgroundColor, t),
    );
  }

  static color(BuildContext context) {
    return Theme.of(context).extension<ColorThemeExtension>()!;
  }
}
