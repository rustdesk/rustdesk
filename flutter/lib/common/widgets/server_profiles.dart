import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

const _kServerProfilesKey = 'server-profiles';

Map<String, ServerConfig> decodeServerProfiles(String raw) {
  final profiles = <String, ServerConfig>{};
  try {
    final json = jsonDecode(raw);
    if (json is Map) {
      json.forEach((name, v) {
        if (name is String && v is Map) {
          profiles[name] = ServerConfig(
              idServer: v['id']?.toString(),
              relayServer: v['relay']?.toString(),
              apiServer: v['api']?.toString(),
              key: v['key']?.toString());
        }
      });
    }
  } catch (_) {}
  return profiles;
}

String encodeServerProfiles(Map<String, ServerConfig> profiles) => jsonEncode({
      for (final e in profiles.entries)
        e.key: {
          'id': e.value.idServer,
          'relay': e.value.relayServer,
          'api': e.value.apiServer,
          'key': e.value.key,
        }
    });

Map<String, ServerConfig> _loadProfiles() =>
    decodeServerProfiles(bind.mainGetLocalOption(key: _kServerProfilesKey));

Future<void> _storeProfiles(Map<String, ServerConfig> profiles) =>
    bind.mainSetLocalOption(
        key: _kServerProfilesKey, value: encodeServerProfiles(profiles));

// The active profile is whichever saved profile equals the live options, so it
// stays correct however the options were changed.
String? _activeProfileName(Map<String, ServerConfig> profiles) {
  Map<String, dynamic> options = {};
  try {
    options = jsonDecode(bind.mainGetOptionsSync());
  } catch (_) {}
  final current = ServerConfig.fromOptions(options);
  for (final e in profiles.entries) {
    final p = e.value;
    if (p.idServer == current.idServer &&
        p.relayServer == current.relayServer &&
        p.apiServer == current.apiServer &&
        p.key == current.key) {
      return e.key;
    }
  }
  return null;
}

class ConnectProfilePicker extends StatefulWidget {
  const ConnectProfilePicker({Key? key}) : super(key: key);

  @override
  State<ConnectProfilePicker> createState() => _ConnectProfilePickerState();
}

class _ConnectProfilePickerState extends State<ConnectProfilePicker> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final profiles = _loadProfiles();
    if (profiles.isEmpty) return const SizedBox.shrink();
    final active = _activeProfileName(profiles);
    final textStyle = Theme.of(context).textTheme.bodyMedium;
    return LayoutBuilder(
        builder: (context, box) => PopupMenuButton<String>(
      tooltip: '',
      position: PopupMenuPosition.under,
      constraints: BoxConstraints.tightFor(width: box.maxWidth),
      itemBuilder: (_) => [
        for (final n in profiles.keys)
          PopupMenuItem(
            value: n,
            child: Row(children: [
              Expanded(child: Text(n)),
              if (n == active) const Icon(Icons.check, size: 18),
            ]),
          ),
      ],
      onSelected: (v) async {
        final config = profiles[v];
        if (config == null) return;
        await applyServerProfile(config);
        setState(() {});
      },
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 8),
        decoration: BoxDecoration(
          border: Border(
              bottom: BorderSide(color: Theme.of(context).dividerColor)),
        ),
        child: Row(children: [
          Expanded(
            child: Text(active ?? translate('Server profiles'),
                style: active == null
                    ? textStyle?.copyWith(color: Theme.of(context).hintColor)
                    : textStyle),
          ),
          const Icon(Icons.arrow_drop_down),
        ]),
      ),
    )).marginOnly(bottom: 8);
  }
}

// One mainSetOptions call instead of four mainSetOption calls: each of those
// sends the cached option map, which the 1s option sync can overwrite in between.
Future<void> applyServerProfile(ServerConfig config) async {
  Map<String, dynamic> options = {};
  try {
    options = jsonDecode(await bind.mainGetOptions());
  } catch (_) {}
  final oldApiServer = await bind.mainGetApiServer();
  void put(String key, String value) {
    if (value.isEmpty) {
      options.remove(key);
    } else {
      options[key] = value;
    }
  }

  put('custom-rendezvous-server', config.idServer);
  put('relay-server', config.relayServer);
  put('api-server', config.apiServer);
  put('key', config.key);
  await bind.mainSetOptions(json: jsonEncode(options));
  final newApiServer = await bind.mainGetApiServer();
  if (oldApiServer.isNotEmpty &&
      oldApiServer != newApiServer &&
      gFFI.userModel.isLogin) {
    gFFI.userModel.logOut(apiServer: oldApiServer);
  }
}

