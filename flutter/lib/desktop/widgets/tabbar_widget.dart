import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

const Color _bgColor = Color.fromARGB(255, 231, 234, 237);
const Color _tabUnselectedColor = Color.fromARGB(255, 240, 240, 240);
const Color _tabHoverColor = Color.fromARGB(255, 245, 245, 245);
const Color _tabSelectedColor = Color.fromARGB(255, 255, 255, 255);
const Color _tabIconColor = MyTheme.accent50;
const Color _tabindicatorColor = _tabIconColor;
const Color _textColor = Color.fromARGB(255, 108, 111, 145);
const Color _iconColor = Color.fromARGB(255, 102, 106, 109);
const Color _iconHoverColor = Colors.black12;
const Color _iconPressedColor = Colors.black26;
const Color _dividerColor = Colors.black12;

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

  DesktopTabBar(
      {Key? key,
      required this.controller,
      required this.tabs,
      required this.onTabClose,
      required this.tabIcon,
      required this.selected})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      color: _bgColor,
      height: _kTabBarHeight,
      child: Row(
        children: [
          Flexible(
            child: Obx(() => TabBar(
                indicatorColor: _tabindicatorColor,
                indicatorSize: TabBarIndicatorSize.tab,
                indicatorWeight: 4,
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
                            // TODO
                            if (e.key <= selected.value) {
                              selected.value = max(0, selected.value - 1);
                            }
                            controller.value.animateTo(selected.value);
                          },
                          onSelected: () {
                            selected.value = e.key;
                            controller.value.animateTo(e.key);
                          },
                        ))
                    .toList())),
          ),
          Padding(
            padding: EdgeInsets.only(left: 10),
            child: _AddButton(),
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

  _Tab({
    Key? key,
    required this.index,
    required this.text,
    required this.icon,
    required this.selected,
    required this.onClose,
    required this.onSelected,
  }) : super(key: key);

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
                    ? _tabSelectedColor
                    : _hover.value
                        ? _tabHoverColor
                        : _tabUnselectedColor,
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
                                  color: _tabIconColor,
                                ),
                              ),
                              Expanded(
                                child: Text(
                                  text,
                                  style: const TextStyle(color: _textColor),
                                ),
                              ),
                              _CloseButton(
                                tabHovered: _hover.value,
                                onClose: () => onClose(),
                              ),
                            ])),
                  ),
                  show_divider
                      ? VerticalDivider(
                          width: 1,
                          indent: _kDividerIndent,
                          endIndent: _kDividerIndent,
                          color: _dividerColor,
                          thickness: 1,
                        )
                      : Container(),
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

  _AddButton({
    Key? key,
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
                  ? _iconPressedColor
                  : _hover.value
                      ? _iconHoverColor
                      : Colors.transparent,
            ),
            child: const Icon(
              Icons.add_sharp,
              color: _iconColor,
              size: _kAddIconSize,
            ),
          ))),
    );
  }
}

class _CloseButton extends StatelessWidget {
  final bool tabHovered;
  final Function onClose;
  final RxBool _hover = false.obs;
  final RxBool _pressed = false.obs;

  _CloseButton({
    Key? key,
    required this.tabHovered,
    required this.onClose,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 5),
        child: SizedBox(
          width: _kIconSize,
          child: tabHovered
              ? Obx((() => _Hoverable(
                    onHover: (hover) => _hover.value = hover,
                    onPressed: (pressed) => _pressed.value = pressed,
                    onTapUp: () => onClose(),
                    child: Container(
                        color: _pressed.value
                            ? _iconPressedColor
                            : _hover.value
                                ? _iconHoverColor
                                : Colors.transparent,
                        child: const Icon(
                          Icons.close,
                          size: _kIconSize,
                          color: _iconColor,
                        )),
                  )))
              : Container(),
        ));
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
