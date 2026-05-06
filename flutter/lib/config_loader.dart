import 'dart:convert';
import 'dart:io';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

class ConfigLoader {
  static Future<String> getConfigPath() async {
    final appDir = await getApplicationDocumentsDirectory();
    return '${appDir.path}/rustSettings/config.json';
  }

  static Future<Map<String, dynamic>?> loadConfigFromFile() async {
    try {
      final configPath = await getConfigPath();
      final file = File(configPath);
      
      if (await file.exists()) {
        final contents = await file.readAsString();
        final Map<String, dynamic> config = jsonDecode(contents);
        print('Config loaded from: $configPath');
        return config;
      } else {
        print('Config file not found at: $configPath');
        return null;
      }
    } catch (e) {
      print('Error loading config: $e');
      return null;
    }
  }

  static Future<String?> getRendezvousServer() async {
    final config = await loadConfigFromFile();
    return config?['rendezvous_server'];
  }

  static Future<String?> getKey() async {
    final config = await loadConfigFromFile();
    return config?['key'];
  }
}
