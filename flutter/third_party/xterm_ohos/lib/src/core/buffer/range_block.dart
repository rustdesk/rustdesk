import 'dart:math';

import 'package:xterm/src/core/buffer/cell_offset.dart';
import 'package:xterm/src/core/buffer/range.dart';
import 'package:xterm/src/core/buffer/segment.dart';

class BufferRangeBlock extends BufferRange {
  BufferRangeBlock(super.begin, super.end);

  BufferRangeBlock.collapsed(super.begin) : super.collapsed();

  @override
  bool get isNormalized {
    // A block range is normalized if begin is the top left corner of the range
    // and end the bottom right corner.
    return (begin.isBefore(end) && begin.x <= end.x) || begin.isEqual(end);
  }

  @override
  BufferRangeBlock get normalized {
    if (isNormalized) {
      return this;
    }
    // Determine new normalized begin and end offset, such that begin is the
    // top left corner and end is the bottom right corner of the block.
    final normalBegin = CellOffset(min(begin.x, end.x), min(begin.y, end.y));
    final normalEnd = CellOffset(max(begin.x, end.x), max(begin.y, end.y));
    return BufferRangeBlock(normalBegin, normalEnd);
  }

  @override
  Iterable<BufferSegment> toSegments() sync* {
    var begin = this.begin;
    var end = this.end;

    if (!isNormalized) {
      end = this.begin;
      begin = this.end;
    }

    final startX = min(begin.x, end.x);
    final endX = max(begin.x, end.x);
    for (var i = begin.y; i <= end.y; i++) {
      yield BufferSegment(this, i, startX, endX);
    }
  }

  @override
  bool contains(CellOffset position) {
    var begin = this.begin;
    var end = this.end;

    if (!isNormalized) {
      end = this.begin;
      begin = this.end;
    }
    if (!(begin.y <= position.y && position.y <= end.y)) {
      return false;
    }

    final startX = min(begin.x, end.x);
    final endX = max(begin.x, end.x);
    return startX <= position.x && position.x <= endX;
  }

  @override
  BufferRangeBlock merge(BufferRange range) {
    // Enlarge the block such that both borders of the range
    // are within the selected block.
    return extend(range.begin).extend(range.end);
  }

  @override
  BufferRangeBlock extend(CellOffset position) {
    // If the position is within the block, there is nothing to do.
    if (contains(position)) {
      return this;
    }
    // Otherwise normalize the block and push the borders outside up to
    // the position to which the block has to extended.
    final normal = normalized;
    final extendBegin = CellOffset(
      min(normal.begin.x, position.x),
      min(normal.begin.y, position.y),
    );
    final extendEnd = CellOffset(
      max(normal.end.x, position.x),
      max(normal.end.y, position.y),
    );
    return BufferRangeBlock(extendBegin, extendEnd);
  }

  @override
  operator ==(Object other) {
    if (identical(this, other)) {
      return true;
    }

    if (other is! BufferRangeBlock) {
      return false;
    }

    return begin == other.begin && end == other.end;
  }

  @override
  int get hashCode => begin.hashCode ^ end.hashCode;

  @override
  String toString() => 'Block Range($begin, $end)';
}
