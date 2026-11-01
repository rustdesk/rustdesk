import 'package:flutter/material.dart';

import 'session_palette.dart';
import 'session_key_menu.dart';
export 'session_key_menu.dart' show SessionModifier;

/// A local keyboard and trackpad for a folded session. All input is delegated
/// to the session's existing permission-checked input path.
class SessionControlPanel extends StatefulWidget {
  const SessionControlPanel({
    super.key,
    required this.toolbar,
    required this.enabled,
    this.keyboardEnabled = true,
    required this.onKey,
    required this.onMove,
    required this.onClick,
    required this.keyboardLabel,
    required this.mouseLabel,
    required this.leftClickLabel,
    required this.rightClickLabel,
    this.spaceLabel = 'Space',
    this.isMac = false,
    this.modifiers = const {},
    this.onModifier,
    this.onShortcut,
  });

  final Widget toolbar;
  final bool enabled;
  final bool keyboardEnabled;
  final ValueChanged<String> onKey;
  final ValueChanged<Offset> onMove;
  final ValueChanged<bool> onClick;
  final String keyboardLabel;
  final String mouseLabel;
  final String leftClickLabel;
  final String rightClickLabel;
  final String spaceLabel;
  final bool isMac;
  final Set<SessionModifier> modifiers;
  final ValueChanged<SessionModifier>? onModifier;
  final ValueChanged<String>? onShortcut;

  @override
  State<SessionControlPanel> createState() => _SessionControlPanelState();
}

class _SessionControlPanelState extends State<SessionControlPanel> {
  bool? _keyboard;
  bool _shift = false;
  bool _symbols = false;
  bool get shifted => widget.onModifier != null
      ? widget.modifiers.contains(SessionModifier.shift)
      : _shift;
  bool get canType => widget.enabled && widget.keyboardEnabled;

  String character(String value) => !shifted
      ? value
      : const {
            '1': '!',
            '2': '@',
            '3': '#',
            '4': r'$',
            '5': '%',
            '6': '^',
            '7': '&',
            '8': '*',
            '9': '(',
            '0': ')',
            ',': '<',
            '.': '>',
            '-': '_',
            '=': '+',
            '/': '?',
            ';': ':',
            "'": '"',
            '[': '{',
            ']': '}',
            r'\': '|',
            '`': '~',
          }[value] ??
          value.toUpperCase();

  void typeKey(String value) {
    final releaseShift = shifted && (value.length == 1 || value == 'VK_SPACE');
    widget.onKey(value.length == 1 ? character(value) : value);
    if (releaseShift) {
      if (widget.onModifier != null) {
        widget.onModifier!(SessionModifier.shift);
      } else {
        setState(() => _shift = false);
      }
    }
  }

  Widget symbols() => key(_symbols ? 'ABC' : '?123', '',
      utility: true, onPressed: () => setState(() => _symbols = !_symbols));

