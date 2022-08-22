import 'dart:math';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
import 'package:scroll_pos/scroll_pos.dart';

const double _kTabBarHeight = kDesktopRemoteTabBarHeight;
const double _kIconSize = 18;
const double _kDividerIndent = 10;
const double _kActionIconSize = 12;

class TabInfo {
  late final String key;
  late final String label;
  late final IconData? selectedIcon;
  late final IconData? unselectedIcon;
  late final bool closable;

  TabInfo(
      {required this.key,
      required this.label,
      this.selectedIcon,
      this.unselectedIcon,
      this.closable = true});
}

class DesktopTabBar extends StatelessWidget {
  late final RxList<TabInfo> tabs;
  late final Function(String)? onTabClose;
  late final bool dark;
  late final _Theme _theme;
  late final bool mainTab;
  late final bool showLogo;
  late final bool showTitle;
  late final bool showMinimize;
  late final bool showMaximize;
  late final bool showClose;
  late final void Function()? onAddSetting;
  late final void Function(int)? onSelected;
  final ScrollPosController scrollController =
      ScrollPosController(itemCount: 0);
  static final Rx<PageController> controller = PageController().obs;
  static final Rx<int> selected = 0.obs;
  static final _tabBarListViewKey = GlobalKey();

  DesktopTabBar(
      {Key? key,
      required this.tabs,
      this.onTabClose,
      required this.dark,
      required this.mainTab,
      this.onAddSetting,
      this.onSelected,
      this.showLogo = true,
      this.showTitle = true,
      this.showMinimize = true,
      this.showMaximize = true,
      this.showClose = true})
      : _theme = dark ? _Theme.dark() : _Theme.light(),
        super(key: key) {
    scrollController.itemCount = tabs.length;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      scrollController.scrollToItem(selected.value,
          center: true, animate: true);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      height: _kTabBarHeight,
      child: Column(
        children: [
          Container(
            height: _kTabBarHeight - 1,
            child: Row(
              children: [
                Expanded(
                  child: Row(
                    children: [
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
                              if (mainTab) {
                                windowManager.startDragging();
                              } else {
                                WindowController.fromWindowId(windowId!)
                                    .startDragging();
                              }
                            },
                            child: _ListView(
                                key: _tabBarListViewKey,
                                controller: controller,
                                scrollController: scrollController,
                                tabInfos: tabs,
                                selected: selected,
                                onTabClose: onTabClose,
                                theme: _theme,
                                onSelected: onSelected)),
                      ),
                      Offstage(
                        offstage: mainTab,
                        child: _AddButton(
                          theme: _theme,
                        ).paddingOnly(left: 10),
                      ),
                    ],
                  ),
                ),
                Offstage(
                  offstage: onAddSetting == null,
                  child: _ActionIcon(
                    message: 'Settings',
                    icon: IconFont.menu,
                    theme: _theme,
                    onTap: () => onAddSetting?.call(),
                    is_close: false,
                  ),
                ),
                WindowActionPanel(
                  mainTab: mainTab,
                  theme: _theme,
                  showMinimize: showMinimize,
                  showMaximize: showMaximize,
                  showClose: showClose,
                )
              ],
            ),
          ),
          Divider(
            height: 1,
            thickness: 1,
          ),
        ],
      ),
    );
  }

  static onAdd(RxList<TabInfo> tabs, TabInfo tab) {
    int index = tabs.indexWhere((e) => e.key == tab.key);
    if (index >= 0) {
      selected.value = index;
    } else {
      tabs.add(tab);
      selected.value = tabs.length - 1;
      assert(selected.value >= 0);
    }
    try {
      controller.value.jumpToPage(selected.value);
    } catch (e) {
      // call before binding controller will throw
      debugPrint("Failed to jumpToPage: $e");
    }
  }

  static remove(RxList<TabInfo> tabs, int index) {
    if (index < 0) return;
    if (index == tabs.length - 1) {
      selected.value = max(0, selected.value - 1);
    } else if (index < tabs.length - 1 && index < selected.value) {
      selected.value = max(0, selected.value - 1);
    }
    tabs.removeAt(index);
    controller.value.jumpToPage(selected.value);
  }

  static void jumpTo(RxList<TabInfo> tabs, int index) {
    if (index < 0 || index >= tabs.length) return;
    selected.value = index;
    controller.value.jumpToPage(selected.value);
  }

  static void close(String? key) {
    final tabBar = _tabBarListViewKey.currentWidget as _ListView?;
    if (tabBar == null) return;
    final tabs = tabBar.tabs;
    if (key == null) {
      if (tabBar.selected.value < tabs.length) {
        tabs[tabBar.selected.value].onClose();
      }
    } else {
      for (final tab in tabs) {
        if (tab.key == key) {
          tab.onClose();
          break;
        }
      }
    }
  }
}

