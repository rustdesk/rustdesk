/// One outgoing session keeps the Android process in the foreground.
/// A second open does not ask again, and a close with nothing open is ignored.
class OutgoingConnectionHold {
  var _held = false;

  bool open() {
    if (_held) return false;
    _held = true;
    return true;
  }

  bool close() {
    if (!_held) return false;
    _held = false;
    return true;
  }
}
