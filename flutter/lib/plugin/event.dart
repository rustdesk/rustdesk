import 'dart:convert';
import 'package:flutter/material.dart';

void handlePluginEvent(
  Map<String, dynamic> evt,
  String peer,
  Function(Map<String, dynamic> e) handleMsgBox,
) {
  Map<String, dynamic>? content;
  try {
    content = json.decode(evt['content']);
  } catch (e) {
    debugPrint(
        'Json decode plugin event content failed: $e, ${evt['content']}');
  }
  if (content?['t'] == 'MsgBox') {
    handleMsgBox(content?['c']);
  }
}
