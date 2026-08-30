import 'package:xterm/src/core/buffer/cell_offset.dart';
import 'package:xterm/src/core/buffer/segment.dart';

abstract class BufferRange {
  final CellOffset begin;

  final CellOffset end;

  const BufferRange(this.begin, this.end);

  BufferRange.collapsed(this.begin) : end = begin;

  bool get isNormalized {
    return begin.isBefore(end) || begin.isEqual(end);
  }

  bool get isCollapsed {
    return begin.isEqual(end);
  }

  BufferRange get normalized;

  /// Convert this range to segments of single lines.
  Iterable<BufferSegment> toSegments();

  /// Returns true if the given[position] is within this range.
  bool contains(CellOffset position);

  /// Returns the smallest range that contains both this range and the given
  /// [range].
  BufferRange merge(BufferRange range);

  /// Returns the smallest range that contains both this range and the given
  /// [position].
  BufferRange extend(CellOffset position);

  @override
  operator ==(Object other) {
    if (identical(this, other)) {
      return true;
    }

    if (other is! BufferRange) {
      return false;
    }

    return begin == other.begin && end == other.end;
  }

  @override
  int get hashCode => begin.hashCode ^ end.hashCode;

  @override
  String toString() => 'Range($begin, $end)';
}
