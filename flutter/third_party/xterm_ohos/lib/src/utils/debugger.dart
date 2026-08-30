import 'package:xterm/src/core/escape/handler.dart';
import 'package:xterm/src/core/escape/parser.dart';
import 'package:xterm/src/core/mouse/mode.dart';
import 'package:xterm/src/base/observable.dart';

class TerminalCommand {
  TerminalCommand(
    this.start,
    this.end,
    this.chars,
    this.escapedChars,
    this.explanation,
    this.error,
  );

  final int start;

  final int end;

  final String chars;

  final String escapedChars;

  final List<String> explanation;

  final bool error;
}

class TerminalDebugger with Observable {
  late final _parser = EscapeParser(_handler);

  late final _handler = _TerminalDebuggerHandler(recordCommand);

  final recorded = <int>[];

  final commands = <TerminalCommand>[];

  void write(String chunk) {
    recorded.addAll(chunk.runes);
    _parser.write(chunk);
    notifyListeners();
  }

  void recordCommand(String explanation, {bool error = false}) {
    final start = _parser.tokenBegin;
    final end = _parser.tokenEnd;

    if (commands.isNotEmpty && commands.last.end == end) {
      commands.last.explanation.add(explanation);
    } else {
      final charCodes = recorded.sublist(start, end);
      final chars = String.fromCharCodes(charCodes);
      final escapedChars = _escape(chars);
      commands.add(
        TerminalCommand(start, end, chars, escapedChars, [explanation], error),
      );
    }
  }

  String getRecord(TerminalCommand command) {
    final charCodes = recorded.sublist(0, command.end);
    return String.fromCharCodes(charCodes);
  }

  static String _escape(String chars) {
    final escaped = StringBuffer();
    for (final char in chars.runes) {
      if (char == 0x1b) {
        escaped.write('ESC');
      } else if (char < 32) {
        escaped.write('^0x${char.toRadixString(16)}');
      } else if (char == 127) {
        escaped.write('^?');
      } else {
        escaped.writeCharCode(char);
      }
    }
    return escaped.toString();
  }
}

class _TerminalDebuggerHandler implements EscapeHandler {
  _TerminalDebuggerHandler(this.onCommand);

  final void Function(String explanation, {bool error}) onCommand;

  @override
  void writeChar(int char) {
    onCommand('writeChar(${String.fromCharCode(char)})');
  }

  /* SBC */

  @override
  void bell() {
    onCommand('bell');
  }

  @override
  void backspaceReturn() {
    onCommand('backspaceReturn');
  }

  @override
  void tab() {
    onCommand('tab');
  }

  @override
  void lineFeed() {
    onCommand('lineFeed');
  }

  @override
  void carriageReturn() {
    onCommand('carriageReturn');
  }

  @override
  void shiftOut() {
    onCommand('shiftOut');
  }

  @override
  void shiftIn() {
    onCommand('shiftIn');
  }

  @override
  void unknownSBC(int char) {
    onCommand('unkownSBC(${String.fromCharCode(char)})', error: true);
  }

  /* ANSI sequence */

  @override
  void saveCursor() {
    onCommand('saveCursor');
  }

  @override
  void restoreCursor() {
    onCommand('restoreCursor');
  }

  @override
  void index() {
    onCommand('index');
  }

  @override
  void nextLine() {
    onCommand('nextLine');
  }

  @override
  void setTapStop() {
    onCommand('setTapStop');
  }

  @override
  void reverseIndex() {
    onCommand('reverseIndex');
  }

  @override
  void designateCharset(int charset, int name) {
    onCommand('designateCharset($charset, $name)');
  }

  @override
  void unkownEscape(int char) {
    onCommand('unkownEscape(${String.fromCharCode(char)})', error: true);
  }

  /* CSI */

  @override
  void repeatPreviousCharacter(int count) {
    onCommand('repeatPreviousCharacter($count)');
  }

  @override
  void unknownCSI(int finalByte) {
    onCommand('unkownCSI(${String.fromCharCode(finalByte)})', error: true);
  }

  @override
  void setCursor(int x, int y) {
    onCommand('setCursor($x, $y)');
  }

