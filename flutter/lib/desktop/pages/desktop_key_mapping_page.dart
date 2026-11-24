import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/key_mapping_model.dart';

String get _localSystem =>
    isMacOS ? 'macos' : (isWindows ? 'windows' : 'linux');

class DesktopKeyMappingPage extends StatefulWidget {
  const DesktopKeyMappingPage({Key? key}) : super(key: key);

  @override
  State<DesktopKeyMappingPage> createState() => _DesktopKeyMappingPageState();
}

class _DesktopKeyMappingPageState extends State<DesktopKeyMappingPage> {
  final KeyMappingConfig _config = KeyMappingConfig.load();
  // A Windows or Linux keyboard is most often remapped for a Mac peer.
  String _system = isMacOS ? 'windows' : 'macos';

  Future<void> _update(void Function() change) async {
    setState(change);
    await _config.save();
  }

  Future<void> _edit({String? from}) async {
    final result = await showDialog<(String, String)>(
      context: context,
      builder: (_) => _KeyMappingDialog(
        system: _system,
        from: from,
        to: from == null ? null : _config.mapsFor(_system)[from],
      ),
    );
    if (result == null) return;
    await _update(() {
      if (from != null && from != result.$1) _config.remove(_system, from);
      _config.set(_system, result.$1, result.$2);
    });
  }

  @override
  Widget build(BuildContext context) {
    final maps = _config.mapsFor(_system);
    final entries = kKeyMappingKeyIds
        .where(maps.containsKey)
        .map((from) => MapEntry(from, maps[from]!))
        .toList();
    return Scaffold(
      appBar: AppBar(title: Text(translate('Key mapping'))),
      body: ListView(
        padding: const EdgeInsets.all(20),
        children: [
          Text(translate('key-mapping-tip'),
              style: TextStyle(color: Theme.of(context).hintColor)),
          const SizedBox(height: 16),
          Row(children: [
            Text(translate('Remote system')),
            const SizedBox(width: 12),
            for (final system in kKeyMappingRemoteSystems)
              ChoiceChip(
                label: Text(keyMappingSystemLabel(system)),
                selected: _system == system,
                onSelected: (_) => setState(() => _system = system),
              ).marginOnly(right: 8),
          ]),
          const SizedBox(height: 16),
          if (entries.isEmpty)
            Text(translate('No key mappings'),
                    style: TextStyle(color: Theme.of(context).hintColor))
                .marginSymmetric(vertical: 12),
          for (final e in entries) _row(e.key, e.value),
          const SizedBox(height: 12),
          Wrap(spacing: 12, runSpacing: 8, children: [
            ElevatedButton.icon(
              onPressed: () => _edit(),
              icon: const Icon(Icons.add),
              label: Text(translate('Add mapping')),
            ),
            if (_system == 'macos' && !isMacOS)
              OutlinedButton.icon(
                onPressed: () => _update(_config.applySwapAltCommand),
                icon: const Icon(Icons.swap_horiz),
                label: Text(translate('Swap Alt and Command')),
              ),
          ]),
        ],
      ),
    );
  }

  Widget _row(String from, String to) {
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ListTile(
        title: Row(children: [
          _KeyCap(keyMappingKeyLabel(from, _localSystem)),
          const Icon(Icons.arrow_forward).marginSymmetric(horizontal: 12),
          _KeyCap(keyMappingKeyLabel(to, _system)),
        ]),
        trailing: Row(mainAxisSize: MainAxisSize.min, children: [
          IconButton(
            tooltip: translate('Edit'),
            icon: const Icon(Icons.edit_outlined),
            onPressed: () => _edit(from: from),
          ),
          IconButton(
            tooltip: translate('Delete'),
            icon: const Icon(Icons.delete_outline),
            onPressed: () => _update(() => _config.remove(_system, from)),
          ),
        ]),
      ),
    );
  }
}

class _KeyCap extends StatelessWidget {
  final String label;
  final bool active;
  final VoidCallback? onTap;

  const _KeyCap(this.label, {this.active = false, this.onTap});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(6),
      child: Container(
        constraints: const BoxConstraints(minWidth: 120),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
              color: active ? theme.colorScheme.primary : theme.dividerColor,
              width: active ? 2 : 1),
        ),
        child: Text(label,
            textAlign: TextAlign.center,
            style: const TextStyle(fontWeight: FontWeight.w600)),
      ),
    );
  }
}

/// Captures the key to remap and its replacement: press each key in turn,
/// or pick it from the list when it cannot be pressed on this keyboard.
class _KeyMappingDialog extends StatefulWidget {
  final String system;
  final String? from;
  final String? to;

  const _KeyMappingDialog({required this.system, this.from, this.to});

  @override
  State<_KeyMappingDialog> createState() => _KeyMappingDialogState();
}

class _KeyMappingDialogState extends State<_KeyMappingDialog> {
  final FocusNode _focusNode = FocusNode();
  late String? _from = widget.from;
  late String? _to = widget.to;
  // 0: capturing the original key, 1: capturing the target, null: done.
  late int? _capturing = widget.from == null ? 0 : null;
  // Windows sends a fake Left Ctrl ahead of AltGr, so a key is taken when it
  // is released: the last key pressed wins.
  String? _pressed;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    if (_capturing == null) return KeyEventResult.ignored;
    if (event is KeyDownEvent) {
      _pressed = keyMappingIdFromPhysicalKey(event.physicalKey) ?? _pressed;
    } else if (event is KeyUpEvent && _pressed != null) {
      final id = _pressed!;
      _pressed = null;
      setState(() {
        if (_capturing == 0) {
          _from = id;
          _capturing = _to == null ? 1 : null;
        } else {
          _to = id;
          _capturing = null;
        }
      });
    }
    return KeyEventResult.handled;
  }

  Widget _field(int index, String label, String? value, String system) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(translate(label), style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 6),
        Row(children: [
          _KeyCap(
            _capturing == index
                ? translate('Press a key')
                : (value == null ? '-' : keyMappingKeyLabel(value, system)),
            active: _capturing == index,
            onTap: () {
              _pressed = null;
              setState(() => _capturing = index);
              _focusNode.requestFocus();
            },
          ),
          PopupMenuButton<String>(
            tooltip: translate('Select'),
            icon: const Icon(Icons.arrow_drop_down),
            itemBuilder: (_) => [
              for (final id in kKeyMappingKeyIds)
                PopupMenuItem(
                    value: id, child: Text(keyMappingKeyLabel(id, system))),
            ],
            onSelected: (id) => setState(() {
              if (index == 0) {
                _from = id;
              } else {
                _to = id;
              }
              _capturing = null;
            }),
          ),
        ]),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final from = _from;
    final to = _to;
    return AlertDialog(
      title: Text(translate('Key mapping')),
      content: Focus(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: _onKeyEvent,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            _field(0, 'Key on this device', from, _localSystem),
            const Icon(Icons.arrow_forward)
                .marginOnly(left: 8, right: 8, bottom: 12),
            _field(1, 'Sent to remote as', to, widget.system),
          ],
        ),
      ),
      actions: [
        dialogButton('Cancel',
            onPressed: () => Navigator.of(context).pop(), isOutline: true),
        dialogButton('OK',
            onPressed: from == null || to == null
                ? null
                : () => Navigator.of(context).pop((from, to))),
      ],
    );
  }
}
