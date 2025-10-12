import 'package:flutter/material.dart';

class PluginUiManager {
  PluginUiManager._();

  static PluginUiManager instance = PluginUiManager._();

  Map<String, Widget> entries = <String, Widget>{};

  void registerEntry(String key, Widget widget) {
    entries[key] = widget;
  }

  void unregisterEntry(String key) {
    entries.remove(key);
  }
}