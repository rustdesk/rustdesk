/// Fixed-size list based lookup table, optimized for small positive integer
/// keys.
class FastLookupTable<T> {
  FastLookupTable(Map<int, T> data) {
    var maxIndex = data.keys.first;

    for (var key in data.keys) {
      if (key > maxIndex) {
        maxIndex = key;
      }
    }

    _maxIndex = maxIndex;

    _table = List<T?>.filled(maxIndex + 1, null);

    for (var entry in data.entries) {
      _table[entry.key] = entry.value;
    }
  }

  late final List<T?> _table;
  late final int _maxIndex;

  T? operator [](int index) {
    if (index > _maxIndex) {
      return null;
    }

    return _table[index];
  }

  int get maxIndex => _maxIndex;
}
