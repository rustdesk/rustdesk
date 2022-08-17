import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

const double _kTabBarHeight = kDesktopRemoteTabBarHeight;
const double _kIconSize = 18;
const double _kDividerIndent = 10;
const double _kAddIconSize = _kTabBarHeight - 15;
final tabBarKey = GlobalKey();

void closeTab(String? id) {
  final tabBar = tabBarKey.currentWidget as TabBar?;
  if (tabBar == null) return;
  final tabs = tabBar.tabs as List<_Tab>;
  if (id == null) {
    final current = tabBar.controller?.index;
    if (current == null) return;
    tabs[current].onClose();
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
  late final IconData icon;
  late final bool closable;

  TabInfo({required this.label, required this.icon, this.closable = true});
}

class DesktopTabBar extends StatelessWidget {
  late final Rx<TabController> controller;
  late final RxList<TabInfo> tabs;
  late final Function(String) onTabClose;
  late final Rx<int> selected;
  late final bool dark;
  late final _Theme _theme;
  late final bool mainTab;
  late final Function()? onAddSetting;

  DesktopTabBar({
    Key? key,
    required this.controller,
    required this.tabs,
    required this.onTabClose,
    required this.selected,
    required this.dark,
    required this.mainTab,
    this.onAddSetting,
  })  : _theme = dark ? _Theme.dark() : _Theme.light(),
        super(key: key);

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
                Flexible(
                  child: Obx(() => TabBar(
                      key: tabBarKey,
                      indicatorColor: _theme.indicatorColor,
                      labelPadding: const EdgeInsets.symmetric(
                          vertical: 0, horizontal: 0),
                      isScrollable: true,
                      indicatorPadding: EdgeInsets.zero,
                      physics: BouncingScrollPhysics(),
                      controller: controller.value,
                      tabs: tabs.asMap().entries.map((e) {
                        int index = e.key;
                        String label = e.value.label;

                        return _Tab(
                          index: index,
                          label: label,
                          icon: e.value.icon,
                          closable: e.value.closable,
                          selected: selected.value,
                          onClose: () {
                            onTabClose(label);
                            if (index <= selected.value) {
                              selected.value = max(0, selected.value - 1);
                            }
                            controller.value.animateTo(selected.value,
                                duration: Duration.zero);
                          },
                          onSelected: () {
                            selected.value = index;
                            controller.value
                                .animateTo(index, duration: Duration.zero);
                          },
                          theme: _theme,
                        );
                      }).toList())),
                ),
                Offstage(
                  offstage: mainTab,
                  child: _AddButton(
                    theme: _theme,
                  ).paddingOnly(left: 10),
                )
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
          )
        ],
      ),
    );
  }

  static onClose(
    TickerProvider vsync,
    Rx<TabController> controller,
    RxList<TabInfo> tabs,
    String label,
  ) {
    tabs.removeWhere((tab) => tab.label == label);
    controller.value = TabController(
        length: tabs.length,
        vsync: vsync,
        initialIndex: max(0, tabs.length - 1));
  }

  static onAdd(TickerProvider vsync, Rx<TabController> controller,
      RxList<TabInfo> tabs, Rx<int> selected, TabInfo tab) {
    int index = tabs.indexWhere((e) => e.label == tab.label);
    if (index >= 0) {
      controller.value.animateTo(index, duration: Duration.zero);
      selected.value = index;
    } else {
      tabs.add(tab);
      controller.value = TabController(
          length: tabs.length, vsync: vsync, initialIndex: tabs.length - 1);
      controller.value.animateTo(tabs.length - 1, duration: Duration.zero);
      selected.value = tabs.length - 1;
    }
  }
}

class _Tab extends StatelessWidget {
  late final int index;
  late final String label;
  late final IconData icon;
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
      required this.icon,
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
    return Ink(
      child: InkWell(
        onHover: (hover) => _hover.value = hover,
        onTap: () => onSelected(),
        child: Row(
          children: [
            Tab(
                child: Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Icon(
                        icon,
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
