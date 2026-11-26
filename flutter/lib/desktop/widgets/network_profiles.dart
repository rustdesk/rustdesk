import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/native/win32.dart'
    if (dart.library.html) 'package:flutter_hbb/web/win32.dart';

const _storageKey = 'network-profiles';
const _activeKey = 'active-network-profile';
const _optionKeys = [
  'custom-rendezvous-server',
  'relay-server',
  'api-server',
  'key',
  kOptionAllowWebSocket,
  kOptionAllowInsecureTLSFallback,
  kOptionDisableUdp,
];
const _proxyKey = 'proxy-url';
final networkProfileChanges = ValueNotifier(0);

bool get networkProfilesAvailable =>
    isWindows &&
    !bind.isDisableSettings() &&
    [
      kOptionHideNetworkSetting,
      kOptionHideServerSetting,
      kOptionHideProxySetting,
      kOptionHideWebSocketSetting,
    ].every((key) => bind.mainGetBuildinOption(key: key) != 'Y');

class _NetworkProfile {
  const _NetworkProfile(this.options, this.proxy);

  factory _NetworkProfile.fromJson(Map<String, dynamic> json) {
    final options = Map<String, String>.from(json['options'] as Map);
    final proxy = List<String>.from(json['proxy'] as List);
    if (!_optionKeys.every(options.containsKey) || proxy.length != 3) {
      throw const FormatException('Invalid network profile');
    }
    return _NetworkProfile(
      {for (final key in _optionKeys) key: options[key]!},
      proxy,
    );
  }

  static Future<_NetworkProfile> read() async {
    final socks = await bind.mainGetSocks();
    return _NetworkProfile({
      for (final key in _optionKeys) key: bind.mainGetOptionSync(key: key),
    }, socks.length == 3 && socks.first.isNotEmpty ? socks : ['', '', '']);
  }

  final Map<String, String> options;
  final List<String> proxy;

  Map<String, dynamic> toJson() => {'options': options, 'proxy': proxy};

  bool matches(_NetworkProfile other) =>
      mapEquals(options, other.options) && listEquals(proxy, other.proxy);

  Future<void> write(_NetworkProfile previous) async {
    for (final key in _optionKeys) {
      if (options[key] != previous.options[key]) {
        await bind.mainSetOption(key: key, value: options[key]!);
      }
    }
    if (!listEquals(proxy, previous.proxy)) {
      await bind.mainSetSocks(
        proxy: proxy[0],
        username: proxy[1],
        password: proxy[2],
      );
    }
    if (!matches(await read())) {
      throw StateError('Network profile settings did not match');
    }
  }
}

Map<String, _NetworkProfile> _loadProfiles() {
  final raw = bind.mainGetLocalOption(key: _storageKey);
  if (raw.isEmpty) return {};
  final json = jsonDecode(transformWindowsUserData(raw, protect: false)) as Map;
  return json.map((key, value) => MapEntry(
      key as String, _NetworkProfile.fromJson(value as Map<String, dynamic>)));
}

Future<void> _storeProfiles(Map<String, _NetworkProfile> profiles) async {
  final encoded = profiles.isEmpty
      ? ''
      : transformWindowsUserData(jsonEncode(profiles), protect: true);
  await bind.mainSetLocalOption(key: _storageKey, value: encoded);
  if (bind.mainGetLocalOption(key: _storageKey) != encoded) {
    throw StateError('Network profiles did not match');
  }
}

Future<void> saveNetworkProfile(BuildContext context) async {
  try {
    final profiles = _loadProfiles();
    final name = await showDialog<String>(
      context: context,
      builder: (context) {
        var name = '';
        return StatefulBuilder(
          builder: (context, setState) {
            final exists = profiles.containsKey(name);
            void submit() {
              if (name.isNotEmpty) Navigator.of(context).pop(name);
            }

            return AlertDialog(
              title: Text(translate('Save network profile')),
              content: TextField(
                autofocus: true,
                maxLength: 80,
                decoration: InputDecoration(
                  labelText: translate('Profile name'),
                  helperText:
                      exists ? translate('Profile will be replaced') : null,
                ),
                onChanged: (value) => setState(() => name = value.trim()),
                onSubmitted: (_) => submit(),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: Text(translate('Cancel')),
                ),
                TextButton(
                  onPressed: name.isEmpty ? null : submit,
                  child: Text(translate(exists ? 'Replace' : 'Save')),
                ),
              ],
            );
          },
        );
      },
    );
    if (name == null) return;
    final profile = await _NetworkProfile.read();
    final updated = _loadProfiles()..[name] = profile;
    await _storeProfiles(updated);
    await bind.mainSetLocalOption(key: _activeKey, value: name);
    notifyNetworkProfileChange();
    showToast(translate('Network profile saved'));
  } catch (_) {
    showToast(translate('Could not save network profile'));
  }
}

void notifyNetworkProfileChange() => networkProfileChanges.value++;

