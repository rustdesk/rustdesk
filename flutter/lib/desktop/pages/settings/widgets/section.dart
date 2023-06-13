import 'package:flutter/material.dart';
import 'package:get/get.dart';

import 'package:flutter_hbb/common.dart';

const double _kCardFixedWidth = 540;
const double _kCardLeftMargin = 15;
const double _kTitleFontSize = 20;

Widget SettingsSection(
    {required String title,
    required List<Widget> children,
    List<Widget>? title_suffix}) {
  return Row(
    children: [
      Flexible(
        child: SizedBox(
          width: _kCardFixedWidth,
          child: Card(
            child: Padding(
              padding: EdgeInsets.all(16),
              child: Column(
                children: [
                  Row(
                    children: [
                      Expanded(
                          child: Text(
                        translate(title),
                        textAlign: TextAlign.start,
                        style: const TextStyle(
                          fontSize: _kTitleFontSize,
                        ),
                      )),
                      ...?title_suffix
                    ],
                  ).marginOnly(bottom: 16),
                  ...children,
                ],
              ),
            ),
          ).marginOnly(left: _kCardLeftMargin, top: 15),
        ),
      ),
    ],
  );
}