  @override
  void setCursorX(int x) {
    onCommand('setCursorX($x)');
  }

  @override
  void setCursorY(int y) {
    onCommand('setCursorY($y)');
  }

  @override
  void sendPrimaryDeviceAttributes() {
    onCommand('sendPrimaryDeviceAttributes');
  }

  @override
  void clearTabStopUnderCursor() {
    onCommand('clearTabStopUnderCursor');
  }

  @override
  void clearAllTabStops() {
    onCommand('clearAllTabStops');
  }

  @override
  void moveCursorX(int offset) {
    onCommand('moveCursorX($offset)');
  }

  @override
  void moveCursorY(int n) {
    onCommand('moveCursorY($n)');
  }

  @override
  void sendSecondaryDeviceAttributes() {
    onCommand('sendSecondaryDeviceAttributes');
  }

  @override
  void sendTertiaryDeviceAttributes() {
    onCommand('sendTertiaryDeviceAttributes');
  }

  @override
  void sendOperatingStatus() {
    onCommand('sendOperatingStatus');
  }

  @override
  void sendCursorPosition() {
    onCommand('sendCursorPosition');
  }

  @override
  void setMargins(int i, [int? bottom]) {
    onCommand('setMargins($i, $bottom)');
  }

  @override
  void cursorNextLine(int amount) {
    onCommand('cursorNextLine($amount)');
  }

  @override
  void cursorPrecedingLine(int amount) {
    onCommand('cursorPrecedingLine($amount)');
  }

  @override
  void eraseDisplayBelow() {
    onCommand('eraseDisplayBelow');
  }

  @override
  void eraseDisplayAbove() {
    onCommand('eraseDisplayAbove');
  }

  @override
  void eraseDisplay() {
    onCommand('eraseDisplay');
  }

  @override
  void eraseScrollbackOnly() {
    onCommand('eraseScrollbackOnly');
  }

  @override
  void eraseLineRight() {
    onCommand('eraseLineRight');
  }

  @override
  void eraseLineLeft() {
    onCommand('eraseLineLeft');
  }

  @override
  void eraseLine() {
    onCommand('eraseLine');
  }

  @override
  void insertLines(int amount) {
    onCommand('insertLines($amount)');
  }

  @override
  void deleteLines(int amount) {
    onCommand('deleteLines($amount)');
  }

  @override
  void deleteChars(int amount) {
    onCommand('deleteChars($amount)');
  }

  @override
  void scrollUp(int amount) {
    onCommand('scrollUp($amount)');
  }

  @override
  void scrollDown(int amount) {
    onCommand('scrollDown($amount)');
  }

  @override
  void eraseChars(int amount) {
    onCommand('eraseChars($amount)');
  }

  @override
  void insertBlankChars(int amount) {
    onCommand('insertBlankChars($amount)');
  }

  @override
  void resize(int cols, int rows) {
    onCommand('resize($cols, $rows)');
  }

  @override
  void sendSize() {
    onCommand('sendSize');
  }

  /* Modes */

  @override
  void setInsertMode(bool enabled) {
    onCommand('setInsertMode($enabled)');
  }

  @override
  void setLineFeedMode(bool enabled) {
    onCommand('setLineFeedMode($enabled)');
  }

  @override
  void setUnknownMode(int mode, bool enabled) {
    onCommand('setUnknownMode($mode, $enabled)', error: true);
  }

  /* DEC Private modes */

  @override
  void setCursorKeysMode(bool enabled) {
    onCommand('setCursorKeysMode($enabled)');
  }

  @override
  void setReverseDisplayMode(bool enabled) {
    onCommand('setReverseDisplayMode($enabled)');
  }

  @override
  void setOriginMode(bool enabled) {
    onCommand('setOriginMode($enabled)');
  }

  @override
  void setColumnMode(bool enabled) {
    onCommand('setColumnMode($enabled)');
  }

  @override
  void setAutoWrapMode(bool enabled) {
    onCommand('setAutoWrapMode($enabled)');
  }

  @override
  void setMouseMode(MouseMode mode) {
    onCommand('setMouseMode($mode)');
  }

  @override
  void setCursorBlinkMode(bool enabled) {
    onCommand('setCursorBlinkMode($enabled)');
  }

