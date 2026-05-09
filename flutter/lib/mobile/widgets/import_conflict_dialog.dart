import 'package:flutter/material.dart';
import '../../common.dart';

enum _PeerChoice { override, skip }

enum ConflictResolution { overrideAll, skipAll, stop }

class ConflictDialogResult {
  // Maps peer id → true (override) / false (skip)
  final Map<String, bool> choices;
  final ConflictResolution? globalAction;

  const ConflictDialogResult({required this.choices, this.globalAction});
}

class ImportConflictDialog extends StatefulWidget {
  final List<String> conflictIds;

  const ImportConflictDialog({Key? key, required this.conflictIds})
      : super(key: key);

  @override
  State<ImportConflictDialog> createState() => _ImportConflictDialogState();
}

class _ImportConflictDialogState extends State<ImportConflictDialog> {
  late final Map<String, _PeerChoice> _choices;

  @override
  void initState() {
    super.initState();
    _choices = {for (final id in widget.conflictIds) id: _PeerChoice.skip};
  }

  void _applyGlobal(ConflictResolution resolution) {
    Navigator.of(context)
        .pop(ConflictDialogResult(choices: _buildChoiceMap(), globalAction: resolution));
  }

  Map<String, bool> _buildChoiceMap() {
    return _choices.map((id, c) => MapEntry(id, c == _PeerChoice.override));
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(translate('Conflicting Peers')),
      content: SizedBox(
        width: double.maxFinite,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              translate(
                  'The following peers already exist in Tabby. Choose how to handle each:'),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
            const SizedBox(height: 12),
            ConstrainedBox(
              constraints: BoxConstraints(
                  maxHeight: MediaQuery.of(context).size.height * 0.4),
              child: ListView.builder(
                shrinkWrap: true,
                itemCount: widget.conflictIds.length,
                itemBuilder: (context, i) {
                  final id = widget.conflictIds[i];
                  return ListTile(
                    dense: true,
                    title: Text(id,
                        style: const TextStyle(fontFamily: 'monospace')),
                    trailing: ToggleButtons(
                      isSelected: [
                        _choices[id] == _PeerChoice.override,
                        _choices[id] == _PeerChoice.skip,
                      ],
                      onPressed: (index) {
                        setState(() {
                          _choices[id] = index == 0
                              ? _PeerChoice.override
                              : _PeerChoice.skip;
                        });
                      },
                      children: [
                        Padding(
                          padding:
                              const EdgeInsets.symmetric(horizontal: 10),
                          child: Text(translate('Override')),
                        ),
                        Padding(
                          padding:
                              const EdgeInsets.symmetric(horizontal: 10),
                          child: Text(translate('Skip')),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => _applyGlobal(ConflictResolution.stop),
          child: Text(translate('Stop'),
              style: const TextStyle(color: Colors.red)),
        ),
        TextButton(
          onPressed: () => _applyGlobal(ConflictResolution.skipAll),
          child: Text(translate('Skip All')),
        ),
        TextButton(
          onPressed: () => _applyGlobal(ConflictResolution.overrideAll),
          child: Text(translate('Override All')),
        ),
        ElevatedButton(
          onPressed: () => Navigator.of(context).pop(
              ConflictDialogResult(choices: _buildChoiceMap())),
          child: Text(translate('Apply')),
        ),
      ],
    );
  }
}

Future<ConflictDialogResult?> showImportConflictDialog(
    BuildContext context, List<String> conflictIds) {
  return showDialog<ConflictDialogResult>(
    context: context,
    barrierDismissible: false,
    builder: (_) => ImportConflictDialog(conflictIds: conflictIds),
  );
}
