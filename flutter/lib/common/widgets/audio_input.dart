import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';

const _kSystemSound = 'System Sound';

typedef AudioINputSetDevice = void Function(String device);
typedef AudioInputBuilder = Widget Function(
    List<String> devices, String currentDevice, AudioINputSetDevice setDevice);

class AudioInput extends StatelessWidget {
  final AudioInputBuilder builder;
  final bool isCm;
  final bool isVoiceCall;

  const AudioInput(
      {Key? key,
      required this.builder,
      required this.isCm,
      required this.isVoiceCall})
      : super(key: key);

  static String getDefault() {
    if (bind.mainAudioSupportLoopback()) {
      return translate(_kSystemSound);
    }
    return '';
  }

  static Future<String> getDefaultForHost() async {
    final audioHost = await bind.mainGetOption(key: 'audio-host');
    if (bind.mainAudioSupportLoopback() && audioHost.isEmpty) {
      return getDefault();
    }
    return '';
  }

  static Future<String> getAudioInput(bool isCm, bool isVoiceCall) {
    if (isVoiceCall) {
      return bind.getVoiceCallInputDevice(isCm: isCm);
    } else {
      return bind.mainGetOption(key: 'audio-input');
    }
  }

  static Future<String> getValue(bool isCm, bool isVoiceCall) async {
    String device = await getAudioInput(isCm, isVoiceCall);
    if (device.isNotEmpty) {
      return device;
    } else {
      return getDefaultForHost();
    }
  }

  static Future<void> setDevice(
      String device, bool isCm, bool isVoiceCall) async {
    if (device == await getDefaultForHost()) {
      device = '';
      if (!isVoiceCall) {
        await bind.mainSetOption(key: 'audio-host', value: '');
      }
    }
    if (isVoiceCall) {
      await bind.setVoiceCallInputDevice(isCm: isCm, device: device);
    } else {
      await bind.mainSetOption(key: 'audio-input', value: device);
    }
  }

  static Future<Map<String, Object>> getDevicesInfo(
      bool isCm, bool isVoiceCall) async {
    List<String> devices = (await bind.mainGetSoundInputs()).toList();
    final audioHost = await bind.mainGetOption(key: 'audio-host');
    if (bind.mainAudioSupportLoopback() && audioHost.isEmpty) {
      devices.insert(0, translate(_kSystemSound));
    }
    String current = await getValue(isCm, isVoiceCall);
    return {'devices': devices, 'current': current};
  }

  @override
  Widget build(BuildContext context) {
    return futureBuilder(
      future: getDevicesInfo(isCm, isVoiceCall),
      hasData: (data) {
        String currentDevice = data['current'];
        List<String> devices = data['devices'] as List<String>;
        if (devices.isEmpty) {
          return const Offstage();
        }
        return builder(devices, currentDevice, (devices) {
          setDevice(devices, isCm, isVoiceCall);
        });
      },
    );
  }
}

class AudioHost extends StatelessWidget {
  final Widget Function(List<String> hosts, String currentHost,
      Future<void> Function(String) setHost) builder;

  const AudioHost({Key? key, required this.builder}) : super(key: key);

  static Future<Map<String, Object>> getHostsInfo() async {
    final hosts = (await bind.mainGetAudioHosts()).toList();
    final configured = await bind.mainGetOption(key: 'audio-host');
    final current = configured.isEmpty ? 'wasapi' : configured;
    return {'hosts': hosts, 'current': current};
  }

  static Future<void> setAudioHost(String host) async {
    await bind.mainSetOption(
        key: 'audio-host', value: host == 'wasapi' ? '' : host);
    await bind.mainSetOption(key: 'audio-input', value: '');
  }

  @override
  Widget build(BuildContext context) {
    return futureBuilder(
      future: getHostsInfo(),
      hasData: (data) {
        final hosts = data['hosts'] as List<String>;
        if (hosts.length < 2) return const Offstage();
        return builder(hosts, data['current'] as String, setAudioHost);
      },
    );
  }
}