class WindowActionPanel extends StatelessWidget {
  final bool mainTab;
  final _Theme theme;

  final bool showMinimize;
  final bool showMaximize;
  final bool showClose;

  const WindowActionPanel(
      {Key? key,
      required this.mainTab,
      required this.theme,
      this.showMinimize = true,
      this.showMaximize = true,
      this.showClose = true})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Offstage(
            offstage: !showMinimize,
            child: _ActionIcon(
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
                () => _ActionIcon(
                  message: is_maximized.value ? "Restore" : "Maximize",
                  icon: is_maximized.value ? IconFont.restore : IconFont.max,
                  theme: theme,
                  onTap: () {
                    if (mainTab) {
                      if (is_maximized.value) {
                        windowManager.unmaximize();
                      } else {
                        WindowController.fromWindowId(windowId!).minimize();
                      }
                    } else {
                      final wc = WindowController.fromWindowId(windowId!);
                      if (is_maximized.value) {
                        wc.unmaximize();
                      } else {
                        final wc = WindowController.fromWindowId(windowId!);
                        wc.isMaximized().then((maximized) {
                          if (maximized) {
                            wc.unmaximize();
                          } else {
                            wc.maximize();
                          }
                        });
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
            child: _ActionIcon(
              message: 'Close',
              icon: IconFont.close,
              theme: theme,
              onTap: () {
                if (mainTab) {
                  windowManager.close();
                } else {
                  WindowController.fromWindowId(windowId!).close();
                }
              },
              is_close: true,
            )),
      ],
    );
  }
}

// ignore: must_be_immutable
class _ListView extends StatelessWidget {
  final Rx<PageController> controller;
  final ScrollPosController scrollController;
  final RxList<TabInfo> tabInfos;
  final Rx<int> selected;
  final Function(String key)? onTabClose;
  final _Theme _theme;
  late List<_Tab> tabs;
  late final void Function(int)? onSelected;

  _ListView(
      {Key? key,
      required this.controller,
      required this.scrollController,
      required this.tabInfos,
      required this.selected,
      required this.onTabClose,
      required _Theme theme,
      this.onSelected})
      : _theme = theme,
        super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      tabs = tabInfos.asMap().entries.map((e) {
        int index = e.key;
        return _Tab(
          index: index,
          label: e.value.label,
          selectedIcon: e.value.selectedIcon,
          unselectedIcon: e.value.unselectedIcon,
          closable: e.value.closable,
          selected: selected.value,
          onClose: () {
            tabInfos.removeWhere((tab) => tab.key == e.value.key);
            onTabClose?.call(e.value.key);
            if (index <= selected.value) {
              selected.value = max(0, selected.value - 1);
            }
            assert(tabInfos.length == 0 || selected.value < tabInfos.length);
            scrollController.itemCount = tabInfos.length;
            if (tabInfos.length > 0) {
              scrollController.scrollToItem(selected.value,
                  center: true, animate: true);
              controller.value.jumpToPage(selected.value);
            }
          },
          onSelected: () {
            selected.value = index;
            scrollController.scrollToItem(index, center: true, animate: true);
            controller.value.jumpToPage(index);
            onSelected?.call(selected.value);
          },
          theme: _theme,
        );
      }).toList();
      return ListView(
          controller: scrollController,
          scrollDirection: Axis.horizontal,
          shrinkWrap: true,
          physics: BouncingScrollPhysics(),
          children: tabs);
    });
  }
}

