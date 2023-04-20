import 'package:flutter/material.dart';

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
}
