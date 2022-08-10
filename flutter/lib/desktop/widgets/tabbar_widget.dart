import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

const double _kTabBarHeight = kDesktopRemoteTabBarHeight;
const double _kTabFixedWidth = 150;
const double _kIconSize = 18;
const double _kDividerIndent = 10;
const double _kAddIconSize = _kTabBarHeight - 15;

class DesktopTabBar extends StatelessWidget {
  late final Rx<TabController> controller;
  late final List<String> tabs;
  late final Function(String) onTabClose;
  late final IconData tabIcon;
  late final Rx<int> selected;
  late final bool dark;
  late final _Theme _theme;

  DesktopTabBar(
      {Key? key,
      required this.controller,
      required this.tabs,
      required this.onTabClose,
      required this.tabIcon,
      required this.selected,
      required this.dark})
      : _theme = dark ? _Theme.dark() : _Theme.light(),
        super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      height: _kTabBarHeight,
      child: Scaffold(
        backgroundColor: _theme.bgColor,
        body: Row(
          children: [
            Flexible(
              child: Obx(() => TabBar(
                  indicator: BoxDecoration(),
                  indicatorColor: Colors.transparent,
                  labelPadding:
                      const EdgeInsets.symmetric(vertical: 0, horizontal: 0),
                  isScrollable: true,
                  physics: BouncingScrollPhysics(),
                  controller: controller.value,
                  tabs: tabs
                      .asMap()
                      .entries
                      .map((e) => _Tab(
                            index: e.key,
                            text: e.value,
                            icon: tabIcon,
                            selected: selected.value,
                            onClose: () {
                              onTabClose(e.value);
                              if (e.key <= selected.value) {
                                selected.value = max(0, selected.value - 1);
                              }
                              controller.value.animateTo(selected.value,
                                  duration: Duration.zero);
                            },
                            onSelected: () {
                              selected.value = e.key;
                              controller.value
                                  .animateTo(e.key, duration: Duration.zero);
                            },
                            theme: _theme,
                          ))
                      .toList())),
            ),
            Padding(
              padding: EdgeInsets.only(left: 10),
              child: _AddButton(
                theme: _theme,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Tab extends StatelessWidget {
  late final int index;
  late final String text;
  late final IconData icon;
  late final int selected;
  late final Function() onClose;
  late final Function() onSelected;
  final RxBool _hover = false.obs;
  late final _Theme theme;

  _Tab(
      {Key? key,
      required this.index,
      required this.text,
      required this.icon,
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
      width: _kTabFixedWidth,
      color: is_selected ? theme.tabSelectedColor : null,
      child: InkWell(
        onHover: (hover) => _hover.value = hover,
        onTap: () => onSelected(),
        child: Row(
          children: [
            Expanded(
              child: Tab(
                  key: this.key,
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        Padding(
                          padding: EdgeInsets.symmetric(horizontal: 5),
                          child: Icon(
                            icon,
                            size: _kIconSize,
                            color: theme.tabIconColor,
                          ),
                        ),
                        Expanded(
                          child: Text(
                            text,
                            style: TextStyle(
                                color: is_selected
                                    ? theme.selectedTextColor
                                    : theme.unSelectedTextColor),
                          ),
                        ),
                        Obx((() => _CloseButton(
                              tabHovered: _hover.value,
                              tabSelected: is_selected,
                              onClose: () => onClose(),
                              theme: theme,
                            ))),
                      ])),
            ),
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
  final bool tabHovered;
  final bool tabSelected;
  final Function onClose;
  late final _Theme theme;

  _CloseButton({
    Key? key,
    required this.tabHovered,
    required this.tabSelected,
    required this.onClose,
    required this.theme,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 5),
        child: SizedBox(
            width: _kIconSize,
            child: Offstage(
              offstage: !tabHovered,
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
            )));
  }
}

class _Theme {
  late Color bgColor;
  late Color tabSelectedColor;
  late Color tabIconColor;
  late Color selectedTextColor;
  late Color unSelectedTextColor;
  late Color selectedIconColor;
  late Color unSelectedIconColor;
  late Color dividerColor;

  _Theme.light() {
    bgColor = Color.fromARGB(255, 253, 253, 253);
    tabSelectedColor = MyTheme.grayBg;
    tabIconColor = MyTheme.accent50;
    selectedTextColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedTextColor = Color.fromARGB(255, 96, 96, 96);
    selectedIconColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedIconColor = Color.fromARGB(255, 96, 96, 96);
    dividerColor = Color.fromARGB(255, 238, 238, 238);
  }

  _Theme.dark() {
    bgColor = Color.fromARGB(255, 50, 50, 50);
    tabSelectedColor = MyTheme.canvasColor;
    tabIconColor = Color.fromARGB(255, 84, 197, 248);
    selectedTextColor = Color.fromARGB(255, 255, 255, 255);
    unSelectedTextColor = Color.fromARGB(255, 207, 207, 207);
    selectedIconColor = Color.fromARGB(255, 215, 215, 215);
    unSelectedIconColor = Color.fromARGB(255, 255, 255, 255);
    dividerColor = Color.fromARGB(255, 64, 64, 64);
  }
}
