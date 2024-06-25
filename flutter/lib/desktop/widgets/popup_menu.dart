import 'dart:core';

import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import './material_mod_popup_menu.dart' as mod_menu;

// https://stackoverflow.com/questions/68318314/flutter-popup-menu-inside-popup-menu
class PopupMenuChildrenItem<T> extends mod_menu.PopupMenuEntry<T> {
  const PopupMenuChildrenItem({
    key,
    this.height = kMinInteractiveDimension,
    this.padding,
    this.enabled,
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
  final RxBool? enabled;
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
  RxBool enabled = true.obs;

  @override
  void initState() {
    super.initState();
    if (widget.enabled != null) {
      enabled.value = widget.enabled!.value;
    }
  }

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
        theme.textTheme.titleMedium!;
    return Obx(() => mod_menu.PopupMenuButton<T>(
          enabled: enabled.value,
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
              constraints: BoxConstraints(
                  minHeight: widget.height, maxHeight: widget.height),
              padding:
                  widget.padding ?? const EdgeInsets.symmetric(horizontal: 16),
              child: widget.child,
            ),
          ),
        ));
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
  final double? boxWidth;
  final Color commonColor;

  const MenuConfig(
      {required this.commonColor,
      this.height = kMinInteractiveDimension,
      this.dividerHeight = 16.0,
      this.boxWidth});
}

typedef DismissCallback = Function();

abstract class MenuEntryBase<T> {
  bool dismissOnClicked;
  DismissCallback? dismissCallback;
  RxBool? enabled;

  MenuEntryBase({
    this.dismissOnClicked = false,
    this.enabled,
    this.dismissCallback,
  });
  List<mod_menu.PopupMenuEntry<T>> build(BuildContext context, MenuConfig conf);

  enabledStyle(BuildContext context) => TextStyle(
      color: Theme.of(context).textTheme.titleLarge?.color,
      fontSize: MenuConfig.fontSize,
      fontWeight: FontWeight.normal);
  disabledStyle() => TextStyle(
      color: Colors.grey,
      fontSize: MenuConfig.fontSize,
      fontWeight: FontWeight.normal);
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

class MenuEntryRadioOption {
  String text;
  String value;
  bool dismissOnClicked;
  RxBool? enabled;
  DismissCallback? dismissCallback;

  MenuEntryRadioOption({
    required this.text,
    required this.value,
    this.dismissOnClicked = false,
    this.enabled,
    this.dismissCallback,
  });
}

typedef RadioOptionsGetter = List<MenuEntryRadioOption> Function();
typedef RadioCurOptionGetter = Future<String> Function();
typedef RadioOptionSetter = Future<void> Function(
    String oldValue, String newValue);

class MenuEntryRadioUtils<T> {}

class MenuEntryRadios<T> extends MenuEntryBase<T> {
  final String text;
  final RadioOptionsGetter optionsGetter;
  final RadioCurOptionGetter curOptionGetter;
  final RadioOptionSetter optionSetter;
  final RxString _curOption = "".obs;
  final EdgeInsets? padding;

