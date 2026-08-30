import 'package:xterm/src/core/buffer/range.dart';

class CellOffset {
  final int x;

  final int y;

  const CellOffset(this.x, this.y);

  bool isEqual(CellOffset other) {
    return other.x == x && other.y == y;
  }

  bool isBefore(CellOffset other) {
    return y < other.y || (y == other.y && x < other.x);
  }

  bool isAfter(CellOffset other) {
    return y > other.y || (y == other.y && x > other.x);
  }

  bool isBeforeOrSame(CellOffset other) {
    return y < other.y || (y == other.y && x <= other.x);
  }

  bool isAfterOrSame(CellOffset other) {
    return y > other.y || (y == other.y && x >= other.x);
  }

  bool isAtSameRow(CellOffset other) {
    return y == other.y;
  }

  bool isAtSameColumn(CellOffset other) {
    return x == other.x;
  }

  bool isWithin(BufferRange range) {
    return range.contains(this);
  }

  @override
  String toString() => 'CellOffset($x, $y)';

  @override
  int get hashCode => x.hashCode ^ y.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CellOffset &&
          runtimeType == other.runtimeType &&
          x == other.x &&
          y == other.y;
}
