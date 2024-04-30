import 'package:flutter/material.dart';
import './common.dart';
import './manager.dart';

final Map<String, LocationModel> _locationModels = {};
final Map<String, OptionModel> _optionModels = {};

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

  void add(List<UiType> uiList) {
    bool found = false;
    for (var ui in uiList) {
      for (int i = 0; i < this.uiList.length; i++) {
        if (this.uiList[i].key == ui.key) {
          this.uiList[i] = ui;
          found = true;
        }
      }
      if (!found) {
        this.uiList.add(ui);
      }
    }
    notifyListeners();
  }

  String? getOpt(String key) => opts.remove(key);

  bool get isEmpty => uiList.isEmpty;
}

class LocationModel with ChangeNotifier {
  final Map<PluginId, PluginModel> pluginModels = {};

  void add(PluginId id, List<UiType> uiList) {
    if (pluginModels[id] != null) {
      pluginModels[id]!.add(uiList);
    } else {
      var model = PluginModel();
      model.add(uiList);
      pluginModels[id] = model;
      notifyListeners();
    }
  }

  void clear() {
    pluginModels.clear();
    notifyListeners();
  }

  void remove(PluginId id) {
    pluginModels.remove(id);
    notifyListeners();
  }

  bool get isEmpty => pluginModels.isEmpty;
}

void addLocationUi(String location, PluginId id, List<UiType> uiList) {
  if (_locationModels[location] == null) {
    _locationModels[location] = LocationModel();
  }
  _locationModels[location]?.add(id, uiList);
}

LocationModel? getLocationModel(String location) => _locationModels[location];

PluginModel? getPluginModel(String location, PluginId id) =>
    _locationModels[location]?.pluginModels[id];

void clearPlugin(PluginId pluginId) {
  for (var element in _locationModels.values) {
    element.remove(pluginId);
  }
}

void clearLocations() {
  for (var element in _locationModels.values) {
    element.clear();
  }
}

OptionModel getOptionModel(
    String location, PluginId pluginId, String peer, String key) {
  final k = OptionModel.key(location, pluginId, peer, key);
  if (_optionModels[k] == null) {
    _optionModels[k] = OptionModel();
  }
  return _optionModels[k]!;
}

void updateOption(
    String location, PluginId id, String peer, String key, String value) {
  final k = OptionModel.key(location, id, peer, key);
  _optionModels[k]?.value = value;
}