  Widget key(String label, String value,
      {int flex = 1,
      IconData? icon,
      bool utility = false,
      bool selected = false,
      VoidCallback? onPressed}) {
    final palette = SessionPalette.of(context);
    return Expanded(
        flex: flex,
        child: SizedBox(
          height: 48,
          child: Padding(
              padding: const EdgeInsets.all(2),
              child: DecoratedBox(
                decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                    boxShadow: [
                      BoxShadow(
                          color: palette.shadow,
                          offset: const Offset(0, 1),
                          blurRadius: 1)
                    ]),
                child: TextButton(
                  style: ButtonStyle(
                    padding: WidgetStateProperty.all(EdgeInsets.zero),
                    minimumSize: WidgetStateProperty.all(const Size(44, 44)),
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    shape: WidgetStateProperty.all(RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                        side: BorderSide(color: palette.line, width: .5))),
                    backgroundColor: WidgetStateProperty.resolveWith((states) =>
                        selected || states.contains(WidgetState.pressed)
                            ? palette.selected
                            : utility
                                ? palette.utility
                                : palette.key),
                    foregroundColor: WidgetStateProperty.resolveWith(
                        (states) => states.contains(WidgetState.disabled)
                            ? palette.secondary.withOpacity(.5)
                            : selected
                                ? palette.accent
                                : palette.foreground),
                    textStyle: WidgetStateProperty.all(Theme.of(context)
                        .textTheme
                        .bodyMedium
                        ?.copyWith(fontSize: 15, fontWeight: FontWeight.w500)),
                  ),
                  onPressed:
                      !canType ? null : onPressed ?? () => typeKey(value),
                  child: icon == null
                      ? Text(label,
                          maxLines: 1, overflow: TextOverflow.ellipsis)
                      : Icon(icon, size: 20, semanticLabel: label),
                ),
              )),
        ));
  }

  List<Widget> letters(String values) =>
      values.split('').map((value) => key(character(value), value)).toList();

  Widget shift() => Tooltip(
      message: 'Shift',
      child: Semantics(
        toggled: shifted,
        child: Row(children: [
          key('Shift', '',
              icon: Icons.arrow_upward,
              utility: true,
              selected: shifted, onPressed: () {
            if (widget.onModifier != null) {
              widget.onModifier?.call(SessionModifier.shift);
            } else {
              setState(() => _shift = !_shift);
            }
          })
        ]),
      ));

  Widget modifier(String label, SessionModifier modifier, IconData icon) =>
      key(label, '',
          utility: true,
          icon: icon,
          selected: widget.modifiers.contains(modifier),
          onPressed: () => widget.onModifier?.call(modifier));

  Widget keyboard(double availableWidth) {
    final minimum = widget.isMac ? 528.0 : 480.0;
    final width = availableWidth.clamp(minimum, 720.0).toDouble();
    return Center(
        child: SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: SizedBox(
          width: width,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: widget.isMac
                ? [
                    Row(children: [
                      ...letters('1234567890'),
                      key('Backspace', 'VK_BACK',
                          icon: Icons.backspace_outlined, utility: true)
                    ]),
                    Row(children: [
                      key('tab', 'VK_TAB', utility: true),
                      ...letters(_symbols ? r'-/:;()$&@"' : 'qwertyuiop')
                    ]),
                    Row(children: [
                      ...letters(_symbols ? '.,[]{}+=_' : 'asdfghjkl'),
                      key('Return', 'VK_RETURN',
                          flex: 2, icon: Icons.keyboard_return, utility: true)
                    ]),
                    Row(children: [
                      Expanded(child: shift()),
                      ...letters(_symbols ? r"?!'\`~|" : 'zxcvbnm'),
                      if (!_symbols) ...letters(',.'),
                      key('Up', 'VK_UP',
                          icon: Icons.keyboard_arrow_up, utility: true)
                    ]),
                    Row(children: [
                      symbols(),
                      key('esc', 'VK_ESCAPE', utility: true),
                      modifier('Control', SessionModifier.control,
                          Icons.keyboard_control_key),
                      modifier('Option', SessionModifier.option,
                          Icons.keyboard_option_key),
                      modifier('Command', SessionModifier.command,
                          Icons.keyboard_command_key),
                      key(widget.spaceLabel, 'VK_SPACE', flex: 4),
                      key('Left', 'VK_LEFT',
                          icon: Icons.keyboard_arrow_left, utility: true),
                      key('Down', 'VK_DOWN',
                          icon: Icons.keyboard_arrow_down, utility: true),
                      key('Right', 'VK_RIGHT',
                          icon: Icons.keyboard_arrow_right, utility: true),
                    ]),
                  ]
                : [
                    Row(children: letters('1234567890')),
                    Row(
                        children:
                            letters(_symbols ? r'-/:;()$&@"' : 'qwertyuiop')),
                    Row(children: [
                      const Spacer(),
                      ...letters(_symbols ? '.,[]{}+=_' : 'asdfghjkl'),
                      const Spacer()
                    ]),
                    Row(children: [
                      Expanded(child: shift()),
                      ...letters(_symbols ? r"?!'\`~|" : 'zxcvbnm'),
                      key('Backspace', 'VK_BACK',
                          icon: Icons.backspace_outlined, utility: true)
                    ]),
                    Row(children: [
                      symbols(),
                      key('Esc', 'VK_ESCAPE', utility: true),
                      key('Tab', 'VK_TAB', utility: true),
                      key(widget.spaceLabel, 'VK_SPACE', flex: 3),
                      key('Left', 'VK_LEFT',
                          icon: Icons.keyboard_arrow_left, utility: true),
                      key('Up', 'VK_UP',
                          icon: Icons.keyboard_arrow_up, utility: true),
                      key('Down', 'VK_DOWN',
                          icon: Icons.keyboard_arrow_down, utility: true),
                      key('Right', 'VK_RIGHT',
                          icon: Icons.keyboard_arrow_right, utility: true),
                      key('Enter', 'VK_RETURN',
                          icon: Icons.keyboard_return, utility: true)
                    ]),
                  ],
          )),
    ));
  }

  Widget trackpad(SessionPalette palette) => Semantics(
        label: widget.mouseLabel,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onPanUpdate:
              !widget.enabled ? null : (event) => widget.onMove(event.delta),
          onTap: !widget.enabled ? null : () => widget.onClick(true),
          onSecondaryTap: !widget.enabled ? null : () => widget.onClick(false),
          onLongPress: !widget.enabled ? null : () => widget.onClick(false),
          child: Container(
            margin: const EdgeInsets.fromLTRB(4, 8, 4, 4),
            decoration: BoxDecoration(
                color: palette.pad,
                border: Border.all(color: palette.line),
                borderRadius: BorderRadius.circular(12)),
            alignment: Alignment.center,
            child: Icon(Icons.touch_app_outlined,
                size: 20, color: palette.secondary.withOpacity(.65)),
          ),
        ),
      );

  @override
  Widget build(BuildContext context) {
    final palette = SessionPalette.of(context);
    return Material(
        color: palette.deck,
        child: Column(children: [
          SizedBox(width: double.infinity, height: 48, child: widget.toolbar),
          Expanded(
              child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: LayoutBuilder(builder: (context, constraints) {
              final minimum = widget.isMac ? 528.0 : 480.0;
              final wantKeyboard = _keyboard ?? constraints.maxWidth >= minimum;
              final showKeyboard = wantKeyboard && constraints.maxHeight >= 336;
              return Column(children: [
                if (showKeyboard) keyboard(constraints.maxWidth),
                Expanded(
                    child: wantKeyboard && !showKeyboard
                        ? SingleChildScrollView(
                            child: keyboard(constraints.maxWidth))
                        : trackpad(palette)),
                SizedBox(
                    height: 48,
                    child: Row(children: [
                      IconButton(
                          tooltip: widget.keyboardLabel,
                          isSelected: wantKeyboard,
                          color:
                              wantKeyboard ? palette.accent : palette.secondary,
                          onPressed: () =>
                              setState(() => _keyboard = !wantKeyboard),
                          icon: const Icon(Icons.keyboard_outlined, size: 20)),
                      SessionKeyMenu(
                        enabled: canType,
                        isMac: widget.isMac,
                        modifiers: widget.modifiers,
                        onModifier: (modifier) =>
                            widget.onModifier?.call(modifier),
                        onKey: widget.onKey,
                        onShortcut: (key) => widget.onShortcut?.call(key),
                      ),
                      Expanded(
                          child: TextButton(
                              onPressed: widget.enabled
                                  ? () => widget.onClick(true)
                                  : null,
                              style: TextButton.styleFrom(
                                  foregroundColor: palette.foreground),
                              child: Text(widget.leftClickLabel,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w500)))),
                      SizedBox(
                          width: 1,
                          height: 16,
                          child: ColoredBox(color: palette.line)),
                      Expanded(
                          child: TextButton(
                              onPressed: widget.enabled
                                  ? () => widget.onClick(false)
                                  : null,
                              style: TextButton.styleFrom(
                                  foregroundColor: palette.foreground),
                              child: Text(widget.rightClickLabel,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w500)))),
                    ])),
              ]);
            }),
          )),
        ]));
  }
}
