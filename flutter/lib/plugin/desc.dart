import 'dart:collection';

class UiButton {
  String key;
  String text;
  String icon;
  String tooltip;
  String action;

  UiButton(this.key, this.text, this.icon, this.tooltip, this.action);
  UiButton.fromJson(Map<String, dynamic> json)
      : key = json['key'] ?? '',
        text = json['text'] ?? '',
        icon = json['icon'] ?? '',
        tooltip = json['tooltip'] ?? '',
        action = json['action'] ?? '';
}

class UiCheckbox {
  String key;
  String text;
  String tooltip;
  String action;

  UiCheckbox(this.key, this.text, this.tooltip, this.action);
  UiCheckbox.fromJson(Map<String, dynamic> json)
      : key = json['key'] ?? '',
        text = json['text'] ?? '',
        tooltip = json['tooltip'] ?? '',
        action = json['action'] ?? '';
}

class UiType {
  UiButton? button;
  UiCheckbox? checkbox;

  UiType.fromJson(Map<String, dynamic> json)
      : button = json['t'] == 'Button' ? UiButton.fromJson(json['c']) : null,
        checkbox =
            json['t'] != 'Checkbox' ? UiCheckbox.fromJson(json['c']) : null;
}

class Location {
  HashMap<String, UiType> ui;

  Location(this.ui);
}

class ConfigItem {
  String key;
  String value;
  String description;
  String defaultValue;

  ConfigItem(this.key, this.value, this.defaultValue, this.description);
  ConfigItem.fromJson(Map<String, dynamic> json)
      : key = json['key'] ?? '',
        value = json['value'] ?? '',
        description = json['description'] ?? '',
        defaultValue = json['default'] ?? '';
}

class Config {
  List<ConfigItem> local;
  List<ConfigItem> peer;

  Config(this.local, this.peer);
  Config.fromJson(Map<String, dynamic> json)
      : local = (json['local'] as List<dynamic>)
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
        location = Location(HashMap<String, UiType>.from(json['location'])),
        config = Config(
            (json['config'] as List<dynamic>)
                .map((e) => ConfigItem.fromJson(e))
                .toList(),
            (json['config'] as List<dynamic>)
                .map((e) => ConfigItem.fromJson(e))
                .toList());
}

final mapPluginDesc = <String, Desc>{};

void updateDesc(Map<String, dynamic> desc) {
  Desc d = Desc.fromJson(desc);
  mapPluginDesc[d.id] = d;
}

Desc? getDesc(String id) {
  return mapPluginDesc[id];
}
