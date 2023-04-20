import 'package:flutter/material.dart';
import './desc.dart';
import './model.dart';

final Map<String, PluginWidget> pluginWidgets = {};

class PluginWidget {
  final String id;
  final String name;
  final String location;
  final Widget widget;

  PluginWidget({
    required this.id,
    required this.name,
    required this.location,
    required this.widget,
  });

  // static Widget createButton(UiButton btn) {}

  // static Widget createCheckbox(UiCheckbox chk) {}

  // // ui location
  // // host|main|settings|display|others
  // // client|remote|toolbar|display
  // static Widget? create(String id, String locatin, UiType ui) {
  //   if (ui.button != null) {
  //     return createButton(ui.button!);
  //   } else if (ui.checkbox != null) {
  //     return createCheckbox(ui.checkbox!);
  //   } else {
  //     return null;
  //   }
  // }
}

void handleReloading(Map<String, dynamic> evt, String peer) {
  if (evt['id'] == null || evt['location'] == null) {
    return;
  }
  final ui = UiType.fromJson(evt);
  if (!ui.isValid) {
    return;
  }
  addLocation(evt['id']!, evt['location']!, ui);
}
