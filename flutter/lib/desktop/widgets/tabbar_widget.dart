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
const double _kAddIconSize = _kTabBarHeight - 15;
final _tabBarKey = GlobalKey();

void closeTab(String? id) {
  final tabBar = _tabBarKey.currentWidget as _ListView?;
  if (tabBar == null) return;
  final tabs = tabBar.tabs;
  if (id == null) {
    if (tabBar.selected.value < tabs.length) {
      tabs[tabBar.selected.value].onClose();
    }
  } else {
    for (final tab in tabs) {
      if (tab.label == id) {
        tab.onClose();
        break;
      }
    }
  }
}

class TabInfo {
  late final String label;
  late final IconData selectedIcon;
  late final IconData unselectedIcon;
  late final bool closable;

  TabInfo(
      {required this.label,
      required this.selectedIcon,
      required this.unselectedIcon,
      this.closable = true});
}

class DesktopTabBar extends StatelessWidget {
  late final RxList<TabInfo> tabs;
  late final Function(String)? onTabClose;
  late final bool dark;
  late final _Theme _theme;
  late final bool mainTab;
  late final Function()? onAddSetting;
  final ScrollPosController scrollController =
      ScrollPosController(itemCount: 0);
  static final Rx<PageController> controller = PageController().obs;
  static final Rx<int> selected = 0.obs;

  DesktopTabBar({
    Key? key,
    required this.tabs,
    this.onTabClose,
    required this.dark,
    required this.mainTab,
    this.onAddSetting,
  })  : _theme = dark ? _Theme.dark() : _Theme.light(),
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
      child: Row(
        children: [
          Expanded(
            child: Row(
              children: [
                Offstage(
                  offstage: !mainTab,
                  child: Row(children: [
                    Image.asset('assets/logo.ico'),
                    Text("RustDesk").paddingOnly(left: 5),
                  ]).paddingSymmetric(horizontal: 12, vertical: 5),
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
                          key: _tabBarKey,
                          controller: controller,
                          scrollController: scrollController,
                          tabInfos: tabs,
                          selected: selected,
                          onTabClose: onTabClose,
                          theme: _theme)),
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
            child: Tooltip(
              message: translate("Settings"),
              child: InkWell(
                child: Icon(
                  Icons.menu,
                  color: _theme.unSelectedIconColor,
                ),
                onTap: () => onAddSetting?.call(),
              ).paddingOnly(right: 10),
            ),
          ),
          WindowActionPanel(
            mainTab: mainTab,
            color: _theme.unSelectedIconColor,
          )
        ],
      ),
    );
  }

  static onAdd(RxList<TabInfo> tabs, TabInfo tab) {
    int index = tabs.indexWhere((e) => e.label == tab.label);
    if (index >= 0) {
      selected.value = index;
    } else {
      tabs.add(tab);
      selected.value = tabs.length - 1;
      assert(selected.value >= 0);
    }
    controller.value.jumpToPage(selected.value);
  }
}

class WindowActionPanel extends StatelessWidget {
  final bool mainTab;
  final Color color;

  const WindowActionPanel(
      {Key? key, required this.mainTab, required this.color})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Tooltip(
          message: translate("Minimize"),
          child: InkWell(
            child: Icon(
              Icons.minimize,
              color: color,
            ).paddingSymmetric(horizontal: 5),
            onTap: () {
              if (mainTab) {
                windowManager.minimize();
              } else {
                WindowController.fromWindowId(windowId!).minimize();
              }
            },
          ),
        ),
        Tooltip(
          message: translate("Maximize"),
          child: InkWell(
            child: Icon(
              Icons.rectangle_outlined,
              color: color,
              size: 20,
            ).paddingSymmetric(horizontal: 5),
            onTap: () {
              if (mainTab) {
                windowManager.isMaximized().then((maximized) {
                  if (maximized) {
                    windowManager.unmaximize();
                  } else {
                    windowManager.maximize();
                  }
                });
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
            },
          ),
        ),
        Tooltip(
          message: translate("Close"),
          child: InkWell(
            child: Icon(
              Icons.close,
              color: color,
            ).paddingSymmetric(horizontal: 5),
            onTap: () {
              if (mainTab) {
                windowManager.close();
              } else {
                WindowController.fromWindowId(windowId!).close();
              }
            },
          ),
        )
      ],
    );
  }
}