  @override
  void setCursorVisibleMode(bool enabled) {
    onCommand('setCursorVisibleMode($enabled)');
  }

  @override
  void useAltBuffer() {
    onCommand('useAltBuffer');
  }

  @override
  void useMainBuffer() {
    onCommand('useMainBuffer');
  }

  @override
  void clearAltBuffer() {
    onCommand('clearAltBuffer');
  }

  @override
  void setAppKeypadMode(bool enabled) {
    onCommand('setAppKeypadMode($enabled)');
  }

  @override
  void setReportFocusMode(bool enabled) {
    onCommand('setReportFocusMode($enabled)');
  }

  @override
  void setMouseReportMode(MouseReportMode mode) {
    onCommand('setMouseReportMode($mode)');
  }

  @override
  void setAltBufferMouseScrollMode(bool enabled) {
    onCommand('setAltBufferMouseScrollMode($enabled)');
  }

  @override
  void setBracketedPasteMode(bool enabled) {
    onCommand('setBracketedPasteMode($enabled)');
  }

  @override
  void setUnknownDecMode(int mode, bool enabled) {
    onCommand('setUnknownDecMode($mode, $enabled)', error: true);
  }

  /* Select Graphic Rendition (SGR) */

  @override
  void resetCursorStyle() {
    onCommand('resetCursorStyle');
  }

  @override
  void setCursorBold() {
    onCommand('setCursorBold');
  }

  @override
  void setCursorFaint() {
    onCommand('setCursorFaint');
  }

  @override
  void setCursorItalic() {
    onCommand('setCursorItalic');
  }

  @override
  void setCursorUnderline() {
    onCommand('setCursorUnderline');
  }

  @override
  void setCursorBlink() {
    onCommand('setCursorBlink');
  }

  @override
  void setCursorInverse() {
    onCommand('setCursorInverse');
  }

  @override
  void setCursorInvisible() {
    onCommand('setCursorInvisible');
  }

  @override
  void setCursorStrikethrough() {
    onCommand('setCursorStrikethrough');
  }

  @override
  void unsetCursorBold() {
    onCommand('unsetCursorBold');
  }

  @override
  void unsetCursorFaint() {
    onCommand('unsetCursorFaint');
  }

  @override
  void unsetCursorItalic() {
    onCommand('unsetCursorItalic');
  }

  @override
  void unsetCursorUnderline() {
    onCommand('unsetCursorUnderline');
  }

  @override
  void unsetCursorBlink() {
    onCommand('unsetCursorBlink');
  }

  @override
  void unsetCursorInverse() {
    onCommand('unsetCursorInverse');
  }

  @override
  void unsetCursorInvisible() {
    onCommand('unsetCursorInvisible');
  }

  @override
  void unsetCursorStrikethrough() {
    onCommand('unsetCursorStrikethrough');
  }

  @override
  void setForegroundColor16(int color) {
    onCommand('setForegroundColor16($color)');
  }

  @override
  void setForegroundColor256(int index) {
    onCommand('setForegroundColor256($index)');
  }

  @override
  void setForegroundColorRgb(int r, int g, int b) {
    onCommand('setForegroundColorRgb($r, $g, $b)');
  }

  @override
  void resetForeground() {
    onCommand('resetForeground');
  }

  @override
  void setBackgroundColor16(int color) {
    onCommand('setBackgroundColor16($color)');
  }

  @override
  void setBackgroundColor256(int index) {
    onCommand('setBackgroundColor256($index)');
  }

  @override
  void setBackgroundColorRgb(int r, int g, int b) {
    onCommand('setBackgroundColorRgb($r, $g, $b)');
  }

  @override
  void resetBackground() {
    onCommand('resetBackground');
  }

  @override
  void unsupportedStyle(int param) {
    onCommand('unsupportedStyle($param)', error: true);
  }

  /* OSC */

  @override
  void setTitle(String name) {
    onCommand('setTitle($name)');
  }

  @override
  void setIconName(String name) {
    onCommand('setIconName($name)');
  }

  @override
  void unknownOSC(String code, List<String> args) {
    onCommand('unknownOSC($code, $args)', error: true);
  }
}
