import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:url_launcher/url_launcher.dart';

class AudioOutput extends StatefulWidget {
  const AudioOutput({super.key});

  @override
  State<AudioOutput> createState() => _AudioOutputState();
}

class _AudioOutputState extends State<AudioOutput> with WidgetsBindingObserver {
  String _mode = 'normal';
  String? _error;
  String? _writeError;
  bool _hasCable = false;
  bool _busy = true;
  bool _refreshing = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _refresh();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _refresh();
    }
  }

  Future<void> _refresh() async {
    if (_refreshing) return;
    _refreshing = true;
    try {
      final data =
          jsonDecode(await bind.mainGetAudioOutputs()) as Map<String, dynamic>;
      if (!mounted) return;
      setState(() {
        _hasCable = data['available'] == true;
        _mode = (data['mode'] as String?) ?? 'normal';
        final workerError = (data['error'] as String?) ?? '';
        _error = _writeError ?? (workerError.isEmpty ? null : workerError);
      });
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    } finally {
      _refreshing = false;
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _setMode(String? mode) async {
    if (mode == null || (mode == 'remote' && !_hasCable)) return;
    setState(() {
      _busy = true;
      _error = null;
      _writeError = null;
    });
    try {
      await bind.mainSetOption(
        key: kOptionRemoteOnlyAudioMode,
        value: mode,
      );
    } catch (e) {
      if (mounted) {
        setState(() {
          _writeError = e.toString();
          _error = _writeError;
        });
      }
    }
    if (mounted && _writeError == null) {
      setState(() => _mode = mode);
    }
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    final canRemote = _hasCable;
    final textColor = disabledTextColor(context, true);
    final errorColor = Theme.of(context).colorScheme.error.withOpacity(0.75);
    return Focus(
      canRequestFocus: false,
      onFocusChange: (focused) {
        if (focused) _refresh();
      },
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        _audioRadio(
          context,
          value: 'normal',
          groupValue: _mode,
          label: 'Normal',
          onChanged: _busy ? null : _setMode,
        ),
        _audioRadio(
          context,
          value: 'remote',
          groupValue: _mode,
          label: 'Remote only',
          onChanged: _busy || !canRemote ? null : _setMode,
        ),
        Padding(
          padding: const EdgeInsets.only(left: 15, top: 4),
          child: Text(
            translate(_hasCable
                ? 'remote-only-audio-tip'
                : 'VB-CABLE is required for Remote only.'),
            style: TextStyle(fontSize: 15, color: textColor),
          ),
        ),
        if (!_hasCable && !_busy)
          Padding(
            padding: const EdgeInsets.only(left: 15),
            child: TextButton(
              style: TextButton.styleFrom(padding: EdgeInsets.zero),
              onPressed: () =>
                  launchUrl(Uri.parse('https://vb-audio.com/Cable/')),
              child: Text(translate('Get VB-CABLE')),
            ),
          ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(left: 15),
            child: SelectableText(
              _error!,
              style: TextStyle(fontSize: 15, color: errorColor),
            ),
          ),
      ]),
    );
  }
}

Widget _audioRadio<T>(
  BuildContext context, {
  required T value,
  required T groupValue,
  required String label,
  required ValueChanged<T>? onChanged,
}) {
  return GestureDetector(
    onTap: onChanged == null ? null : () => onChanged(value),
    child: Padding(
      padding: const EdgeInsets.only(left: 10),
      child: Row(
        children: [
          Radio<T>(
            value: value,
            groupValue: groupValue,
            onChanged: onChanged == null
                ? null
                : (next) {
                    if (next != null) onChanged(next);
                  },
          ),
          Expanded(
            child: Text(
              translate(label),
              style: TextStyle(
                fontSize: 15,
                color: disabledTextColor(context, onChanged != null),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}