class _ListView extends StatelessWidget {
  late Rx<PageController> controller;
  final ScrollPosController scrollController;
  final RxList<TabInfo> tabInfos;
  final Rx<int> selected;
  final Function(String label)? onTabClose;
  final _Theme _theme;
  late List<_Tab> tabs;

  _ListView({
    Key? key,
    required this.controller,
    required this.scrollController,
    required this.tabInfos,
    required this.selected,
    required this.onTabClose,
    required _Theme theme,
  })  : _theme = theme,
        super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      tabs = tabInfos.asMap().entries.map((e) {
        int index = e.key;
        String label = e.value.label;
        return _Tab(
          index: index,
          label: label,
          selectedIcon: e.value.selectedIcon,
          unselectedIcon: e.value.unselectedIcon,
          closable: e.value.closable,
          selected: selected.value,
          onClose: () {
            tabInfos.removeWhere((tab) => tab.label == label);
            onTabClose?.call(label);
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
  late final IconData selectedIcon;
  late final IconData unselectedIcon;
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
      required this.selectedIcon,
      required this.unselectedIcon,
      required this.closable,
      required this.selected,
      required this.onClose,
      required this.onSelected,
      required this.theme})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    bool is_selected = index == selected;
    bool show_divider = index != selected - 1 && index != selected;
    return Stack(
      children: [
        Ink(
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
                              Icon(
                                is_selected ? selectedIcon : unselectedIcon,
                                size: _kIconSize,
                                color: is_selected
                                    ? theme.selectedtabIconColor
                                    : theme.unSelectedtabIconColor,
                              ).paddingOnly(right: 5),
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
        ),
        Positioned(
            height: 2,
            left: 0,
            right: 0,
            bottom: 0,
            child: Center(
              child: Container(
                  color:
                      is_selected ? theme.indicatorColor : Colors.transparent),
            ))
      ],
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
    return Ink(
      height: _kTabBarHeight,
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: () =>
            rustDeskWinManager.call(WindowType.Main, "main_window_on_top", ""),
        child: Icon(
          Icons.add_sharp,
          size: _kAddIconSize,
          color: theme.unSelectedIconColor,
        ),
      ),
    );
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

class _Theme {
  late Color unSelectedtabIconColor;
  late Color selectedtabIconColor;
  late Color selectedTextColor;
  late Color unSelectedTextColor;
  late Color selectedIconColor;
  late Color unSelectedIconColor;
  late Color dividerColor;
  late Color indicatorColor;

  _Theme.light() {
    unSelectedtabIconColor = Color.fromARGB(255, 162, 203, 241);
    selectedtabIconColor = MyTheme.accent;
    selectedTextColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedTextColor = Color.fromARGB(255, 96, 96, 96);
    selectedIconColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedIconColor = Color.fromARGB(255, 96, 96, 96);
    dividerColor = Color.fromARGB(255, 238, 238, 238);
    indicatorColor = MyTheme.accent;
  }

  _Theme.dark() {
    unSelectedtabIconColor = Color.fromARGB(255, 30, 65, 98);
    selectedtabIconColor = MyTheme.accent;
    selectedTextColor = Color.fromARGB(255, 255, 255, 255);
    unSelectedTextColor = Color.fromARGB(255, 207, 207, 207);
    selectedIconColor = Color.fromARGB(255, 215, 215, 215);
    unSelectedIconColor = Color.fromARGB(255, 255, 255, 255);
    dividerColor = Color.fromARGB(255, 64, 64, 64);
    indicatorColor = MyTheme.accent;
  }
}