  MenuEntryRadios({
    required this.text,
    required this.optionsGetter,
    required this.curOptionGetter,
    required this.optionSetter,
    this.padding,
    dismissOnClicked = false,
    dismissCallback,
    RxBool? enabled,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        ) {
    () async {
      _curOption.value = await curOptionGetter();
    }();
  }

  List<MenuEntryRadioOption> get options => optionsGetter();
  RxString get curOption => _curOption;
  setOption(String option) async {
    await optionSetter(_curOption.value, option);
    if (_curOption.value != option) {
      final opt = await curOptionGetter();
      if (_curOption.value != opt) {
        _curOption.value = opt;
      }
    }
  }

  mod_menu.PopupMenuEntry<T> _buildMenuItem(
      BuildContext context, MenuConfig conf, MenuEntryRadioOption opt) {
    Widget getTextChild() {
      final enabledTextChild = Text(
        opt.text,
        style: enabledStyle(context),
      );
      final disabledTextChild = Text(
        opt.text,
        style: disabledStyle(),
      );
      if (opt.enabled == null) {
        return enabledTextChild;
      } else {
        return Obx(
            () => opt.enabled!.isTrue ? enabledTextChild : disabledTextChild);
      }
    }

    final child = Container(
      padding: padding,
      alignment: AlignmentDirectional.centerStart,
      constraints:
          BoxConstraints(minHeight: conf.height, maxHeight: conf.height),
      child: Row(
        children: [
          getTextChild(),
          Expanded(
              child: Align(
                  alignment: Alignment.centerRight,
                  child: Transform.scale(
                    scale: MenuConfig.iconScale,
                    child: Obx(() => opt.value == curOption.value
                        ? IconButton(
                            padding:
                                const EdgeInsets.fromLTRB(8.0, 0.0, 8.0, 0.0),
                            hoverColor: Colors.transparent,
                            focusColor: Colors.transparent,
                            onPressed: () {},
                            icon: Icon(
                              Icons.check,
                              color: (opt.enabled ?? true.obs).isTrue
                                  ? conf.commonColor
                                  : Colors.grey,
                            ))
                        : const SizedBox.shrink()),
                  ))),
        ],
      ),
    );
    onPressed() {
      if (opt.dismissOnClicked && Navigator.canPop(context)) {
        Navigator.pop(context);
        if (opt.dismissCallback != null) {
          opt.dismissCallback!();
        }
      }
      setOption(opt.value);
    }

    return mod_menu.PopupMenuItem(
      padding: EdgeInsets.zero,
      height: conf.height,
      child: Container(
        width: conf.boxWidth,
        child: opt.enabled == null
            ? TextButton(
                child: child,
                onPressed: onPressed,
              )
            : Obx(() => TextButton(
                  child: child,
                  onPressed: opt.enabled!.isTrue ? onPressed : null,
                )),
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
  final EdgeInsets? padding;

  MenuEntrySubRadios({
    required this.text,
    required this.optionsGetter,
    required this.curOptionGetter,
    required this.optionSetter,
    this.padding,
    dismissOnClicked = false,
    RxBool? enabled,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
        ) {
    () async {
      _curOption.value = await curOptionGetter();
    }();
  }

  List<MenuEntryRadioOption> get options => optionsGetter();
  RxString get curOption => _curOption;
  setOption(String option) async {
    await optionSetter(_curOption.value, option);
    if (_curOption.value != option) {
      final opt = await curOptionGetter();
      if (_curOption.value != opt) {
        _curOption.value = opt;
      }
    }
  }

  mod_menu.PopupMenuEntry<T> _buildSecondMenu(
      BuildContext context, MenuConfig conf, MenuEntryRadioOption opt) {
    return mod_menu.PopupMenuItem(
      padding: EdgeInsets.zero,
      height: conf.height,
      child: Container(
          width: conf.boxWidth,
          child: TextButton(
            child: Container(
              padding: padding,
              alignment: AlignmentDirectional.centerStart,
              constraints: BoxConstraints(
                  minHeight: conf.height, maxHeight: conf.height),
              child: Row(
                children: [
                  Text(
                    opt.text,
                    style: TextStyle(
                        color: Theme.of(context).textTheme.titleLarge?.color,
                        fontSize: MenuConfig.fontSize,
                        fontWeight: FontWeight.normal),
                  ),
                  Expanded(
                      child: Align(
                    alignment: Alignment.centerRight,
                    child: Transform.scale(
                        scale: MenuConfig.iconScale,
                        child: Obx(() => opt.value == curOption.value
                            ? IconButton(
                                padding: EdgeInsets.zero,
                                hoverColor: Colors.transparent,
                                focusColor: Colors.transparent,
                                onPressed: () {},
                                icon: Icon(
                                  Icons.check,
                                  color: conf.commonColor,
                                ))
                            : const SizedBox.shrink())),
                  )),
                ],
              ),
            ),
            onPressed: () {
              if (opt.dismissOnClicked && Navigator.canPop(context)) {
                Navigator.pop(context);
                if (opt.dismissCallback != null) {
                  opt.dismissCallback!();
                }
              }
              setOption(opt.value);
            },
          )),
    );
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      PopupMenuChildrenItem(
        enabled: super.enabled,
        padding: padding,
        height: conf.height,
        itemBuilder: (BuildContext context) =>
            options.map((opt) => _buildSecondMenu(context, conf, opt)).toList(),
        child: Row(children: [
          const SizedBox(width: MenuConfig.midPadding),
          Text(
            text,
            style: TextStyle(
                color: Theme.of(context).textTheme.titleLarge?.color,
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

enum SwitchType {
  sswitch,
  scheckbox,
}

typedef SwitchGetter = Future<bool> Function();
typedef SwitchSetter = Future<void> Function(bool);

abstract class MenuEntrySwitchBase<T> extends MenuEntryBase<T> {
  final SwitchType switchType;
  final String text;
  final EdgeInsets? padding;
  Rx<TextStyle>? textStyle;

  MenuEntrySwitchBase({
    required this.switchType,
    required this.text,
    required dismissOnClicked,
    this.textStyle,
    this.padding,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        );

  bool get isEnabled => enabled?.value ?? true;

  RxBool get curOption;
  Future<void> setOption(bool? option);

  tryPop(BuildContext context) {
    if (dismissOnClicked && Navigator.canPop(context)) {
      Navigator.pop(context);
      super.dismissCallback?.call();
    }
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    textStyle ??= TextStyle(
            color: Theme.of(context).textTheme.titleLarge?.color,
            fontSize: MenuConfig.fontSize,
            fontWeight: FontWeight.normal)
        .obs;
    return [
      mod_menu.PopupMenuItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        child: Container(
            width: conf.boxWidth,
            child: TextButton(
              child: Container(
                  padding: padding,
                  alignment: AlignmentDirectional.centerStart,
                  height: conf.height,
                  child: Row(children: [
                    Obx(() => Text(
                          text,
                          style: textStyle!.value,
                        )),
                    Expanded(
                        child: Align(
                      alignment: Alignment.centerRight,
                      child: Transform.scale(
                          scale: MenuConfig.iconScale,
                          child: Obx(() {
                            if (switchType == SwitchType.sswitch) {
                              return Switch(
                                value: curOption.value,
                                onChanged: isEnabled
                                    ? (v) {
                                        tryPop(context);
                                        setOption(v);
                                      }
                                    : null,
                              );
                            } else {
                              return Checkbox(
                                value: curOption.value,
                                onChanged: isEnabled
                                    ? (v) {
                                        tryPop(context);
                                        setOption(v);
                                      }
                                    : null,
                              );
                            }
                          })),
                    ))
                  ])),
              onPressed: isEnabled
                  ? () {
                      tryPop(context);
                      setOption(!curOption.value);
                    }
                  : null,
            )),
      )
    ];
  }
}

class MenuEntrySwitch<T> extends MenuEntrySwitchBase<T> {
  final SwitchGetter getter;
  final SwitchSetter setter;
  final RxBool _curOption = false.obs;

  MenuEntrySwitch({
    required SwitchType switchType,
    required String text,
    required this.getter,
    required this.setter,
    Rx<TextStyle>? textStyle,
    EdgeInsets? padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          switchType: switchType,
          text: text,
          textStyle: textStyle,
          padding: padding,
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        ) {
    () async {
      _curOption.value = await getter();
    }();
  }

  @override
  RxBool get curOption => _curOption;
  @override
  setOption(bool? option) async {
    if (option != null) {
      await setter(option);
      final opt = await getter();
      if (_curOption.value != opt) {
        _curOption.value = opt;
      }
    }
  }
}

// Compatible with MenuEntrySwitch, it uses value instead of getter
class MenuEntrySwitchSync<T> extends MenuEntrySwitchBase<T> {
  final SwitchSetter setter;
  final RxBool _curOption = false.obs;

  MenuEntrySwitchSync({
    required SwitchType switchType,
    required String text,
    required bool currentValue,
    required this.setter,
    Rx<TextStyle>? textStyle,
    EdgeInsets? padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          switchType: switchType,
          text: text,
          textStyle: textStyle,
          padding: padding,
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        ) {
    _curOption.value = currentValue;
  }

  @override
  RxBool get curOption => _curOption;
  @override
  setOption(bool? option) async {
    if (option != null) {
      await setter(option);
      // Notice: no ensure with getter, best used on menus that are destroyed on click
      if (_curOption.value != option) {
        _curOption.value = option;
      }
    }
  }
}

typedef Switch2Getter = RxBool Function();
typedef Switch2Setter = Future<void> Function(bool);

class MenuEntrySwitch2<T> extends MenuEntrySwitchBase<T> {
  final Switch2Getter getter;
  final SwitchSetter setter;

  MenuEntrySwitch2({
    required SwitchType switchType,
    required String text,
    required this.getter,
    required this.setter,
    Rx<TextStyle>? textStyle,
    EdgeInsets? padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          switchType: switchType,
          text: text,
          textStyle: textStyle,
          padding: padding,
          dismissOnClicked: dismissOnClicked,
          dismissCallback: dismissCallback,
        );

  @override
  RxBool get curOption => getter();
  @override
  setOption(bool? option) async {
    if (option != null) {
      await setter(option);
    }
  }
}

class MenuEntrySubMenu<T> extends MenuEntryBase<T> {
  final String text;
  final List<MenuEntryBase<T>> entries;
  final EdgeInsets? padding;

  MenuEntrySubMenu({
    required this.text,
    required this.entries,
    this.padding,
    RxBool? enabled,
  }) : super(enabled: enabled);

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    super.enabled ??= true.obs;
    return [
      PopupMenuChildrenItem(
        enabled: super.enabled,
        height: conf.height,
        padding: padding,
        position: mod_menu.PopupMenuPosition.overSide,
        itemBuilder: (BuildContext context) => entries
            .map((entry) => entry.build(context, conf))
            .expand((i) => i)
            .toList(),
        child: Row(children: [
          const SizedBox(width: MenuConfig.midPadding),
          Obx(() => Text(
                text,
                style: super.enabled!.value
                    ? enabledStyle(context)
                    : disabledStyle(),
              )),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Obx(() => Icon(
                  Icons.keyboard_arrow_right,
                  color: super.enabled!.value ? conf.commonColor : Colors.grey,
                )),
          ))
        ]),
      )
    ];
  }
}

class MenuEntryButton<T> extends MenuEntryBase<T> {
  final Widget Function(TextStyle? style) childBuilder;
  Function() proc;
  final EdgeInsets? padding;

  MenuEntryButton({
    required this.childBuilder,
    required this.proc,
    this.padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        );

  Widget _buildChild(BuildContext context, MenuConfig conf) {
    super.enabled ??= true.obs;
    return Obx(() => Container(
        width: conf.boxWidth,
        child: TextButton(
          onPressed: super.enabled!.value
              ? () {
                  if (super.dismissOnClicked && Navigator.canPop(context)) {
                    Navigator.pop(context);
                    if (super.dismissCallback != null) {
                      super.dismissCallback!();
                    }
                  }
                  proc();
                }
              : null,
          child: Container(
            padding: padding,
            alignment: AlignmentDirectional.centerStart,
            constraints:
                BoxConstraints(minHeight: conf.height, maxHeight: conf.height),
            child: childBuilder(
                super.enabled!.value ? enabledStyle(context) : disabledStyle()),
          ),
        )));
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      mod_menu.PopupMenuItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        child: _buildChild(context, conf),
      )
    ];
  }
}

class CustomPopupMenuTheme {
  static const Color commonColor = MyTheme.accent;
  // kMinInteractiveDimension
  static const double height = 20.0;
  static const double dividerHeight = 3.0;
}
