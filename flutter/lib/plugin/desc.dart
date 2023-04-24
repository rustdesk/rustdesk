import 'dart:convert';
import 'dart:collection';
import 'package:flutter/foundation.dart';

import './common.dart';

const String kValueTrue = '1';
const String kValueFalse = '0';

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

class Config {
  List<ConfigItem> shared;
  List<ConfigItem> peer;

  Config(this.shared, this.peer);
  Config.fromJson(Map<String, dynamic> json)
      : shared = (json['shared'] as List<dynamic>)
            .map((e) => ConfigItem.fromJson(e))
            .toList(),
        peer = (json['peer'] as List<dynamic>)
            .map((e) => ConfigItem.fromJson(e))
            .toList();
}

class Desc {
  String id;
  String name;
  String version;
  String description;
  String author;
  String home;
  String license;
  String published;
  String released;
  String github;
  Location location;
  Config config;

  Desc(
      this.id,
      this.name,
      this.version,
      this.description,
      this.author,
      this.home,
      this.license,
      this.published,
      this.released,
      this.github,
      this.location,
      this.config);

  Desc.fromJson(Map<String, dynamic> json)
      : id = json['id'] ?? '',
        name = json['name'] ?? '',
        version = json['version'] ?? '',
        description = json['description'] ?? '',
        author = json['author'] ?? '',
        home = json['home'] ?? '',
        license = json['license'] ?? '',
        published = json['published'] ?? '',
        released = json['released'] ?? '',
        github = json['github'] ?? '',
        location = Location.fromJson(json['location']),
        config = Config.fromJson(json['config']);
}

class DescModel with ChangeNotifier {
  final data = <PluginId, Desc>{};

  DescModel._();

  void _updateDesc(Map<String, dynamic> desc) {
    try {
      Desc d = Desc.fromJson(json.decode(desc['desc']));
      data[d.id] = d;
      notifyListeners();
    } catch (e) {
      debugPrint('DescModel json.decode fail(): $e');
    }
  }

  Desc? _getDesc(String id) {
    return data[id];
  }

  Map<PluginId, Desc> get all => data;

  static final DescModel _instance = DescModel._();
  static DescModel get instance => _instance;
}

void updateDesc(Map<String, dynamic> desc) =>
    DescModel.instance._updateDesc(desc);
Desc? getDesc(String id) => DescModel.instance._getDesc(id);
