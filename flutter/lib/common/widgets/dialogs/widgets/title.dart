import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';

import 'package:flutter_hbb/common.dart';

/// [icon] accepts IconData ("Icons.done_rounded") or filename ("arrow.svg")
Widget dialogTitle(String text, {dynamic icon, bool centered = false}) {
  double iconSize = 24;
  double iconMarginRight = 8;
  List<Widget> children = [
    Expanded(
      child: Text(translate(text)),
    )
  ];

  if (icon is IconData) {
    children.insert(
        0, Icon(icon, size: iconSize).marginOnly(right: iconMarginRight));
  } else if (icon is String) {
    if (File('assets/$icon').existsSync()) {
      children.insert(
        0,
        SizedBox(
          width: iconSize,
          height: iconSize,
          child: SvgPicture.asset('assets/$icon', fit: BoxFit.contain),
        ).marginOnly(right: iconMarginRight),
      );
    } else {
      debugPrint('dialogTitle: File "assets/$icon" not found');
    }
  }

  return Row(
    // todo centered, row width expanded text
    mainAxisAlignment:
        centered ? MainAxisAlignment.center : MainAxisAlignment.start,
    children: [...children],
  );
}

/// Headline "Delete" with "delete" icon
Widget dialogTitleDelete({String? text}) {
  return dialogTitle(text ?? 'Delete', icon: Icons.delete_outline_rounded);
}
