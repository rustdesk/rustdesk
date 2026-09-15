import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/display_scale_model.dart';

class DisplayScale extends StatefulWidget {
  final String Function(String) translate;
  final DisplayScaleModel controller;
  final bool excludeInputSemantics;

  const DisplayScale(
      {super.key,
      required this.translate,
      required this.controller,
      this.excludeInputSemantics = false});

  @override
  State<DisplayScale> createState() => _DisplayScaleState();
}

class _DisplayScaleState extends State<DisplayScale> {
  final _text = TextEditingController();
  final _menuFocus = FocusNode();
  final _firstItemFocus = FocusNode();
  bool _editing = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_sync);
    _sync();
  }

  void _sync() {
    if (_editing) return;
    final text = widget.controller.draftText;
    if (_text.text == text) return;
    _editing = true;
    _text.text = text;
    _editing = false;
  }

  void _edit(String text) {
    if (_editing || widget.controller.current?.custom == null) return;
    _editing = true;
    widget.controller.edit(text);
    _editing = false;
  }

  @override
  void dispose() {
    widget.controller.removeListener(_sync);
    _menuFocus.dispose();
    _firstItemFocus.dispose();
    _text.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final current = widget.controller.current;
    final translate = widget.translate;
    final hasCustom = current?.custom != null || widget.controller.customMode;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(translate('System scaling'),
            style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        if (widget.controller.busy) const LinearProgressIndicator(),
        if (current != null)
          LayoutBuilder(builder: (context, constraints) {
            return MenuAnchor(
              childFocusNode: _menuFocus,
              onClose: _menuFocus.requestFocus,
              consumeOutsideTap: true,
              crossAxisUnconstrained: false,
              style: MenuStyle(
                minimumSize:
                    WidgetStatePropertyAll(Size(constraints.maxWidth, 0)),
                maximumSize:
                    WidgetStatePropertyAll(Size(constraints.maxWidth, 240)),
              ),
              menuChildren: [
                if (hasCustom)
                  MenuItemButton(
                      focusNode: _firstItemFocus,
                      onPressed:
                          !widget.controller.busy && current.custom != null
                              ? widget.controller.useCustom
                              : null,
                      child: Text(translate('Custom'))),
                for (final percent in current.options)
                  MenuItemButton(
                      focusNode: !hasCustom && percent == current.options.first
                          ? _firstItemFocus
                          : null,
                      onPressed: widget.controller.busy
                          ? null
                          : () => widget.controller.select(percent),
                      child: Text(
                          '${formatDisplayScale(percent)}%${percent == current.recommended ? ' (${translate('Recommended')})' : ''}')),
              ],
              builder: (context, menu, _) => OutlinedButton(
                key: const ValueKey('system-scale-menu'),
                focusNode: _menuFocus,
                style: OutlinedButton.styleFrom(
                    foregroundColor: Theme.of(context).colorScheme.onSurface,
                    minimumSize: const Size.fromHeight(48),
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(4))),
                onPressed: widget.controller.busy
                    ? null
                    : () {
                        if (menu.isOpen) {
                          menu.close();
                        } else {
                          menu.open();
                          _firstItemFocus.requestFocus();
                        }
                      },
                child: Row(children: [
                  Expanded(
                      child: Text(widget.controller.customMode
                          ? translate('Custom')
                          : '${formatDisplayScale(widget.controller.percent ?? current.percent)}%')),
                  const Icon(Icons.arrow_drop_down),
                ]),
              ),
            );
          }),
        if (!widget.controller.valid && !widget.controller.customMode)
          Text(translate('Select a scaling level supported by the display.'),
              style: TextStyle(color: Theme.of(context).colorScheme.error)),
        if (current != null && widget.controller.customMode) ...[
          const SizedBox(height: 12),
          Row(children: [
            IconButton(
                key: const ValueKey('scale-decrease'),
                tooltip: translate('Decrease'),
                icon: const Icon(Icons.remove),
                onPressed: !widget.controller.busy &&
                        widget.controller.valid &&
                        current.adjacent(widget.controller.percent!, -1) != null
                    ? () => widget.controller.step(-1)
                    : null),
            Expanded(
                child: ExcludeSemantics(
                    excluding: widget.excludeInputSemantics,
                    child: TextField(
                        key: const ValueKey('custom-scale-input'),
                        controller: _text,
                        onChanged: _edit,
                        enabled:
                            !widget.controller.busy && current.custom != null,
                        keyboardType: const TextInputType.numberWithOptions(
                            decimal: true),
                        decoration: InputDecoration(
                            isDense: true,
                            border: const OutlineInputBorder(),
                            suffixText: '%',
                            errorMaxLines: 3,
                            errorText: !widget.controller.valid &&
                                    widget.controller.suggestion == null
                                ? translate(
                                    'Select a scaling level supported by the display.')
                                : null)))),
            IconButton(
                key: const ValueKey('scale-increase'),
                tooltip: translate('Increase'),
                icon: const Icon(Icons.add),
                onPressed: !widget.controller.busy &&
                        widget.controller.valid &&
                        current.adjacent(widget.controller.percent!, 1) != null
                    ? () => widget.controller.step(1)
                    : null),
          ]),
          if (widget.controller.suggestion != null)
            Align(
                alignment: Alignment.centerLeft,
                child: TextButton(
                    key: const ValueKey('accept-scale-suggestion'),
                    onPressed: widget.controller.busy
                        ? null
                        : widget.controller.acceptSuggestion,
                    child: Text(
                        '${translate('Use nearest supported scale')}: ${formatDisplayScale(widget.controller.suggestion!)}%'))),
        ],
        if (widget.controller.error != null) ...[
          Text(translate(widget.controller.error!),
              style: TextStyle(color: Theme.of(context).colorScheme.error)),
          Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                  onPressed:
                      widget.controller.busy ? null : widget.controller.refresh,
                  icon: const Icon(Icons.refresh, size: 18),
                  label: Text(translate('Refresh')))),
        ],
      ],
    );
  }
}
