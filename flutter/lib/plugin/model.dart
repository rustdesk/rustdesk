import 'package:flutter/material.dart';
import './common.dart';
import './desc.dart';

final Map<String, LocationModel> locationModels = {};
final Map<String, OptionModel> optionModels = {};

class OptionModel with ChangeNotifier {
  String? v;

  String? get value => v;
  set value(String? v) {
    this.v = v;
    notifyListeners();
  }

  static String key(String location, PluginId id, String peer, String k) =>
      '$location|$id|$peer|$k';
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

OptionModel addOptionModel(
    String location, PluginId pluginId, String peer, String key) {
  final k = OptionModel.key(location, pluginId, peer, key);
  if (optionModels[k] == null) {
    optionModels[k] = OptionModel();
  }
  return optionModels[k]!;
}

void updateOption(
    String location, PluginId id, String peer, String key, String value) {
  final k = OptionModel.key(location, id, peer, key);
  optionModels[k]?.value = value;
}
