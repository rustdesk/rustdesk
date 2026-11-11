// Per-peer keyboard shortcut exclude list (Rust: `src/keyboard/exclude.rs`).
//
// Every shortcut is sent to the remote PC by default. A preset left unchecked, and every custom chord added here, is
// excluded: while a session window has the keyboard, the chord goes to the local PC.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';

const String kOptionKeyboardExcludeShortcuts = 'keyboard-exclude-shortcuts';

/// Canonical chords (`Ctrl+Alt+Shift+Win+<key>`) offered as checkboxes.
const List<String> kKeyboardShortcutPresets = [
  'Win',
  'Alt+Shift',
  'Ctrl+Shift',
  'Alt+Tab',
  'Alt+Esc',
  'Alt+F4',
  'Ctrl+Esc',
  'Ctrl+Shift+Esc',
  'Win+D',
  'Win+E',
  'Win+R',
  'Win+V',
  'Win+Tab',
  'Win+Space',
  'Shift+Win+S',
  'Win+Left',
  'Win+Right',
  'Win+Up',
  'Win+Down',
  'Ctrl+Win+Left',
  'Ctrl+Win+Right',
  'Ctrl+Win+D',
  'PrintScreen',
];

List<String> _parseList(String raw) => raw
    .split(',')
    .map((e) => bind.mainNormalizeKeyboardShortcut(chord: e))
    .where((e) => e.isNotEmpty)
    .toSet()
    .toList();

void keyboardExcludeShortcutsDialog(FFI ffi) async {
  final sessionId = ffi.sessionId;
  final raw = await bind.sessionGetPeerOption(
      sessionId: sessionId, name: kOptionKeyboardExcludeShortcuts);
  final excluded = _parseList(raw);
  final custom =
      excluded.where((e) => !kKeyboardShortcutPresets.contains(e)).toList();
  var recording = false;
  Timer? poll;
  DateTime? recordingSince;

  void stopRecording() {
    poll?.cancel();
    poll = null;
    if (recording) {
      bind.mainStopKeyboardShortcutRecording();
    }
    recording = false;
  }

  ffi.dialogManager.show((setState, close, context) {
    void add(String chord) {
      if (chord.isEmpty) return;
      if (!excluded.contains(chord)) {
        excluded.add(chord);
      }
      if (!kKeyboardShortcutPresets.contains(chord) &&
          !custom.contains(chord)) {
        custom.add(chord);
      }
    }

    // The keyboard hook records the next chord exclusively: no key event reaches another keyboard hook
    // (e.g. AutoHotkey) or this dialog until a chord is complete.
    void startRecording() {
      bind.mainStartKeyboardShortcutRecording();
      recordingSince = DateTime.now();
      setState(() => recording = true);
      poll = Timer.periodic(const Duration(milliseconds: 50), (_) {
        final chord = bind.mainTakeRecordedKeyboardShortcut();
        final timedOut =
            DateTime.now().difference(recordingSince!).inSeconds >= 15;
        if (chord.isEmpty && !timedOut) return;
        stopRecording();
        try {
          setState(() => add(chord));
        } catch (_) {
          // The dialog was dismissed by `dialogManager`.
        }
      });
    }

    void onClose() {
      stopRecording();
      close();
    }

    Future<void> submit() async {
      await bind.sessionSetKeyboardExcludeShortcuts(
          sessionId: sessionId, value: excluded.join(','));
      onClose();
    }

    return CustomAlertDialog(
      title: Text('Shortcuts sent to the remote PC'),
      content: SizedBox(
        width: 420,
        height: 480,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Checked: sent to the remote PC. Unchecked: kept on this PC.'),
            const SizedBox(height: 8),
            Expanded(
              child: ListView(
                children: [
                  ...kKeyboardShortcutPresets.map((chord) => CheckboxListTile(
                        dense: true,
                        title: Text(chord),
                        value: !excluded.contains(chord),
                        onChanged: (v) => setState(() {
                          if (v == true) {
                            excluded.remove(chord);
                          } else if (!excluded.contains(chord)) {
                            excluded.add(chord);
                          }
                        }),
                      )),
                  ...custom.map((chord) => ListTile(
                        dense: true,
                        leading: const Icon(Icons.block, size: 18),
                        title: Text(chord),
                        trailing: IconButton(
                          icon: const Icon(Icons.delete_outline, size: 18),
                          onPressed: () => setState(() {
                            custom.remove(chord);
                            excluded.remove(chord);
                          }),
                        ),
                      )),
                ],
              ),
            ),
            Row(
              children: [
                Expanded(
                  child: Text(recording
                      ? 'Press the shortcut to keep on this PC...'
                      : 'Add a shortcut to keep on this PC'),
                ),
                dialogButton(recording ? 'Cancel' : 'Record',
                    icon: Icon(recording
                        ? Icons.stop_rounded
                        : Icons.fiber_manual_record),
                    onPressed: recording
                        ? () => setState(() => stopRecording())
                        : startRecording,
                    isOutline: true),
              ],
            ),
          ],
        ),
      ),
      actions: [
        dialogButton('Cancel',
            icon: const Icon(Icons.close_rounded),
            onPressed: onClose,
            isOutline: true),
        dialogButton('OK',
            icon: const Icon(Icons.done_rounded), onPressed: submit),
      ],
      onSubmit: recording ? null : submit,
      onCancel: recording ? null : onClose,
    );
  });
}
