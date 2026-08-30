/// A circular buffer in which elements know their index in the buffer.
class IndexAwareCircularBuffer<T extends IndexedItem> {
  /// Creates a new circular list with the specified [maxLength].
  IndexAwareCircularBuffer(int maxLength)
      : _array = List<T?>.filled(maxLength, null);

  /// The backing array for this list. Length is always equal to [maxLength].
  late List<T?> _array;

  /// The number of elements in the list. This is always less than or equal to
  /// [maxLength].
  var _length = 0;

  /// The index of the first element in [_array].
  var _startIndex = 0;

  /// The start index of this list, including items that has been dropped in
  /// overflow
  var _absoluteStartIndex = 0;

  /// Gets the cyclic index for the specified regular index. The cyclic index
  /// can then be used on the backing array to get the element associated with
  /// the regular index.
  @pragma('vm:prefer-inline')
  int _getCyclicIndex(int index) {
    return (_startIndex + index) % _array.length;
  }

  /// Removes the element at [index] from the list.
  @pragma('vm:prefer-inline')
  void _dropChild(int index) {
    final cyclicIndex = _getCyclicIndex(index);
    _array[cyclicIndex]?._detach();
    _array[cyclicIndex] = null;
  }

  /// Adds the specified [child] to the list at the specified [index].
  @pragma('vm:prefer-inline')
  void _adoptChild(int index, T child) {
    final cyclicIndex = _getCyclicIndex(index);
    _array[cyclicIndex]?._detach();
    _array[cyclicIndex] = child.._attach(this, index);
  }

  /// Moves the element at [fromIndex] to [toIndex]. Both indexes should be
  /// less than [maxLength].
  @pragma('vm:prefer-inline')
  void _moveChild(int fromIndex, int toIndex) {
    final fromCyclicIndex = _getCyclicIndex(fromIndex);
    final toCyclicIndex = _getCyclicIndex(toIndex);
    _array[toCyclicIndex]?._detach();
    _array[toCyclicIndex] = _array[fromCyclicIndex]?.._move(toIndex);
    _array[fromCyclicIndex] = null;
  }

  /// Gets the element at the specified [index] in the list.
  @pragma('vm:prefer-inline')
  T? _getChild(int index) {
    return _array[_getCyclicIndex(index)];
  }

  /// The number of elements that can be stored in the list.
  int get maxLength {
    return _array.length;
  }

  /// Sets the number of elements that can be stored in the list. This operation
  /// is relatively expensive, as it requires the backing array to be
  /// reallocated.
  set maxLength(int value) {
    if (value <= 0) {
      throw ArgumentError.value(value, 'value', "maxLength can't be negative!");
    }

    if (value == _array.length) return;

    // Reconstruct array, starting at index 0. Only transfer values from the
    // indexes 0 to length.
    final newArray = List<T?>.generate(
      value,
      (index) => index < _length ? _getChild(index) : null,
    );

    _startIndex = 0;
    _array = newArray;
  }

  /// Number of elements in the list.
  int get length {
    return _length;
  }

  /// Iterates over the list and calls [callback] for each element.
  void forEach(void Function(T item) callback) {
    final length = _length;
    for (int i = 0; i < length; i++) {
      callback(_getChild(i)!);
    }
  }

  /// Gets the element at the specified [index] in the list. Throws if the
  /// index is out of bounds.
  T operator [](int index) {
    RangeError.checkValueInInterval(index, 0, length - 1, 'index');
    return _getChild(index)!;
  }

  /// Sets the element at the specified [index] in the list. Throws if the
  /// index is out of bounds.
  operator []=(int index, T value) {
    RangeError.checkValueInInterval(index, 0, length - 1, 'index');
    _adoptChild(index, value);
  }

  /// Removes all elements from the list.
  void clear() {
    for (var i = 0; i < _length; i++) {
      _dropChild(i);
    }
    _startIndex = 0;
    _length = 0;
  }

  /// Adds all elements in [items] to the list.
  void pushAll(Iterable<T> items) {
    for (var element in items) {
      push(element);
    }
  }

  /// Adds [value] to the end of the list. May cause the first element to be
  /// trimmed if the list is full.
  void push(T value) {
    _adoptChild(_length, value);

    if (_length == _array.length) {
      // When the list is full, we trim the first element
      _startIndex++;
      _absoluteStartIndex++;
      if (_startIndex == _array.length) {
        _startIndex = 0;
      }
    } else {
      // When the list is not full, we just increase the length
      _length++;
    }
  }

