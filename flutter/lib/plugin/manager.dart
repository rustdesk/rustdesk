// The plugin manager is a singleton class that manages the plugins.
// 1. It merge metadata and the desc of plugins.

import 'dart:convert';
import 'dart:collection';
import 'package:flutter/material.dart';

const String kValueTrue = '1';
const String kValueFalse = '0';

class ConfigItem {
  String key;
  String description;
  String defaultValue;

  ConfigItem(this.key, this.defaultValue, this.description);
  ConfigItem.fromJson(Map<String, dynamic> json)
      : key = json['key'] ?? '',
        description = json['description'] ?? '',
        defaultValue = json['default'] ?? '';

  static String get trueValue => kValueTrue;
  static String get falseValue => kValueFalse;
  static bool isTrue(String value) => value == kValueTrue;
  static bool isFalse(String value) => value == kValueFalse;
}

class UiType {
  String key;
  String text;
  String tooltip;
  String action;

  UiType(this.key, this.text, this.tooltip, this.action);

  UiType.fromJson(Map<String, dynamic> json)
      : key = json['key'] ?? '',
        text = json['text'] ?? '',
        tooltip = json['tooltip'] ?? '',
        action = json['action'] ?? '';

  static UiType? create(Map<String, dynamic> json) {
    if (json['t'] == 'Button') {
      return UiButton.fromJson(json['c']);
    } else if (json['t'] == 'Checkbox') {
      return UiCheckbox.fromJson(json['c']);
    } else {
      return null;
    }
  }
}

class UiButton extends UiType {
  String icon;

  UiButton(
      {required String key,
      required String text,
      required this.icon,
      required String tooltip,
      required String action})
      : super(key, text, tooltip, action);

  UiButton.fromJson(Map<String, dynamic> json)
      : icon = json['icon'] ?? '',
        super.fromJson(json);
}

class UiCheckbox extends UiType {
  UiCheckbox(
      {required String key,
      required String text,
      required String tooltip,
      required String action})
      : super(key, text, tooltip, action);

  UiCheckbox.fromJson(Map<String, dynamic> json) : super.fromJson(json);
}

class Location {
  // location key:
  //  host|main|settings|plugin
  //  client|remote|toolbar|display
  HashMap<String, UiType> ui;

  Location(this.ui);
  Location.fromJson(Map<String, dynamic> json) : ui = HashMap() {
    (json['ui'] as Map<String, dynamic>).forEach((key, value) {
      var ui = UiType.create(value);
      if (ui != null) {
        this.ui[ui.key] = ui;
      }
    });
  }
}

class PublishInfo {
  PublishInfo({
    required this.lastReleased,
    required this.published,
  });

  final DateTime lastReleased;
  final DateTime published;
}

class Meta {
  Meta({
    required this.id,
    required this.name,
    required this.version,
    required this.description,
    required this.author,
    required this.home,
    required this.license,
    required this.publishInfo,
    required this.source,
  });

  final String id;
  final String name;
  final String version;
  final String description;
  final String author;
  final String home;
  final String license;
  final PublishInfo publishInfo;
  final String source;
}

class SourceInfo {
  String name; // 1. RustDesk github 2. Local
  String url;
  String description;

  SourceInfo({
    required this.name,
    required this.url,
    required this.description,
  });
}

class PluginInfo with ChangeNotifier {
  SourceInfo sourceInfo;
  Meta meta;
  String installedVersion; // It is empty if not installed.
  String failedMsg;
  String invalidReason; // It is empty if valid.

  PluginInfo({
    required this.sourceInfo,
    required this.meta,
    required this.installedVersion,
    required this.invalidReason,
    this.failedMsg = '',
  });

  bool get installed => installedVersion.isNotEmpty;
  bool get needUpdate => installed && installedVersion != meta.version;

  void setInstall(String msg) {
    if (msg == "finished") {
      msg = '';
    }
    failedMsg = msg;
    if (msg.isEmpty) {
      installedVersion = meta.version;
    }
    notifyListeners();
  }

