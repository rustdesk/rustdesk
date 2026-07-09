/// Decides whether mouse input should be dropped because this session window is
/// not the focused OS window.
///
/// Backs the opt-in "only control the focused window" feature (see
/// `kOptionControlFocusedWindowOnly`): with multiple remote sessions each in
/// their own window, a mouse merely passing over an unfocused window should not
/// move or click that remote.
///
/// Kept as a pure function (no FFI/window imports) so it can be unit-tested in
/// isolation. [isOptionEnabled] is a *lazy* getter; it is only evaluated once
/// the cheaper conditions require it, so the option read is skipped entirely for
/// the common focused-and-active case.
///
/// - [isDesktop]: focus gating only applies on desktop; mobile/web don't track
///   window focus, so they never gate.
/// - [isWindowFocused]: whether this session window currently has OS focus.
/// - [isOptionEnabled]: whether the feature is turned on (default off).
bool shouldBlockUnfocusedMouseInput({
  required bool isDesktop,
  required bool isWindowFocused,
  required bool Function() isOptionEnabled,
}) {
  if (!isDesktop) return false;
  if (isWindowFocused) return false;
  return isOptionEnabled();
}
