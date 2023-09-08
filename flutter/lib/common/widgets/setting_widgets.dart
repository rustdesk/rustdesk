import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

customImageQualityWidget(
    {required double initQuality,
    required double initFps,
    required Function(double) setQuality,
    required Function(double) setFps,
    required bool showFps}) {
  final qualityValue = initQuality.obs;
  final fpsValue = initFps.obs;

  final RxBool moreQualityChecked = RxBool(qualityValue.value > 100);
  final debouncerQuality = Debouncer<double>(
    Duration(milliseconds: 1000),
    onChanged: (double v) {
      setQuality(v);
    },
    initialValue: qualityValue.value,
  );
  final debouncerFps = Debouncer<double>(
    Duration(milliseconds: 1000),
    onChanged: (double v) {
      setFps(v);
    },
    initialValue: fpsValue.value,
  );

  onMoreChanged(bool? value) {
    if (value == null) return;
    moreQualityChecked.value = value;
    if (!value && qualityValue.value > 100) {
      qualityValue.value = 100;
    }
    debouncerQuality.value = qualityValue.value;
  }

  return Column(
    children: [
      Obx(() => Row(
            children: [
              Expanded(
                flex: 3,
                child: Slider(
                  value: qualityValue.value,
                  min: 10.0,
                  max: moreQualityChecked.value ? 2000 : 100,
                  divisions: moreQualityChecked.value ? 199 : 18,
                  onChanged: (double value) async {
                    qualityValue.value = value;
                    debouncerQuality.value = value;
                  },
                ),
              ),
              Expanded(
                  flex: 1,
                  child: Text(
                    '${qualityValue.value.round()}%',
                    style: const TextStyle(fontSize: 15),
                  )),
              Expanded(
                  flex: isMobile ? 2 : 1,
                  child: Text(
                    translate('Bitrate'),
                    style: const TextStyle(fontSize: 15),
                  )),
              // mobile doesn't have enough space
              if (!isMobile)
                Expanded(
                    flex: 1,
                    child: Row(
                      children: [
                        Checkbox(
                          value: moreQualityChecked.value,
                          onChanged: onMoreChanged,
                        ),
                        Expanded(
                          child: Text(translate('More')),
                        )
                      ],
                    ))
            ],
          )),
      if (isMobile)
        Obx(() => Row(
              children: [
                Expanded(
                  child: Align(
                    alignment: Alignment.centerRight,
                    child: Checkbox(
                      value: moreQualityChecked.value,
                      onChanged: onMoreChanged,
                    ),
                  ),
                ),
                Expanded(
                  child: Text(translate('More')),
                )
              ],
            )),
      if (showFps)
        Obx(() => Row(
              children: [
                Expanded(
                  flex: 3,
                  child: Slider(
                    value: fpsValue.value,
                    min: 5.0,
                    max: 120.0,
                    divisions: 23,
                    onChanged: (double value) async {
                      fpsValue.value = value;
                      debouncerFps.value = value;
                    },
                  ),
                ),
                Expanded(
                    flex: 1,
                    child: Text(
                      '${fpsValue.value.round()}',
                      style: const TextStyle(fontSize: 15),
                    )),
                Expanded(
                    flex: 2,
                    child: Text(
                      translate('FPS'),
                      style: const TextStyle(fontSize: 15),
                    ))
              ],
            )),
    ],
  );
}

customImageQualitySetting() {
  final qualityKey = 'custom_image_quality';
  final fpsKey = 'custom-fps';

  var initQuality =
      (double.tryParse(bind.mainGetUserDefaultOption(key: qualityKey)) ?? 50.0);
  if (initQuality < 10 || initQuality > 2000) {
    initQuality = 50;
  }
  var initFps =
      (double.tryParse(bind.mainGetUserDefaultOption(key: fpsKey)) ?? 30.0);
  if (initFps < 5 || initFps > 120) {
    initFps = 30;
  }

  return customImageQualityWidget(
      initQuality: initQuality,
      initFps: initFps,
      setQuality: (v) {
        bind.mainSetUserDefaultOption(key: qualityKey, value: v.toString());
      },
      setFps: (v) {
        bind.mainSetUserDefaultOption(key: fpsKey, value: v.toString());
      },
      showFps: true);
}

