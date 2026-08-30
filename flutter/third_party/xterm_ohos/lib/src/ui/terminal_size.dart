class TerminalSize {
  final int width;

  final int height;

  const TerminalSize(this.width, this.height);

  @override
  String toString() => 'TerminalSize($width, $height)';

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) {
      return true;
    }
    if (other is! TerminalSize) {
      return false;
    }
    return other.width == width && other.height == height;
  }

  @override
  int get hashCode => width.hashCode ^ height.hashCode;
}
