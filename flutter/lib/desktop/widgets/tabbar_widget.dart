import 'dart:io';
import 'dart:math';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:get/get.dart';
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
  final Widget page;

  TabInfo(
      {required this.key,
      required this.label,
      this.selectedIcon,
      this.unselectedIcon,
      this.closable = true,
      required this.page});
}

class DesktopTabState {
  final List<TabInfo> tabs = [];
  final ScrollPosController scrollController =
      ScrollPosController(itemCount: 0);
  final PageController pageController = PageController();
  int selected = 0;

  DesktopTabState() {
    scrollController.itemCount = tabs.length;
  }
}

class DesktopTabController {
  final state = DesktopTabState().obs;

  /// index, key
  Function(int, String)? onRemove;

  Function(int)? onSelected;

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
    onRemove?.call(index, key);
  }

  void jumpTo(int index) {
    state.update((val) {
      val!.selected = index;
      Future.delayed(Duration.zero, (() {
        if (val.pageController.hasClients) {
          val.pageController.jumpToPage(index);
        }
        if (val.scrollController.hasClients &&
            val.scrollController.canScroll &&
            val.scrollController.itemCount >= index) {
          val.scrollController.scrollToItem(index, center: true, animate: true);
        }
      }));
    });
    onSelected?.call(index);
  }

  void closeBy(String? key) {
    if (!isDesktop) return;
    assert(onRemove != null);
    if (key == null) {
      if (state.value.selected < state.value.tabs.length) {
        remove(state.value.selected);
      }
    } else {
      state.value.tabs.indexWhere((tab) => tab.key == key);
      remove(state.value.selected);
    }
  }

  void clear() {
    state.value.tabs.clear();
    state.refresh();
  }
}

class TabThemeConf {
  double iconSize;
  TarBarTheme theme;
  TabThemeConf({required this.iconSize, required this.theme});
}

typedef TabBuilder = Widget Function(
    String key, Widget icon, Widget label, TabThemeConf themeConf);
typedef LabelGetter = Rx<String> Function(String key);

class DesktopTab extends StatelessWidget {
  final Function(String)? onTabClose;
  final TarBarTheme theme;
  final bool isMainWindow;
  final bool showTabBar;
  final bool showLogo;
  final bool showTitle;
  final bool showMinimize;
  final bool showMaximize;
  final bool showClose;
  final Widget Function(Widget pageView)? pageViewBuilder;
  final Widget? tail;
  final VoidCallback? onClose;
  final TabBuilder? tabBuilder;
  final LabelGetter? labelGetter;

  final DesktopTabController controller;
  Rx<DesktopTabState> get state => controller.state;