Future<bool> setServerConfig(
  List<TextEditingController> controllers,
  List<RxString> errMsgs,
  ServerConfig config,
) async {
  config.idServer = config.idServer.trim();
  config.relayServer = config.relayServer.trim();
  config.apiServer = config.apiServer.trim();
  config.key = config.key.trim();
  // id
  if (config.idServer.isNotEmpty) {
    errMsgs[0].value =
        translate(await bind.mainTestIfValidServer(server: config.idServer));
    if (errMsgs[0].isNotEmpty) {
      return false;
    }
  }
  // relay
  if (config.relayServer.isNotEmpty) {
    errMsgs[1].value =
        translate(await bind.mainTestIfValidServer(server: config.relayServer));
    if (errMsgs[1].isNotEmpty) {
      return false;
    }
  }
  // api
  if (config.apiServer.isNotEmpty) {
    if (!config.apiServer.startsWith('http://') &&
        !config.apiServer.startsWith('https://')) {
      errMsgs[2].value =
          '${translate("API Server")}: ${translate("invalid_http")}';
      return false;
    }
  }
  final oldApiServer = await bind.mainGetApiServer();

  // should set one by one
  await bind.mainSetOption(
      key: 'custom-rendezvous-server', value: config.idServer);
  await bind.mainSetOption(key: 'relay-server', value: config.relayServer);
  await bind.mainSetOption(key: 'api-server', value: config.apiServer);
  await bind.mainSetOption(key: 'key', value: config.key);

  final newApiServer = await bind.mainGetApiServer();
  if (oldApiServer.isNotEmpty &&
      oldApiServer != newApiServer &&
      gFFI.userModel.isLogin) {
    gFFI.userModel.logOut(apiServer: oldApiServer);
  }
  return true;
}

List<Widget> ServerConfigImportExportWidgets(
  List<TextEditingController> controllers,
  List<RxString> errMsgs,
) {
  import() {
    Clipboard.getData(Clipboard.kTextPlain).then((value) {
      final text = value?.text;
      if (text != null && text.isNotEmpty) {
        try {
          final sc = ServerConfig.decode(text);
          if (sc.idServer.isNotEmpty) {
            controllers[0].text = sc.idServer;
            controllers[1].text = sc.relayServer;
            controllers[2].text = sc.apiServer;
            controllers[3].text = sc.key;
            Future<bool> success = setServerConfig(controllers, errMsgs, sc);
            success.then((value) {
              if (value) {
                showToast(
                    translate('Import server configuration successfully'));
              } else {
                showToast(translate('Invalid server configuration'));
              }
            });
          } else {
            showToast(translate('Invalid server configuration'));
          }
        } catch (e) {
          showToast(translate('Invalid server configuration'));
        }
      } else {
        showToast(translate('Clipboard is empty'));
      }
    });
  }

  export() {
    final text = ServerConfig(
            idServer: controllers[0].text.trim(),
            relayServer: controllers[1].text.trim(),
            apiServer: controllers[2].text.trim(),
            key: controllers[3].text.trim())
        .encode();
    debugPrint("ServerConfig export: $text");
    Clipboard.setData(ClipboardData(text: text));
    showToast(translate('Export server configuration successfully'));
  }

  return [
    Tooltip(
      message: translate('Import Server Config'),
      child: IconButton(
          icon: Icon(Icons.paste, color: Colors.grey), onPressed: import),
    ),
    Tooltip(
        message: translate('Export Server Config'),
        child: IconButton(
            icon: Icon(Icons.copy, color: Colors.grey), onPressed: export))
  ];
}
