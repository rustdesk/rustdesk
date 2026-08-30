import 'package:xterm/src/core/cursor.dart';
import 'package:xterm/src/core/mouse/mode.dart';

abstract class TerminalState {
  int get viewWidth;

  int get viewHeight;

  CursorStyle get cursor;

  bool get reflowEnabled;

  /* Modes */

  bool get insertMode;

  bool get lineFeedMode;

  /* DEC Private modes */

  bool get cursorKeysMode;

  bool get reverseDisplayMode;

  bool get originMode;

  bool get autoWrapMode;

  MouseMode get mouseMode;

  MouseReportMode get mouseReportMode;

  bool get cursorBlinkMode;

  bool get cursorVisibleMode;

  bool get appKeypadMode;

  bool get reportFocusMode;

  bool get altBufferMouseScrollMode;

  bool get bracketedPasteMode;
}
