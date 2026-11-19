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
  String _active = '';
  String? _error;
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
      setState(() {
        _devices = (data['devices'] as List).cast<Map<String, dynamic>>();
        _active = data['selected'] as String;
        _canRestore = data['can_restore'] as bool;
        if (!_devices.any((d) => d['id'] == _selected)) {
          _selected = _devices.any((d) => d['id'] == _active)
              ? _active
              : (_devices.isEmpty ? '' : _devices.first['id'] as String);
        }
      });
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _change(String? mode) async {
    if (mode == null) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await bind.mainSetOption(
          key: kOptionRemoteOnlyAudioDevice,
          value: mode == 'normal' ? '' : _selected);
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    }
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    final mode =
        _active.isNotEmpty ? 'remote' : (_canRestore ? 'pending' : 'normal');
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      RadioListTile<String>(
        title: Text(translate('Normal')),
        value: 'normal',
        groupValue: mode,
        onChanged: _busy ? null : _change,
      ),
      RadioListTile<String>(
        title: Text(translate('Remote only')),
        value: 'remote',
        groupValue: mode,
        onChanged: _busy || _selected.isEmpty ? null : _change,
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
              onChanged: _busy || mode != 'normal'
                  ? null
                  : (value) => setState(() => _selected = value ?? ''),
            ),
          Text(translate('remote-only-audio-tip')),
          if (_devices.isEmpty && !_busy)
            Text(translate('Install VB-CABLE to use remote-only audio.')),
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
