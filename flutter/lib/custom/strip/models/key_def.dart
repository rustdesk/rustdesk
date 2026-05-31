enum KeyType {
  regular,
  modifier,
  macroOpener,
  layer,
  keyboardToggle,
  stripToggle,
  disconnect,
  chatToggle,
  displaySwitch,
  zoomFit,
  mouseModeToggle,
  clipboardPaste,
  nextDisplay,
  typeString,
  sessionSwitch,
  fileSend,
  arrowCross,
}

class KeyDef {
  final String label;
  final String keyName;
  final KeyType type;
  final double widthFactor;
  final double? height;
  final String? keyString;
  final bool sendEnter;

  const KeyDef({
    required this.label,
    required this.keyName,
    required this.type,
    this.widthFactor = 1.0,
    this.height,
    this.keyString,
    this.sendEnter = false,
  });

  KeyDef copyWith({double? widthFactor}) => KeyDef(
        label: label,
        keyName: keyName,
        type: type,
        widthFactor: widthFactor ?? this.widthFactor,
        height: height,
        keyString: keyString,
        sendEnter: sendEnter,
      );
}

class StripRow {
  final List<KeyDef> left;
  final List<KeyDef> middle;
  final List<KeyDef> right;
  const StripRow({required this.left, this.middle = const [], required this.right});
}

class StripLayout {
  final List<StripRow> rows;
  const StripLayout({required this.rows});

  StripLayout mirrored() => StripLayout(
        rows: rows
            .map((r) => StripRow(left: r.right, middle: r.middle.reversed.toList(), right: r.left))
            .toList(),
      );
}
