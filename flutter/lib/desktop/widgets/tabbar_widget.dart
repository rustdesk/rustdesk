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
      color: _theme.bgColor,
      height: _kTabBarHeight,
      child: Row(
        children: [
          Flexible(
            child: Obx(() => TabBar(
                indicatorColor: _theme.tabindicatorColor,
                indicatorSize: TabBarIndicatorSize.tab,
                indicatorWeight: 1,
                labelPadding:
                    const EdgeInsets.symmetric(vertical: 0, horizontal: 0),
                indicatorPadding: EdgeInsets.zero,
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
    return Obx(
      (() => _Hoverable(
            onHover: (hover) => _hover.value = hover,
            onTapUp: () => onSelected(),
            child: Container(
              width: _kTabFixedWidth,
              decoration: BoxDecoration(
                color: is_selected
                    ? theme.tabSelectedColor
                    : _hover.value
                        ? theme.tabHoverColor
                        : theme.tabUnselectedColor,
              ),
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
                              _CloseButton(
                                tabHovered: _hover.value,
                                tabSelected: is_selected,
                                onClose: () => onClose(),
                                theme: theme,
                              ),
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
          )),
    );
  }
}

class _AddButton extends StatelessWidget {
  final RxBool _hover = false.obs;
  final RxBool _pressed = false.obs;
  late final _Theme theme;

  _AddButton({
    Key? key,
    required this.theme,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _Hoverable(
      onHover: (hover) => _hover.value = hover,
      onPressed: (pressed) => _pressed.value = pressed,
      onTapUp: () =>
          rustDeskWinManager.call(WindowType.Main, "main_window_on_top", ""),
      child: Obx((() => Container(
            height: _kTabBarHeight,
            decoration: ShapeDecoration(
              shape: const CircleBorder(),
              color: _pressed.value
                  ? theme.iconPressedBgColor
                  : _hover.value
                      ? theme.iconHoverBgColor
                      : Colors.transparent,
            ),
            child: Icon(
              Icons.add_sharp,
              color: theme.unSelectedIconColor,
              size: _kAddIconSize,
            ),
          ))),
    );
  }
}

class _CloseButton extends StatelessWidget {
  final bool tabHovered;
  final bool tabSelected;
  final Function onClose;
  final RxBool _hover = false.obs;
  final RxBool _pressed = false.obs;
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
              child: Obx((() => _Hoverable(
                    onHover: (hover) => _hover.value = hover,
                    onPressed: (pressed) => _pressed.value = pressed,
                    onTapUp: () => onClose(),
                    child: Container(
                        color: _pressed.value
                            ? theme.iconPressedBgColor
                            : _hover.value
                                ? theme.iconHoverBgColor
                                : Colors.transparent,
                        child: Icon(
                          Icons.close,
                          size: _kIconSize,
                          color: tabSelected
                              ? theme.selectedIconColor
                              : theme.unSelectedIconColor,
                        )),
                  ))),
            )));
  }
}

class _Hoverable extends StatelessWidget {
  final Widget child;
  final Function(bool hover) onHover;
  final Function(bool pressed)? onPressed;
  final Function()? onTapUp;

  const _Hoverable(
      {Key? key,
      required this.child,
      required this.onHover,
      this.onPressed,
      this.onTapUp})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
        onEnter: (_) => onHover(true),
        onExit: (_) => onHover(false),
        child: onPressed == null && onTapUp == null
            ? child
            : GestureDetector(
                onTapDown: (details) => onPressed?.call(true),
                onTapUp: (details) {
                  onPressed?.call(false);
                  onTapUp?.call();
                },
                child: child,
              ));
  }
}

class _Theme {
  late Color bgColor;
  late Color tabUnselectedColor;
  late Color tabHoverColor;
  late Color tabSelectedColor;
  late Color tabIconColor;
  late Color tabindicatorColor;
  late Color selectedTextColor;
  late Color unSelectedTextColor;
  late Color selectedIconColor;
  late Color unSelectedIconColor;
  late Color iconHoverBgColor;
  late Color iconPressedBgColor;
  late Color dividerColor;

  _Theme.light() {
    bgColor = Color.fromARGB(255, 253, 253, 253);
    tabUnselectedColor = Color.fromARGB(255, 253, 253, 253);
    tabHoverColor = Color.fromARGB(255, 245, 245, 245);
    tabSelectedColor = MyTheme.grayBg;
    tabIconColor = MyTheme.accent50;
    tabindicatorColor = MyTheme.grayBg;
    selectedTextColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedTextColor = Color.fromARGB(255, 96, 96, 96);
    selectedIconColor = Color.fromARGB(255, 26, 26, 26);
    unSelectedIconColor = Color.fromARGB(255, 96, 96, 96);
    iconHoverBgColor = Color.fromARGB(255, 224, 224, 224);
    iconPressedBgColor = Color.fromARGB(255, 215, 215, 215);
    dividerColor = Color.fromARGB(255, 238, 238, 238);
  }

  _Theme.dark() {
    bgColor = Color.fromARGB(255, 50, 50, 50);
    tabUnselectedColor = Color.fromARGB(255, 50, 50, 50);
    tabHoverColor = Color.fromARGB(255, 59, 59, 59);
    tabSelectedColor = MyTheme.canvasColor;
    tabIconColor = Color.fromARGB(255, 84, 197, 248);
    tabindicatorColor = MyTheme.canvasColor;
    selectedTextColor = Color.fromARGB(255, 255, 255, 255);
    unSelectedTextColor = Color.fromARGB(255, 207, 207, 207);
    selectedIconColor = Color.fromARGB(255, 215, 215, 215);
    unSelectedIconColor = Color.fromARGB(255, 255, 255, 255);
    iconHoverBgColor = Color.fromARGB(255, 67, 67, 67);
    iconPressedBgColor = Color.fromARGB(255, 73, 73, 73);
    dividerColor = Color.fromARGB(255, 64, 64, 64);
  }
}