class _Tab extends StatelessWidget {
  late final int index;
  late final String label;
  late final IconData? selectedIcon;
  late final IconData? unselectedIcon;
  late final bool closable;
  late final int selected;
  late final Function() onClose;
  late final Function() onSelected;
  final RxBool _hover = false.obs;
  late final _Theme theme;

  _Tab(
      {Key? key,
      required this.index,
      required this.label,
      this.selectedIcon,
      this.unselectedIcon,
      required this.closable,
      required this.selected,
      required this.onClose,
      required this.onSelected,
      required this.theme})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    bool show_icon = selectedIcon != null && unselectedIcon != null;
    bool is_selected = index == selected;
    bool show_divider = index != selected - 1 && index != selected;
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
                      Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Offstage(
                              offstage: !show_icon,
                              child: Icon(
                                is_selected ? selectedIcon : unselectedIcon,
                                size: _kIconSize,
                                color: is_selected
                                    ? theme.selectedtabIconColor
                                    : theme.unSelectedtabIconColor,
                              ).paddingOnly(right: 5)),
                          Text(
                            translate(label),
                            textAlign: TextAlign.center,
                            style: TextStyle(
                                color: is_selected
                                    ? theme.selectedTextColor
                                    : theme.unSelectedTextColor),
                          ),
                        ],
                      ),
                      Offstage(
                        offstage: !closable,
                        child: Obx((() => _CloseButton(
                              visiable: _hover.value,
                              tabSelected: is_selected,
                              onClose: () => onClose(),
                              theme: theme,
                            ))),
                      )
                    ])).paddingSymmetric(horizontal: 10),
            Offstage(
              offstage: !show_divider,
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

class _AddButton extends StatelessWidget {
  late final _Theme theme;

  _AddButton({
    Key? key,
    required this.theme,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _ActionIcon(
        message: 'New Connection',
        icon: IconFont.add,
        theme: theme,
        onTap: () =>
            rustDeskWinManager.call(WindowType.Main, "main_window_on_top", ""),
        is_close: false);
  }
}

class _CloseButton extends StatelessWidget {
  final bool visiable;
  final bool tabSelected;
  final Function onClose;
  late final _Theme theme;

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

class _ActionIcon extends StatelessWidget {
  final String message;
  final IconData icon;
  final _Theme theme;
  final Function() onTap;
  final bool is_close;
  const _ActionIcon({
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
          child: InkWell(
            hoverColor: is_close ? Colors.red : theme.hoverColor,
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

class _Theme {
  late Color unSelectedtabIconColor;
  late Color selectedtabIconColor;
  late Color selectedTextColor;
  late Color unSelectedTextColor;
  late Color selectedIconColor;
  late Color unSelectedIconColor;
  late Color dividerColor;
  late Color hoverColor;

  _Theme.light() {
    unSelectedtabIconColor = Color.fromARGB(255, 162, 203, 241);
    selectedtabIconColor = MyTheme.accent;
    selectedTextColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedTextColor = Color.fromARGB(255, 96, 96, 96);
    selectedIconColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedIconColor = Color.fromARGB(255, 96, 96, 96);
    dividerColor = Color.fromARGB(255, 238, 238, 238);
    hoverColor = Colors.grey.withOpacity(0.2);
  }

  _Theme.dark() {
    unSelectedtabIconColor = Color.fromARGB(255, 30, 65, 98);
    selectedtabIconColor = MyTheme.accent;
    selectedTextColor = Color.fromARGB(255, 255, 255, 255);
    unSelectedTextColor = Color.fromARGB(255, 207, 207, 207);
    selectedIconColor = Color.fromARGB(255, 215, 215, 215);
    unSelectedIconColor = Color.fromARGB(255, 255, 255, 255);
    dividerColor = Color.fromARGB(255, 64, 64, 64);
    hoverColor = Colors.black26;
  }
}
