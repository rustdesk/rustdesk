import 'package:flutter/material.dart';
import './common.dart';
import './desc.dart';

// ui location
// host|main|settings|display|others
// client|remote|toolbar|display

final Map<PluginId, Map<String, LocationModel>> locationModels = {};

class LocationModel with ChangeNotifier {
  final List<UiType> uiList = [];

  void add(UiType ui) {
    uiList.add(ui);
    notifyListeners();
  }

  bool get isEmpty => uiList.isEmpty;
}

void addLocation(PluginId id, String location, UiType ui) {
  if (!locationModels.containsKey(id)) {
    locationModels[id] = {};
  }
  if (!locationModels[id]!.containsKey(location)) {
    locationModels[id]![location] = LocationModel();
  }
  locationModels[id]![location]!.add(ui);
}