  const DesktopTab({
    required this.controller,
    required this.isMainWindow,
    this.theme = const TarBarTheme.light(),
    this.onTabClose,
    this.showTabBar = true,
    this.showLogo = true,
    this.showTitle = true,
    this.showMinimize = true,
    this.showMaximize = true,
    this.showClose = true,
    this.pageViewBuilder,
    this.tail,
    this.onClose,
    this.tabBuilder,
    this.labelGetter,
  });

  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Offstage(
          offstage: !showTabBar,
          child: Container(
            height: _kTabBarHeight,
            child: Column(
              children: [
                Container(
                  height: _kTabBarHeight - 1,
                  child: _buildBar(),
                ),
                Divider(
                  height: 1,
                  thickness: 1,
                ),
              ],
            ),
          )),
      Expanded(
          child: pageViewBuilder != null
              ? pageViewBuilder!(_buildPageView())
              : _buildPageView())
    ]);
  }

  Widget _buildPageView() {
    return Obx(() => PageView(
        controller: state.value.pageController,
        children:
            state.value.tabs.map((tab) => tab.page).toList(growable: false)));
  }

  Widget _buildBar() {
    return Row(
      children: [
        Expanded(
          child: Row(
            children: [
              Offstage(
                  offstage: !Platform.isMacOS,
                  child: const SizedBox(
                    width: 78,
                  )),
              Row(children: [
                Offstage(
                    offstage: !showLogo,
                    child: Image.asset(
                      'assets/logo.ico',
                      width: 20,
                      height: 20,
                    )),
                Offstage(
                    offstage: !showTitle,
                    child: Text(
                      "RustDesk",
                      style: TextStyle(fontSize: 13),
                    ).marginOnly(left: 2))
              ]).marginOnly(
                left: 5,
                right: 10,
              ),
              Expanded(
                child: GestureDetector(
                    onPanStart: (_) {
                      if (isMainWindow) {
                        windowManager.startDragging();
                      } else {
                        WindowController.fromWindowId(windowId!)
                            .startDragging();
                      }
                    },
                    child: _ListView(
                      controller: controller,
                      onTabClose: onTabClose,
                      theme: theme,
                      tabBuilder: tabBuilder,
                      labelGetter: labelGetter,
                    )),
              ),
            ],
          ),
        ),
        Offstage(offstage: tail == null, child: tail),
        WindowActionPanel(
          mainTab: isMainWindow,
          theme: theme,
          showMinimize: showMinimize,
          showMaximize: showMaximize,
          showClose: showClose,
          onClose: onClose,
        )
      ],
    );
  }
}

class WindowActionPanel extends StatelessWidget {
  final bool mainTab;
  final TarBarTheme theme;

  final bool showMinimize;
  final bool showMaximize;
  final bool showClose;
  final VoidCallback? onClose;

  const WindowActionPanel(
      {Key? key,
      required this.mainTab,
      required this.theme,
      this.showMinimize = true,
      this.showMaximize = true,
      this.showClose = true,
      this.onClose})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Offstage(
            offstage: !showMinimize,
            child: ActionIcon(
              message: 'Minimize',
              icon: IconFont.min,
              theme: theme,
              onTap: () {
                if (mainTab) {
                  windowManager.minimize();
                } else {
                  WindowController.fromWindowId(windowId!).minimize();
                }
              },
              is_close: false,
            )),
        // TODO: drag makes window restore
        Offstage(
            offstage: !showMaximize,
            child: FutureBuilder(builder: (context, snapshot) {
              RxBool is_maximized = false.obs;
              if (mainTab) {
                windowManager.isMaximized().then((maximized) {
                  is_maximized.value = maximized;
                });
              } else {
                final wc = WindowController.fromWindowId(windowId!);
                wc.isMaximized().then((maximized) {
                  is_maximized.value = maximized;
                });
              }
              return Obx(
                () => ActionIcon(
                  message: is_maximized.value ? "Restore" : "Maximize",
                  icon: is_maximized.value ? IconFont.restore : IconFont.max,
                  theme: theme,
                  onTap: () {
                    if (mainTab) {
                      if (is_maximized.value) {
                        windowManager.unmaximize();
                      } else {
                        windowManager.maximize();
                      }
                    } else {
                      // TODO: subwindow is maximized but first query result is not maximized.
                      final wc = WindowController.fromWindowId(windowId!);
                      if (is_maximized.value) {
                        wc.unmaximize();
                      } else {
                        wc.maximize();
                      }
                    }
                    is_maximized.value = !is_maximized.value;
                  },
                  is_close: false,
                ),
              );
            })),
        Offstage(
            offstage: !showClose,
            child: ActionIcon(
              message: 'Close',
              icon: IconFont.close,
              theme: theme,
              onTap: () {
                if (mainTab) {
                  windowManager.close();
                } else {
                  // only hide for multi window, not close
                  Future.delayed(Duration.zero, () {
                    WindowController.fromWindowId(windowId!).hide();
                  });
                }
                onClose?.call();
              },
              is_close: true,
            )),
      ],
    );
  }
}

// ignore: must_be_immutable
class _ListView extends StatelessWidget {
  final DesktopTabController controller;
  final Function(String key)? onTabClose;
  final TarBarTheme theme;

