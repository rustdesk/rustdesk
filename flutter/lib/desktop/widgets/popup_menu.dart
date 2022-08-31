import 'dart:core';

import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:tuple/tuple.dart';

import './material_mod_popup_menu.dart' as mod_menu;

// https://stackoverflow.com/questions/68318314/flutter-popup-menu-inside-popup-menu
class PopupMenuChildrenItem<T> extends mod_menu.PopupMenuEntry<T> {
  const PopupMenuChildrenItem({
    key,
    this.height = kMinInteractiveDimension,
    this.padding,
    this.enable = true,
    this.textStyle,
    this.onTap,
    this.position = mod_menu.PopupMenuPosition.overSide,
    this.offset = Offset.zero,
    required this.itemBuilder,
    required this.child,
  }) : super(key: key);

  final mod_menu.PopupMenuPosition position;
  final Offset offset;
  final TextStyle? textStyle;
  final EdgeInsets? padding;
  final bool enable;
  final void Function()? onTap;
  final List<mod_menu.PopupMenuEntry<T>> Function(BuildContext) itemBuilder;
  final Widget child;

  @override
  final double height;

  @override
  bool represents(T? value) => false;

  @override
  MyPopupMenuItemState<T, PopupMenuChildrenItem<T>> createState() =>
      MyPopupMenuItemState<T, PopupMenuChildrenItem<T>>();
}

class MyPopupMenuItemState<T, W extends PopupMenuChildrenItem<T>>
    extends State<W> {
  @protected
  void handleTap(T value) {
    widget.onTap?.call();
    Navigator.pop<T>(context, value);
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final PopupMenuThemeData popupMenuTheme = PopupMenuTheme.of(context);
    TextStyle style = widget.textStyle ??
        popupMenuTheme.textStyle ??
        theme.textTheme.subtitle1!;

    return mod_menu.PopupMenuButton<T>(
      enabled: widget.enable,
      position: widget.position,
      offset: widget.offset,
      onSelected: handleTap,
      itemBuilder: widget.itemBuilder,
      padding: EdgeInsets.zero,
      child: AnimatedDefaultTextStyle(
        style: style,
        duration: kThemeChangeDuration,
        child: Container(
          alignment: AlignmentDirectional.centerStart,
          constraints: BoxConstraints(minHeight: widget.height),
          padding: widget.padding ?? const EdgeInsets.symmetric(horizontal: 16),
          child: widget.child,
        ),
      ),
    );
  }
}

class MenuConfig {
  // adapt to the screen height
  static const fontSize = 14.0;
  static const midPadding = 10.0;
  static const iconScale = 0.8;
  static const iconWidth = 12.0;
  static const iconHeight = 12.0;

  final double height;
  final double dividerHeight;
  final Color commonColor;

  const MenuConfig(
      {required this.commonColor,
      this.height = kMinInteractiveDimension,
      this.dividerHeight = 16.0});
}

abstract class MenuEntryBase<T> {
  List<mod_menu.PopupMenuEntry<T>> build(BuildContext context, MenuConfig conf);
}

class MenuEntryDivider<T> extends MenuEntryBase<T> {
  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      mod_menu.PopupMenuDivider(
        height: conf.dividerHeight,
      )
    ];
  }
}

typedef RadioOptionsGetter = List<Tuple2<String, String>> Function();
typedef RadioCurOptionGetter = Future<String> Function();
typedef RadioOptionSetter = Future<void> Function(String);

class MenuEntryRadioUtils<T> {}

class MenuEntryRadios<T> extends MenuEntryBase<T> {
  final String text;
  final RadioOptionsGetter optionsGetter;
  final RadioCurOptionGetter curOptionGetter;
  final RadioOptionSetter optionSetter;
  final RxString _curOption = "".obs;

  MenuEntryRadios(
      {required this.text,
      required this.optionsGetter,
      required this.curOptionGetter,
      required this.optionSetter}) {
    () async {
      _curOption.value = await curOptionGetter();
    }();
  }