  /// Removes and returns the last value on the list, throws if the list is
  /// empty.
  T pop() {
    assert(_length > 0, 'Cannot pop from an empty list');
    final result = _getChild(_length - 1);
    _dropChild(_length - 1);
    _length--;
    return result!;
  }

  /// Deletes [count] elements starting at [index], shifting all elements after
  /// [index] to the left.
  void remove(int index, [int count = 1]) {
    if (count > 0) {
      if (index + count >= _length) {
        count = _length - index;
      }
      for (var i = index; i < _length - count; i++) {
        _moveChild(i + count, i);
      }
      for (var i = _length - count; i < _length; i++) {
        _dropChild(i);
      }
      _length -= count;
    }
  }

  /// Inserts [item] at [index], shifting all elements after [index] to the
  /// right. May cause the first element to be trimmed if the list is full.
  void insert(int index, T item) {
    RangeError.checkValueInInterval(index, 0, _length, 'index');

    if (index == _length) {
      return push(item);
    }

    if (index == 0 && _length >= _array.length) {
      // when something is inserted at index 0 and the list is full then
      // the new value immediately gets removed => nothing changes
      return;
    }

    for (var i = _length - 1; i >= index; i--) {
      _moveChild(i, i + 1);
    }

    _adoptChild(index, item);

    if (_length >= _array.length) {
      _startIndex += 1;
      _absoluteStartIndex += 1;
    } else {
      _length++;
    }
  }

  /// Inserts [items] at [index] in order.
  void insertAll(int index, List<T> items) {
    for (var i = items.length - 1; i >= 0; i--) {
      insert(index, items[i]);
      // when the list is full then we have to move the index down
      // as newly inserted values remove values with a lower index
      if (_length >= _array.length) {
        index--;
        if (index < 0) {
          return;
        }
      }
    }
  }

  /// Removes [count] elements starting at [index], shifting all elements after
  /// [index] to the left.
  ///
  /// This method is cheap since it does not actually modify the list, but
  /// instead just adjusts the start index and length.
  void trimStart(int count) {
    if (count > _length) count = _length;
    _startIndex += count;
    _startIndex %= _array.length;
    _length -= count;
  }

  /// Replaces all elements in the list with [replacement].
  void replaceWith(List<T> replacement) {
    for (var i = 0; i < _length; i++) {
      _dropChild(i);
    }

    var copyStart = 0;
    if (replacement.length > maxLength) {
      copyStart = replacement.length - maxLength;
    }

    for (var i = 0; i < copyStart; i++) {
      _dropChild(i);
    }

    final copyLength = replacement.length - copyStart;
    for (var i = 0; i < copyLength; i++) {
      _adoptChild(i, replacement[copyStart + i]);
    }

    _startIndex = 0;
    _length = copyLength;
  }

  /// Replaces the element at [index] with [value] and returns the replaced
  /// item.
  T swap(int index, T value) {
    final result = _getChild(index);
    _adoptChild(index, value);
    return result!;
  }

  /// Whether adding another element would cause the first element to be
  /// trimmed.
  bool get isFull => length == maxLength;

  /// Returns a list containing all elements in the list.
  List<T> toList() {
    return List<T>.generate(length, (index) => this[index]);
  }

  String debugDump() {
    final buffer = StringBuffer();
    buffer.writeln('CircularList:');
    for (var i = 0; i < _length; i++) {
      final child = _getChild(i);
      buffer.writeln('  $i: $child');
    }
    return buffer.toString();
  }
}

mixin IndexedItem {
  IndexAwareCircularBuffer? _owner;

  int? _absoluteIndex;

  /// The index of this item in the buffer. Must only be accessed when
  /// [attached] is true.
  int get index => _absoluteIndex! - _owner!._absoluteStartIndex;

  /// Whether this item is currently stored in a buffer.
  bool get attached => _owner != null;

  /// Sets the owner and index of this item. This is called by the buffer when
  /// the item is adopted.
  void _attach(IndexAwareCircularBuffer owner, int index) {
    _owner = owner;
    _absoluteIndex = owner._absoluteStartIndex + index;
  }

  /// Marks this item as detached from a buffer. This is called after the item
  /// has been removed from the buffer.
  void _detach() {
    _owner = null;
    _absoluteIndex = null;
  }

  /// Moves this item to [newIndex] in the buffer.
  void _move(int newIndex) {
    assert(attached);
    _absoluteIndex = _owner!._absoluteStartIndex + newIndex;
  }
}