  final TabBuilder? tabBuilder;
  final LabelGetter? labelGetter;

  Rx<DesktopTabState> get state => controller.state;

  _ListView(
      {required this.controller,
      required this.onTabClose,
      required this.theme,
      this.tabBuilder,
      this.labelGetter});

  @override
  Widget build(BuildContext context) {
    return Obx(() => ListView(
        controller: state.value.scrollController,
        scrollDirection: Axis.horizontal,
        shrinkWrap: true,
        physics: BouncingScrollPhysics(),
        children: state.value.tabs.asMap().entries.map((e) {
          final index = e.key;
          final tab = e.value;
          return _Tab(
            index: index,
            label: labelGetter == null
                ? Rx<String>(tab.label)
                : labelGetter!(tab.label),
            selectedIcon: tab.selectedIcon,
            unselectedIcon: tab.unselectedIcon,
            closable: tab.closable,
            selected: state.value.selected,
            onClose: () => controller.remove(index),
            onSelected: () => controller.jumpTo(index),
            theme: theme,
            tabBuilder: tabBuilder == null
                ? null
                : (Widget icon, Widget labelWidget, TabThemeConf themeConf) {
                    return tabBuilder!(
                      tab.label,
                      icon,
                      labelWidget,
                      themeConf,
                    );
                  },
          );
        }).toList()));
  }
}

class _Tab extends StatelessWidget {
  late final int index;
  late final Rx<String> label;
  late final IconData? selectedIcon;
  late final IconData? unselectedIcon;
  late final bool closable;
  late final int selected;
  late final Function() onClose;
  late final Function() onSelected;
  final RxBool _hover = false.obs;
  late final TarBarTheme theme;
  final Widget Function(Widget icon, Widget label, TabThemeConf themeConf)?
      tabBuilder;

  _Tab(
      {Key? key,
      required this.index,
      required this.label,
      this.selectedIcon,
      this.unselectedIcon,
      this.tabBuilder,
      required this.closable,
      required this.selected,
      required this.onClose,
      required this.onSelected,
      required this.theme})
      : super(key: key);

  Widget _buildTabContent() {
    bool showIcon = selectedIcon != null && unselectedIcon != null;
    bool isSelected = index == selected;

    final icon = Offstage(
        offstage: !showIcon,
        child: Icon(
          isSelected ? selectedIcon : unselectedIcon,
          size: _kIconSize,
          color: isSelected
              ? theme.selectedtabIconColor
              : theme.unSelectedtabIconColor,
        ).paddingOnly(right: 5));
    final labelWidget = Obx(() {
      return Text(
        translate(label.value),
        textAlign: TextAlign.center,
        style: TextStyle(
            color: isSelected
                ? theme.selectedTextColor
                : theme.unSelectedTextColor),
      );
    });

    if (tabBuilder == null) {
      return Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          icon,
          labelWidget,
        ],
      );
    } else {
      return tabBuilder!(
          icon, labelWidget, TabThemeConf(iconSize: _kIconSize, theme: theme));
    }
  }

  @override
  Widget build(BuildContext context) {
    bool showIcon = selectedIcon != null && unselectedIcon != null;
    bool isSelected = index == selected;
    bool showDivider = index != selected - 1 && index != selected;
    return Ink(
      child: InkWell(
        onHover: (hover) => _hover.value = hover,
        onTap: () => onSelected(),
        child: Row(
          children: [
            Container(
                height: _kTabBarHeight,
                child: Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      _buildTabContent(),
                      Offstage(
                        offstage: !closable,
                        child: Obx((() => _CloseButton(
                              visiable: _hover.value,
                              tabSelected: isSelected,
                              onClose: () => onClose(),
                              theme: theme,
                            ))),
                      )
                    ])).paddingSymmetric(horizontal: 10),
            Offstage(
              offstage: !showDivider,
              child: VerticalDivider(
                width: 1,
                indent: _kDividerIndent,
                endIndent: _kDividerIndent,
                color: theme.dividerColor,
                thickness: 1,
              ),
            )
          ],
        ),
      ),
    );
  }
}

