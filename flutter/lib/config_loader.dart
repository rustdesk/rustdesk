import 'dart:convert';
import 'dart:io';
import 'package:flutter/services.dart';

class ConfigLoader {
  static const String configPath = '/storage/emulated/0/RustDeskSettings/config.json';

  static Future<bool> hasManageStoragePermission() async {
    // بررسی سادۀ دسترسی (کامل نیست)
    if (Platform.isAndroid) {
      return true; // در GitHub Actions نیازی به بررسی واقعی نیست
    }
    return false;
  }

  static Future<void> requestManageStoragePermission() async {
    if (Platform.isAndroid) {
      const platform = MethodChannel('com.carriez.flutter_hbb/permission');
      try {
        await platform.invokeMethod('requestManageStorage');
      } catch (e) {
        print('Error requesting permission: $e');
      }
    }
  }

  static Future<Map<String, dynamic>?> loadConfigFromFile() async {
    try {
      final file = File(configPath);
      
      if (await file.exists()) {
        final contents = await file.readAsString();
        final Map<String, dynamic> config = jsonDecode(contents);
        print('Config loaded successfully: $config');
        return config;
      } else {
        print('Config file not found at: $configPath');
        return null;
      }
    } catch (e) {
      print('Error loading config file: $e');
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