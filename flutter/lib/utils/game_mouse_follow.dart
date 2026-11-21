/// Opt-in per peer. A manual release is respected until the remote cursor
/// has appeared and disappeared again.
class GameMouseFollow {
  GameMouseFollow({this.readPreference, this.writePreference});

  final bool Function()? readPreference;
  final void Function(bool)? writePreference;
  bool enabled = false;
  bool? visible;
  bool pending = false;
  bool altHeld = false;
  bool get wantsPointer => visible == true || altHeld;

  bool updateAlt(bool down) {
    if (!enabled || altHeld == down) return false;
    altHeld = down;
    pending = !down && visible == false;
    return true;
  }

  void enable(bool value, {bool remember = false}) {
    if (remember) writePreference?.call(value);
    enabled = value;
    altHeld = false;
    pending = enabled && visible == false;
  }

  bool updateVisibility(bool value) {
    if (visible == value) return false;
    if (visible == null && readPreference != null) enabled = readPreference!();
    visible = value;
    pending = enabled && !value && !altHeld;
    return enabled;
  }

  void engaged() => pending = false;

  void reset() {
    visible = null;
    enable(false);
  }
}
