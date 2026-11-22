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

class _AudioOutputState extends State<AudioOutput> {
  List<Map<String, dynamic>> _devices = [];
  String _selected = '';
  String _mode = 'normal';
  String? _error;
  bool _auditioning = false;
  int _connectedCount = 0;
  bool _canRestore = false;
  bool _busy = true;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  Future<void> _refresh() async {
    try {
      final data =
          jsonDecode(await bind.mainGetAudioOutputs()) as Map<String, dynamic>;
      if (!mounted) return;
      final devices = (data['devices'] as List? ?? const [])
          .map((item) => Map<String, dynamic>.from(item as Map))
          .toList();
      setState(() {
        _devices = devices;
        _mode = (data['mode'] as String?) ?? 'normal';
        _selected = (data['selected'] as String?) ?? '';
        _connectedCount = (data['connected_count'] as num?)?.toInt() ?? 0;
        _auditioning = data['auditioning'] == true;
        _canRestore = data['can_restore'] == true;
        final workerError = (data['error'] as String?) ?? '';
        _error = workerError.isEmpty ? null : workerError;
        if (!_devices.any((d) => d['id'] == _selected)) {
          _selected = _devices.isEmpty ? '' : _devices.first['id'] as String;
        }
      });
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _setMode(String? mode) async {
    if (mode == null || (mode == 'remote' && _selected.isEmpty)) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await bind.mainSetOption(
        key: kOptionRemoteOnlyAudioDevice,
        value: mode == 'normal' ? '' : _selected,
      );
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    }
    await _refresh();
  }

  Future<void> _selectDevice(String? value) async {
    if (value == null) return;
    setState(() {
      _selected = value;
      _error = null;
    });
    if (_mode == 'remote' && _connectedCount > 0) {
      setState(() => _busy = true);
      try {
        await bind.mainSetOption(
            key: kOptionRemoteOnlyAudioDevice, value: value);
      } catch (e) {
        if (mounted) setState(() => _error = e.toString());
      }
      await _refresh();
    }
  }

  Future<void> _toggleAudition() async {
    if (_selected.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await bind.mainSetOption(
        key: kOptionRemoteOnlyAudioPreview,
        value: _auditioning ? '' : _selected,
      );
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    }
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      RadioListTile<String>(
        title: Text(translate('Normal')),
        value: 'normal',
        groupValue: _mode,
        onChanged: _busy ? null : _setMode,
      ),
      RadioListTile<String>(
        title: Text(translate('Remote only')),
        value: 'remote',
        groupValue: _mode,
        onChanged: _busy || _selected.isEmpty ? null : _setMode,
      ),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          if (_devices.isNotEmpty)
            DropdownButton<String>(
              isExpanded: true,
              value: _selected.isEmpty ? null : _selected,
              items: _devices
                  .map((d) => DropdownMenuItem<String>(
                      value: d['id'] as String,
                      child: Text(d['name'] as String,
                          overflow: TextOverflow.ellipsis)))
                  .toList(),
              onChanged: _busy ? null : _selectDevice,
            ),
          if (_devices.isNotEmpty)
            TextButton(
              onPressed: _busy || _connectedCount > 0 ? null : _toggleAudition,
              child: Text(translate(
                  _auditioning ? 'Stop audio preview' : 'Audio preview')),
            ),
          Text(translate('remote-only-audio-tip')),
          if (_devices.isEmpty && !_busy)
            Text(translate('Install VB-CABLE to use remote-only audio.')),
          if (_canRestore && _connectedCount == 0)
            Text(translate('Audio output will be restored automatically.')),
          Wrap(children: [
            TextButton(
                onPressed: () =>
                    launchUrl(Uri.parse('https://vb-audio.com/Cable/')),
                child: Text(translate('Get VB-CABLE'))),
            TextButton(
                onPressed: _busy
                    ? null
                    : () {
                        setState(() {
                          _busy = true;
                          _error = null;
                        });
                        _refresh();
                      },
                child: Text(translate('Refresh'))),
          ]),
          if (_error != null)
            SelectableText(_error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error)),
        ]),
      ),
    ]);
  }
}
