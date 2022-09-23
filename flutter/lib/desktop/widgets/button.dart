import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';

class Button extends StatefulWidget {
  GestureTapCallback onTap;
  String text;
  double? minWidth;
  bool isOutline;

  Button({
    Key? key,
    this.minWidth,
    this.isOutline = false,
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
                minWidth: widget.minWidth ?? 80.0,
              ),
              child: Container(
                height: 27,
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
                                ? MyTheme.border
                                : MyTheme.button),
                  ),
                  borderRadius: BorderRadius.circular(5),
                ),
                child: Text(
                  translate(
                    widget.text,
                  ),
                  style: TextStyle(
                      fontSize: 12,
                      color: pressed.value || !widget.isOutline
                          ? MyTheme.color(context).bg
                          : MyTheme.color(context).text),
                ).marginSymmetric(horizontal: 12),
              )),
        ));
  }
}
