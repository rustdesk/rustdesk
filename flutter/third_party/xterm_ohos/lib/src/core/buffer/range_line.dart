import 'package:xterm/src/core/buffer/cell_offset.dart';
import 'package:xterm/src/core/buffer/range.dart';
import 'package:xterm/src/core/buffer/segment.dart';

class BufferRangeLine extends BufferRange {
  BufferRangeLine(super.begin, super.end);

  BufferRangeLine.collapsed(super.begin) : super.collapsed();

  @override
  BufferRangeLine get normalized {
    return isNormalized ? this : BufferRangeLine(end, begin);
  }

  @override
  Iterable<BufferSegment> toSegments() sync* {
    final self = normalized;
    for (var i = self.begin.y; i <= self.end.y; i++) {
      var startX = i == self.begin.y ? self.begin.x : null;
      var endX = i == self.end.y ? self.end.x : null;
      yield BufferSegment(this, i, startX, endX);
    }
  }

  @override
  bool contains(CellOffset position) {
    final self = normalized;
    return self.begin.isBeforeOrSame(position) &&
        self.end.isAfterOrSame(position);
  }

  @override
  BufferRangeLine merge(BufferRange range) {
    final self = normalized;
    final begin = self.begin.isBefore(range.begin) ? self.begin : range.begin;
    final end = self.end.isAfter(range.end) ? self.end : range.end;
    return BufferRangeLine(begin, end);
  }

  @override
  BufferRangeLine extend(CellOffset position) {
    final self = normalized;
    final begin = self.begin.isAfter(position) ? position : self.begin;
    final end = self.end.isBefore(position) ? position : self.end;
    return BufferRangeLine(begin, end);
  }

  @override
  operator ==(Object other) {
    if (identical(this, other)) {
      return true;
    }

    if (other is! BufferRangeLine) {
      return false;
    }

    return begin == other.begin && end == other.end;
  }

  @override
  int get hashCode => begin.hashCode ^ end.hashCode;

  @override
  String toString() => 'Line Range($begin, $end)';
}