  List<Tuple2<String, String>> get options => optionsGetter();
  RxString get curOption => _curOption;
  setOption(String option) async {
    await optionSetter(option);
    final opt = await curOptionGetter();
    if (_curOption.value != opt) {
      _curOption.value = opt;
    }
  }

  mod_menu.PopupMenuEntry<T> _buildMenuItem(
      BuildContext context, MenuConfig conf, Tuple2<String, String> opt) {
    return mod_menu.PopupMenuItem(
      padding: EdgeInsets.zero,
      height: conf.height,
      child: TextButton(
        child: Container(
          alignment: AlignmentDirectional.centerStart,
          constraints: BoxConstraints(minHeight: conf.height),
          child: Row(
            children: [
              Text(
                opt.item1,
                style: const TextStyle(
                    color: Colors.black,
                    fontSize: MenuConfig.fontSize,
                    fontWeight: FontWeight.normal),
              ),
              Expanded(
                  child: Align(
                alignment: Alignment.centerRight,
                child: SizedBox(
                    width: 20.0,
                    height: 20.0,
                    child: Obx(() => opt.item2 == curOption.value
                        ? Icon(
                            Icons.check,
                            color: conf.commonColor,
                          )
                        : const SizedBox.shrink())),
              )),
            ],
          ),
        ),
        onPressed: () {
          if (opt.item2 != curOption.value) {
            setOption(opt.item2);
          }
        },
      ),
    );
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return options.map((opt) => _buildMenuItem(context, conf, opt)).toList();
  }
}

class MenuEntrySubRadios<T> extends MenuEntryBase<T> {
  final String text;
  final RadioOptionsGetter optionsGetter;
  final RadioCurOptionGetter curOptionGetter;
  final RadioOptionSetter optionSetter;
  final RxString _curOption = "".obs;

  MenuEntrySubRadios(
      {required this.text,
      required this.optionsGetter,
      required this.curOptionGetter,
      required this.optionSetter}) {
    () async {
      _curOption.value = await curOptionGetter();
    }();
  }

  List<Tuple2<String, String>> get options => optionsGetter();
  RxString get curOption => _curOption;
  setOption(String option) async {
    await optionSetter(option);
    final opt = await curOptionGetter();
    if (_curOption.value != opt) {
      _curOption.value = opt;
    }
  }

  mod_menu.PopupMenuEntry<T> _buildSecondMenu(
      BuildContext context, MenuConfig conf, Tuple2<String, String> opt) {
    return mod_menu.PopupMenuItem(
      padding: EdgeInsets.zero,
      height: conf.height,
      child: TextButton(
        child: Container(
          alignment: AlignmentDirectional.centerStart,
          constraints: BoxConstraints(minHeight: conf.height),
          child: Row(
            children: [
              Text(
                opt.item1,
                style: const TextStyle(
                    color: Colors.black,
                    fontSize: MenuConfig.fontSize,
                    fontWeight: FontWeight.normal),
              ),
              Expanded(
                  child: Align(
                alignment: Alignment.centerRight,
                child: SizedBox(
                    width: 20.0,
                    height: 20.0,
                    child: Obx(() => opt.item2 == curOption.value
                        ? Icon(
                            Icons.check,
                            color: conf.commonColor,
                          )
                        : const SizedBox.shrink())),
              )),
            ],
          ),
        ),
        onPressed: () {
          if (opt.item2 != curOption.value) {
            setOption(opt.item2);
          }
        },
      ),
    );
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      PopupMenuChildrenItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        itemBuilder: (BuildContext context) =>
            options.map((opt) => _buildSecondMenu(context, conf, opt)).toList(),
        child: Row(children: [
          const SizedBox(width: MenuConfig.midPadding),
          Text(
            text,
            style: const TextStyle(
                color: Colors.black,
                fontSize: MenuConfig.fontSize,
                fontWeight: FontWeight.normal),
          ),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Icon(
              Icons.keyboard_arrow_right,
              color: conf.commonColor,
            ),
          ))
        ]),
      )
    ];
  }
}

