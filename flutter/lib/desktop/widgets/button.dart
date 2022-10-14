import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';

class Button extends StatefulWidget {
  GestureTapCallback onTap;
  String text;
  double? textSize;
  double? minWidth;
  bool isOutline;
  double? padding;
  Color? textColor;
  double? radius;
  Color? borderColor;

  Button({
    Key? key,
    this.minWidth,
    this.isOutline = false,
    this.textSize,
    this.padding,
    this.textColor,
    this.radius,
    this.borderColor,
    required this.onTap,
    required this.text,
  }) : super(key: key);

  @override
  State<Button> createState() => _ButtonState();
}

class _ButtonState extends State<Button> {
  RxBool hover = false.obs;
  RxBool pressed = false.obs;

  @override
  Widget build(BuildContext context) {
    return Obx(() => InkWell(
          onTapDown: (_) => pressed.value = true,
          onTapUp: (_) => pressed.value = false,
          onTapCancel: () => pressed.value = false,
          onHover: (value) => hover.value = value,
          onTap: widget.onTap,
          child: ConstrainedBox(
              constraints: BoxConstraints(
                minWidth: widget.minWidth ?? 70.0,
              ),
              child: Container(
                padding: EdgeInsets.all(widget.padding ?? 4.5),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: pressed.value
                      ? MyTheme.accent
                      : (widget.isOutline
                          ? Colors.transparent
                          : MyTheme.button),
                  border: Border.all(
                    color: pressed.value
                        ? MyTheme.accent
                        : hover.value
                            ? MyTheme.hoverBorder
                            : (widget.isOutline
                                ? widget.borderColor ?? MyTheme.border
                                : MyTheme.button),
                  ),
                  borderRadius: BorderRadius.circular(widget.radius ?? 5),
                ),
                child: Text(
                  translate(
                    widget.text,
                  ),
                  style: TextStyle(
                      fontSize: widget.textSize ?? 12.0,
                      color: widget.isOutline
                          ? widget.textColor ??
                              Theme.of(context).textTheme.titleLarge?.color
                          : Colors.white),
                ).marginSymmetric(horizontal: 12),
              )),
        ));
  }
}