void showServerProfileEditor(OverlayDialogManager dialogManager, String name,
    ServerConfig config, void Function(String? oldName, String newName,
            ServerConfig config)
        onSave) {
  final ctrls = [
    TextEditingController(text: name),
    TextEditingController(text: config.idServer),
    TextEditingController(text: config.relayServer),
    TextEditingController(text: config.apiServer),
    TextEditingController(text: config.key),
  ];
  final labels = [
    translate('Profile name'),
    translate('ID Server'),
    translate('Relay Server'),
    translate('API Server'),
    'Key',
  ];
  dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate('Edit profile')),
      content: SizedBox(
        width: 400,
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          for (var i = 0; i < ctrls.length; i++)
            Padding(
              padding: EdgeInsets.only(top: i == 0 ? 8 : 0, bottom: 18),
              child: TextField(
                controller: ctrls[i],
                decoration: InputDecoration(labelText: labels[i]),
              ),
            ),
        ]),
      ),
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: () {
          final newName = ctrls[0].text.trim();
          if (newName.isEmpty) return;
          onSave(
              name,
              newName,
              ServerConfig(
                  idServer: ctrls[1].text,
                  relayServer: ctrls[2].text,
                  apiServer: ctrls[3].text,
                  key: ctrls[4].text));
          close();
        }),
      ],
      onCancel: close,
    );
  });
}

void showServerProfiles(OverlayDialogManager dialogManager) {
  final nameCtrl = TextEditingController();
  var profiles = _loadProfiles();

  dialogManager.show((setState, close, context) {
    Future<void> saveCurrent() async {
      final name = nameCtrl.text.trim();
      if (name.isEmpty) return;
      Map<String, dynamic> options = {};
      try {
        options = jsonDecode(await bind.mainGetOptions());
      } catch (_) {}
      profiles[name] = ServerConfig.fromOptions(options);
      await _storeProfiles(profiles);
      nameCtrl.clear();
      setState(() {});
    }

    Future<void> use(String name) async {
      final config = profiles[name];
      if (config == null) return;
      await applyServerProfile(config);
      showToast(translate('Successful'));
      close();
    }

    Future<void> remove(String name) async {
      profiles.remove(name);
      await _storeProfiles(profiles);
      setState(() {});
    }

    final activeName = _activeProfileName(profiles);
    return CustomAlertDialog(
      title: Text(translate('Server profiles')),
      content: SizedBox(
        width: 400,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final name in profiles.keys)
              Container(
                margin: const EdgeInsets.only(bottom: 8),
                padding: const EdgeInsets.only(left: 14, right: 6),
                decoration: BoxDecoration(
                  border: Border.all(color: Theme.of(context).dividerColor),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Row(children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(name,
                            style: const TextStyle(fontWeight: FontWeight.w600),
                            overflow: TextOverflow.ellipsis),
                        Text(
                            profiles[name]!.idServer.isEmpty
                                ? translate('Default')
                                : profiles[name]!.idServer,
                            style: Theme.of(context).textTheme.bodySmall,
                            overflow: TextOverflow.ellipsis),
                      ],
                    ),
                  ),
                  TextButton(
                    onPressed: name == activeName ? null : () => use(name),
                    child: Text(translate('Apply')),
                  ),
                  IconButton(
                    tooltip: translate('Edit'),
                    icon: const Icon(Icons.edit_outlined),
                    hoverColor: Colors.transparent,
                    onPressed: () => showServerProfileEditor(
                        dialogManager, name, profiles[name]!,
                        (oldName, newName, config) async {
                      if (newName != oldName && profiles.containsKey(newName)) {
                        showToast(translate('Failed'));
                        return;
                      }
                      profiles = {
                        for (final e in profiles.entries)
                          if (e.key == oldName) newName: config else e.key: e.value
                      };
                      await _storeProfiles(profiles);
                      setState(() {});
                    }),
                  ),
                  IconButton(
                    tooltip: translate('Delete'),
                    icon: const Icon(Icons.delete_outline),
                    hoverColor: Colors.transparent,
                    onPressed: () => remove(name),
                  ),
                ]),
              ),
            const SizedBox(height: 16),
            Row(children: [
              Expanded(
                child: TextField(
                  controller: nameCtrl,
                  decoration: InputDecoration(
                      hintText: translate('Profile name')),
                ),
              ),
              TextButton(
                onPressed: saveCurrent,
                child: Text(translate('Save current')),
              ),
            ]),
          ],
        ),
      ),
      actions: [dialogButton('Close', onPressed: close, isOutline: true)],
      onCancel: close,
    );
  });
}
