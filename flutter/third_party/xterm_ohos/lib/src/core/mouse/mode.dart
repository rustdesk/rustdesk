/// https://terminalguide.namepad.de/mouse/
enum MouseMode {
  none,

  clickOnly,

  upDownScroll(reportScroll: true),

  upDownScrollDrag(reportScroll: true),

  upDownScrollMove(reportScroll: true),
  ;

  const MouseMode({this.reportScroll = false});

  final bool reportScroll;
}

/// https://terminalguide.namepad.de/mouse/
enum MouseReportMode {
  /// The default mouse reporting mode where digits are encoded as bytes with
  /// `32 + code`. This mode has a range from 1 to 223.
  normal,

  /// When code < 96 this is the same as [normal], otherwise the `code + 32` is
  /// encoded as 2 bytes in UTF-8. This mode has a range from 1 to 2015.
  utf,

  /// In this mode the code are encoded as 10-based numbers. Tha range is
  /// unlimited.
  sgr,

  /// Similar to [sgr], the difference is that the button id is encoded as
  /// `32 + code`.
  urxvt,
}