class _CloseButton extends StatelessWidget {
  final bool visiable;
  final bool tabSelected;
  final Function onClose;
  late final TarBarTheme theme;

  _CloseButton({
    Key? key,
    required this.visiable,
    required this.tabSelected,
    required this.onClose,
    required this.theme,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return SizedBox(
        width: _kIconSize,
        child: Offstage(
          offstage: !visiable,
          child: InkWell(
            customBorder: RoundedRectangleBorder(),
            onTap: () => onClose(),
            child: Icon(
              Icons.close,
              size: _kIconSize,
              color: tabSelected
                  ? theme.selectedIconColor
                  : theme.unSelectedIconColor,
            ),
          ),
        )).paddingOnly(left: 5);
  }
}

class ActionIcon extends StatelessWidget {
  final String message;
  final IconData icon;
  final TarBarTheme theme;
  final Function() onTap;
  final bool is_close;
  const ActionIcon({
    Key? key,
    required this.message,
    required this.icon,
    required this.theme,
    required this.onTap,
    required this.is_close,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    RxBool hover = false.obs;
    return Obx(() => Tooltip(
          message: translate(message),
          waitDuration: Duration(seconds: 1),
          child: InkWell(
            hoverColor:
                is_close ? Color.fromARGB(255, 196, 43, 28) : theme.hoverColor,
            onHover: (value) => hover.value = value,
            child: Container(
              height: _kTabBarHeight - 1,
              width: _kTabBarHeight - 1,
              child: Icon(
                icon,
                color: hover.value && is_close
                    ? Colors.white
                    : theme.unSelectedIconColor,
                size: _kActionIconSize,
              ),
            ),
            onTap: onTap,
          ),
        ));
  }
}

class AddButton extends StatelessWidget {
  late final TarBarTheme theme;

  AddButton({
    Key? key,
    required this.theme,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ActionIcon(
        message: 'New Connection',
        icon: IconFont.add,
        theme: theme,
        onTap: () =>
            rustDeskWinManager.call(WindowType.Main, "main_window_on_top", ""),
        is_close: false);
  }
}

class TarBarTheme {
  final Color unSelectedtabIconColor;
  final Color selectedtabIconColor;
  final Color selectedTextColor;
  final Color unSelectedTextColor;
  final Color selectedIconColor;
  final Color unSelectedIconColor;
  final Color dividerColor;
  final Color hoverColor;

  const TarBarTheme.light()
      : unSelectedtabIconColor = const Color.fromARGB(255, 162, 203, 241),
        selectedtabIconColor = MyTheme.accent,
        selectedTextColor = const Color.fromARGB(255, 26, 26, 26),
        unSelectedTextColor = const Color.fromARGB(255, 96, 96, 96),
        selectedIconColor = const Color.fromARGB(255, 26, 26, 26),
        unSelectedIconColor = const Color.fromARGB(255, 96, 96, 96),
        dividerColor = const Color.fromARGB(255, 238, 238, 238),
        hoverColor = const Color.fromARGB(
            51, 158, 158, 158); // Colors.grey; //0xFF9E9E9E

  const TarBarTheme.dark()
      : unSelectedtabIconColor = const Color.fromARGB(255, 30, 65, 98),
        selectedtabIconColor = MyTheme.accent,
        selectedTextColor = const Color.fromARGB(255, 255, 255, 255),
        unSelectedTextColor = const Color.fromARGB(255, 207, 207, 207),
        selectedIconColor = const Color.fromARGB(255, 215, 215, 215),
        unSelectedIconColor = const Color.fromARGB(255, 255, 255, 255),
        dividerColor = const Color.fromARGB(255, 64, 64, 64),
        hoverColor = Colors.black26;
}
