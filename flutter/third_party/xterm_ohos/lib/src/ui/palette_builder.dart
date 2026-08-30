import 'package:flutter/widgets.dart';
import 'package:xterm/src/ui/terminal_theme.dart';
import 'package:xterm/src/utils/lookup_table.dart';

class PaletteBuilder {
  final TerminalTheme theme;

  PaletteBuilder(this.theme);

  List<Color> build() {
    return List<Color>.generate(
      256,
      paletteColor,
      growable: false,
    );
  }

  /// https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit
  Color paletteColor(int colNum) {
    switch (colNum) {
      case 0:
        return theme.black;
      case 1:
        return theme.red;
      case 2:
        return theme.green;
      case 3:
        return theme.yellow;
      case 4:
        return theme.blue;
      case 5:
        return theme.magenta;
      case 6:
        return theme.cyan;
      case 7:
        return theme.white;
      case 8:
        return theme.brightBlack;
      case 9:
        return theme.brightRed;
      case 10:
        return theme.brightGreen;
      case 11:
        return theme.brightYellow;
      case 12:
        return theme.brightBlue;
      case 13:
        return theme.brightMagenta;
      case 14:
        return theme.brightCyan;
      case 15:
        return theme.white;
    }

    if (colNum < 232) {
      var r = 0;
      var g = 0;
      var b = 0;

      final index = colNum - 16;

      for (var i = 0; i < index; i++) {
        if (b == 0) {
          b = 95;
        } else if (b < 255) {
          b += 40;
        } else {
          b = 0;
          if (g == 0) {
            g = 95;
          } else if (g < 255) {
            g += 40;
          } else {
            g = 0;
            if (r == 0) {
              r = 95;
            } else if (r < 255) {
              r += 40;
            } else {
              break;
            }
          }
        }
      }

      return Color.fromARGB(0xFF, r, g, b);
    }

    return Color(_grayscaleColors[colNum.clamp(232, 255)]!);
  }
}

final _grayscaleColors = FastLookupTable({
  232: 0xff080808,
  233: 0xff121212,
  234: 0xff1c1c1c,
  235: 0xff262626,
  236: 0xff303030,
  237: 0xff3a3a3a,
  238: 0xff444444,
  239: 0xff4e4e4e,
  240: 0xff585858,
  241: 0xff626262,
  242: 0xff6c6c6c,
  243: 0xff767676,
  244: 0xff808080,
  245: 0xff8a8a8a,
  246: 0xff949494,
  247: 0xff9e9e9e,
  248: 0xffa8a8a8,
  249: 0xffb2b2b2,
  250: 0xffbcbcbc,
  251: 0xffc6c6c6,
  252: 0xffd0d0d0,
  253: 0xffdadada,
  254: 0xffe4e4e4,
  255: 0xffeeeeee,
});