  void setUninstall(String msg) {
    failedMsg = msg;
    if (msg.isEmpty) {
      installedVersion = '';
    }
    notifyListeners();
  }
}

class PluginManager with ChangeNotifier {
  String failedReason = ''; // The reason of failed to load plugins.
  final List<PluginInfo> _plugins = [];

  PluginManager._();
  static final PluginManager _instance = PluginManager._();
  static PluginManager get instance => _instance;

  List<PluginInfo> get plugins => _plugins;

  PluginInfo? getPlugin(String id) {
    for (var p in _plugins) {
      if (p.meta.id == id) {
        return p;
      }
    }
    return null;
  }

  void handleEvent(Map<String, dynamic> evt) {
    if (evt['plugin_list'] != null) {
      _handlePluginList(evt['plugin_list']);
    } else if (evt['plugin_install'] != null && evt['id'] != null) {
      _handlePluginInstall(evt['id'], evt['plugin_install']);
    } else if (evt['plugin_uninstall'] != null && evt['id'] != null) {
      _handlePluginUninstall(evt['id'], evt['plugin_uninstall']);
    } else {
      debugPrint('Failed to handle manager event: $evt');
    }
  }

  void _sortPlugins() {
    plugins.sort((a, b) {
      if (a.installed) {
        return -1;
      } else if (b.installed) {
        return 1;
      } else {
        return 0;
      }
    });
  }

  void _handlePluginList(String pluginList) {
    _plugins.clear();
    try {
      for (var p in json.decode(pluginList) as List<dynamic>) {
        final plugin = _getPluginFromEvent(p);
        if (plugin == null) {
          continue;
        }
        _plugins.add(plugin);
      }
    } catch (e) {
      debugPrint('Failed to decode $e,  plugin list \'$pluginList\'');
    }
    _sortPlugins();
    notifyListeners();
  }

  void _handlePluginInstall(String id, String msg) {
    debugPrint('Plugin \'$id\' install msg $msg');
    for (var i = 0; i < _plugins.length; i++) {
      if (_plugins[i].meta.id == id) {
        _plugins[i].setInstall(msg);
        _sortPlugins();
        notifyListeners();
        return;
      }
    }
  }

  void _handlePluginUninstall(String id, String msg) {
    debugPrint('Plugin \'$id\' uninstall msg $msg');
    for (var i = 0; i < _plugins.length; i++) {
      if (_plugins[i].meta.id == id) {
        _plugins[i].setUninstall(msg);
        _sortPlugins();
        notifyListeners();
        return;
      }
    }
  }

  PluginInfo? _getPluginFromEvent(Map<String, dynamic> evt) {
    final s = evt['source'];
    assert(s != null, 'Source is null');
    if (s == null) {
      return null;
    }
    final source = SourceInfo(
      name: s['name'],
      url: s['url'] ?? '',
      description: s['description'] ?? '',
    );

    final m = evt['meta'];
    assert(m != null, 'Meta is null');
    if (m == null) {
      return null;
    }

    late DateTime lastReleased;
    late DateTime published;
    try {
      lastReleased = DateTime.parse(
          m['publish_info']?['last_released'] ?? '1970-01-01T00+00:00');
    } catch (e) {
      lastReleased = DateTime.utc(1970);
    }
    try {
      published = DateTime.parse(
          m['publish_info']?['published'] ?? '1970-01-01T00+00:00');
    } catch (e) {
      published = DateTime.utc(1970);
    }

    final meta = Meta(
      id: m['id'],
      name: m['name'],
      version: m['version'],
      description: m['description'] ?? '',
      author: m['author'],
      home: m['home'] ?? '',
      license: m['license'] ?? '',
      source: m['source'] ?? '',
      publishInfo:
          PublishInfo(lastReleased: lastReleased, published: published),
    );
    return PluginInfo(
      sourceInfo: source,
      meta: meta,
      installedVersion: evt['installed_version'],
      invalidReason: evt['invalid_reason'] ?? '',
    );
  }
}

PluginManager get pluginManager => PluginManager.instance;
