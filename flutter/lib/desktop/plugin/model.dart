import 'package:flutter/material.dart';
import './common.dart';
import './desc.dart';

final Map<String, LocationModel> locationModels = {};

class PluginModel with ChangeNotifier {
  final List<UiType> uiList = [];

  void add(UiType ui) {
    uiList.add(ui);
    notifyListeners();
  }

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
