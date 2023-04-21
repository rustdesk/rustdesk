import 'package:flutter/material.dart';
import './common.dart';
import './desc.dart';

final Map<String, LocationModel> locationModels = {};
final Map<String, KvModel> kvModels = {};

class KvModel with ChangeNotifier {
  final Map<String, String> kv = {};

  String? get(String key) => kv.remove(key);

  void set(String key, String value) {
    kv[key] = value;
    notifyListeners();
  }
}

class PluginModel with ChangeNotifier {
  final List<UiType> uiList = [];
  final Map<String, String> opts = {};

  void add(UiType ui) {
    uiList.add(ui);
    notifyListeners();
  }

  String? getOpt(String key) => opts.remove(key);

  bool get isEmpty => uiList.isEmpty;
}

class LocationModel with ChangeNotifier {
  final Map<PluginId, PluginModel> pluginModels = {};

  void add(PluginId id, UiType ui) {
    if (pluginModels[id] != null) {
      pluginModels[id]!.add(ui);
    } else {
      var model = PluginModel();
      model.add(ui);
      pluginModels[id] = model;
      notifyListeners();
    }
  }

  bool get isEmpty => pluginModels.isEmpty;
}

void addLocationUi(String location, PluginId id, UiType ui) {
  locationModels[location]?.add(id, ui);
}

LocationModel addLocation(String location) {
  if (locationModels[location] == null) {
    locationModels[location] = LocationModel();
  }
  return locationModels[location]!;
}

String makeKvModelInstance(String location, PluginId id, String peer) =>
    '$location|$id|$peer';

KvModel addKvModel(String location, PluginId pluginId, String peer) {
  final instance = makeKvModelInstance(location, pluginId, peer);
  if (kvModels[instance] == null) {
    kvModels[instance] = KvModel();
  }
  return kvModels[instance]!;
}

void updateOption(
    String location, PluginId id, String peer, String key, String value) {
  final instance = makeKvModelInstance(location, id, peer);
  kvModels[instance]?.set(key, value);
}
