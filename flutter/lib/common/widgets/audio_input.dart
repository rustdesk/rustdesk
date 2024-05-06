import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';

typedef AudioINputSetDevice = void Function(String device);
typedef AudioInputBuilder = Widget Function(
    List<String> devices, String currentDevice, AudioINputSetDevice setDevice);

class AudioInput extends StatelessWidget {
  final AudioInputBuilder builder;

  const AudioInput({Key? key, required this.builder}) : super(key: key);

  static String getDefault() {
    if (isWindows) return translate('System Sound');
    return '';
  }

  static Future<String> getValue() async {
    String device = await bind.mainGetOption(key: 'audio-input');
    if (device.isNotEmpty) {
      return device;
    } else {
      return getDefault();
    }
  }

  static Future<void> setDevice(String device) async {
    if (device == getDefault()) device = '';
    await bind.mainSetOption(key: 'audio-input', value: device);
  }

  static Future<Map<String, Object>> getDevicesInfo() async {
    List<String> devices = (await bind.mainGetSoundInputs()).toList();
    if (isWindows) {
      devices.insert(0, translate('System Sound'));
    }
    String current = await getValue();
    return {'devices': devices, 'current': current};
  }

  @override
  Widget build(BuildContext context) {
    return futureBuilder(
      future: getDevicesInfo(),
      hasData: (data) {
        String currentDevice = data['current'];
        List<String> devices = data['devices'] as List<String>;
        if (devices.isEmpty) {
          return const Offstage();
        }
        return builder(devices, currentDevice, setDevice);
      },
    );
  }
}