class NetworkProfileButton extends StatefulWidget {
  const NetworkProfileButton({super.key});

  @override
  State<NetworkProfileButton> createState() => _NetworkProfileButtonState();
}

class _NetworkProfileButtonState extends State<NetworkProfileButton> {
  late Future<String?> _activeName = _findActiveName();
  bool _busy = false;

  Future<String?> _findActiveName() async {
    final profiles = _loadProfiles();
    if (profiles.isEmpty) return null;
    final current = await _NetworkProfile.read();
    final preferred = bind.mainGetLocalOption(key: _activeKey);
    if (profiles[preferred]?.matches(current) == true) return preferred;
    for (final entry in profiles.entries) {
      if (entry.value.matches(current)) return entry.key;
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    networkProfileChanges.addListener(_refresh);
  }

  @override
  void dispose() {
    networkProfileChanges.removeListener(_refresh);
    super.dispose();
  }

  void _refresh() => setState(() => _activeName = _findActiveName());

  Future<void> _deleteProfile(String name) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(translate('Confirm Delete')),
        content: Text(name),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(translate('Cancel')),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(translate('Delete')),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      final profiles = _loadProfiles();
      if (profiles.remove(name) == null) return;
      await _storeProfiles(profiles);
      if (bind.mainGetLocalOption(key: _activeKey) == name) {
        await bind.mainSetLocalOption(key: _activeKey, value: '');
      }
    } catch (_) {
      showToast(translate('Could not delete network profile'));
    } finally {
      notifyNetworkProfileChange();
    }
  }

  Future<void> _selectProfile() async {
    setState(() => _busy = true);
    try {
      final profiles = _loadProfiles();
      final active = await _findActiveName();
      if (!mounted) return;
      final selection = await showDialog<({String name, bool delete})>(
        context: context,
        builder: (context) => SimpleDialog(
          title: Text(translate('Network profiles')),
          children: [
            if (profiles.isEmpty)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 24),
                child: Text(translate('Save a profile in Network settings.')),
              ),
            for (final name in profiles.keys)
              ListTile(
                leading:
                    Icon(name == active ? Icons.check : Icons.lan_outlined),
                title: Text(name),
                onTap: () =>
                    Navigator.of(context).pop((name: name, delete: false)),
                trailing: IconButton(
                  tooltip: translate('Delete'),
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () =>
                      Navigator.of(context).pop((name: name, delete: true)),
                ),
              ),
          ],
        ),
      );
      if (selection == null || !mounted) return;
      final name = selection.name;
      if (selection.delete) {
        await _deleteProfile(name);
        return;
      }
      final target = profiles[name]!;
      if (target.matches(await _NetworkProfile.read())) {
        await bind.mainSetLocalOption(key: _activeKey, value: name);
        notifyNetworkProfileChange();
        return;
      }
      if (bind.mainIsInstalled()) {
        final pin = bind.mainGetUnlockPin();
        if (pin.isNotEmpty && !isUnlockPinDisabled()) {
          checkUnlockPinDialog(pin, () => _applyWithFeedback(name, target));
          return;
        }
        if (!await callMainCheckSuperUserPermission()) return;
      }
      await _applyWithFeedback(name, target);
    } catch (_) {
      showToast(translate('Could not load network profiles'));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _applyWithFeedback(String name, _NetworkProfile target) async {
    if (!mounted) return;
    setState(() => _busy = true);
    try {
      final current = await _NetworkProfile.read();
      if (_optionKeys.any((key) =>
              target.options[key] != current.options[key] &&
              isOptionFixed(key)) ||
          (!listEquals(target.proxy, current.proxy) &&
              isOptionFixed(_proxyKey))) {
        showToast(translate('Network profile changes locked settings'));
        return;
      }
      final oldApi = await bind.mainGetApiServer();
      try {
        await target.write(current);
      } catch (_) {
        await current.write(await _NetworkProfile.read());
        rethrow;
      }
      await bind.mainSetLocalOption(key: _activeKey, value: name);
      if (oldApi.isNotEmpty &&
          oldApi != await bind.mainGetApiServer() &&
          gFFI.userModel.isLogin) {
        await gFFI.userModel.logOut(apiServer: oldApi);
      }
      showToast(translate('Network profile applied'));
    } catch (_) {
      showToast(translate('Could not apply network profile'));
    } finally {
      notifyNetworkProfileChange();
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<String?>(
      future: _activeName,
      builder: (context, snapshot) => OutlinedButton(
        style: OutlinedButton.styleFrom(
          padding: const EdgeInsets.symmetric(horizontal: 8),
        ),
        onPressed: _busy ? null : _selectProfile,
        child: Row(
          children: [
            const Icon(Icons.lan_outlined, size: 18),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                snapshot.data ?? translate('Network profiles'),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            const Icon(Icons.expand_more, size: 18),
          ],
        ),
      ),
    );
  }
}
