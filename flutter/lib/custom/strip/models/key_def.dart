enum KeyType {
  regular,
  modifier,
  macroOpener,
  layer,
  keyboardToggle,
  stripToggle,
  disconnect,
  chatToggle,
}

class KeyDef {
  final String label;
  final String keyName;
  final KeyType type;
  final double widthFactor;

  const KeyDef({
    required this.label,
    required this.keyName,
    required this.type,
    this.widthFactor = 1.0,
  });
}

class StripRow {
  final List<KeyDef> left;
  final List<KeyDef> right;
  const StripRow({required this.left, required this.right});
}

class StripLayout {
  final List<StripRow> rows;
  const StripLayout({required this.rows});

  StripLayout mirrored() => StripLayout(
        rows: rows
            .map((r) => StripRow(left: r.right, right: r.left))
            .toList(),
      );
}