typedef SwitchGetter = Future<bool> Function();
typedef SwitchSetter = Future<void> Function(bool);

abstract class MenuEntrySwitchBase<T> extends MenuEntryBase<T> {
  final String text;

  MenuEntrySwitchBase({required this.text});

  RxBool get curOption;
  Future<void> setOption(bool option);

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      mod_menu.PopupMenuItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        child: TextButton(
          child: Container(
              alignment: AlignmentDirectional.centerStart,
              height: conf.height,
              child: Row(children: [
                // const SizedBox(width: MenuConfig.midPadding),
                Text(
                  text,
                  style: const TextStyle(
                      color: Colors.black,
                      fontSize: MenuConfig.fontSize,
                      fontWeight: FontWeight.normal),
                ),
                Expanded(
                    child: Align(
                  alignment: Alignment.centerRight,
                  child: Obx(() => Switch(
                        value: curOption.value,
                        onChanged: (v) => setOption(v),
                      )),
                ))
              ])),
          onPressed: () {
            setOption(!curOption.value);
          },
        ),
      )
    ];
  }
}

class MenuEntrySwitch<T> extends MenuEntrySwitchBase<T> {
  final SwitchGetter getter;
  final SwitchSetter setter;
  final RxBool _curOption = false.obs;

  MenuEntrySwitch(
      {required String text, required this.getter, required this.setter})
      : super(text: text) {
    () async {
      _curOption.value = await getter();
    }();
  }

  @override
  RxBool get curOption => _curOption;
  @override
  setOption(bool option) async {
    await setter(option);
    final opt = await getter();
    if (_curOption.value != opt) {
      _curOption.value = opt;
    }
  }
}

typedef Switch2Getter = RxBool Function();
typedef Switch2Setter = Future<void> Function(bool);

class MenuEntrySwitch2<T> extends MenuEntrySwitchBase<T> {
  final Switch2Getter getter;
  final SwitchSetter setter;

  MenuEntrySwitch2(
      {required String text, required this.getter, required this.setter})
      : super(text: text);

  @override
  RxBool get curOption => getter();
  @override
  setOption(bool option) async {
    await setter(option);
  }
}

class MenuEntrySubMenu<T> extends MenuEntryBase<T> {
  final String text;
  final List<MenuEntryBase<T>> entries;

  MenuEntrySubMenu({
    required this.text,
    required this.entries,
  });

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      PopupMenuChildrenItem(
        height: conf.height,
        padding: EdgeInsets.zero,
        position: mod_menu.PopupMenuPosition.overSide,
        itemBuilder: (BuildContext context) => entries
            .map((entry) => entry.build(context, conf))
            .expand((i) => i)
            .toList(),
        child: Row(children: [
          const SizedBox(width: MenuConfig.midPadding),
          Text(
            text,
            style: const TextStyle(
                color: Colors.black,
                fontSize: MenuConfig.fontSize,
                fontWeight: FontWeight.normal),
          ),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Icon(
              Icons.keyboard_arrow_right,
              color: conf.commonColor,
            ),
          ))
        ]),
      )
    ];
  }
}

class MenuEntryButton<T> extends MenuEntryBase<T> {
  final Widget Function(TextStyle? style) childBuilder;
  Function() proc;

  MenuEntryButton({
    required this.childBuilder,
    required this.proc,
  });

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      mod_menu.PopupMenuItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        child: TextButton(
          child: Container(
              alignment: AlignmentDirectional.centerStart,
              constraints: BoxConstraints(minHeight: conf.height),
              child: childBuilder(
                const TextStyle(
                    color: Colors.black,
                    fontSize: MenuConfig.fontSize,
                    fontWeight: FontWeight.normal),
              )),
          onPressed: () {
            proc();
          },
        ),
      )
    ];
  }
}
