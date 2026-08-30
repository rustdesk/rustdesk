import 'dart:ui';

import 'package:flutter/painting.dart';
import 'package:xterm/src/ui/terminal_text_style.dart';

Size calcCharSize(TerminalStyle style, TextScaler textScaler) {
  const test = 'mmmmmmmmmm';

  final textStyle = style.toTextStyle();
  final builder = ParagraphBuilder(textStyle.getParagraphStyle());
  builder.pushStyle(textStyle.getTextStyle(textScaler: textScaler));
  builder.addText(test);

  final paragraph = builder.build();
  paragraph.layout(ParagraphConstraints(width: double.infinity));

  return Size(
    paragraph.maxIntrinsicWidth / test.length,
    paragraph.height,
  );
}
