import 'package:get/get.dart';
import 'package:managed_configurations/managed_configurations.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';

class ManagedAppConfigs {

  Future<void> loadConfigs() async {
    final managedConfig = ManagedConfigurations();
    final managedAppConfig = await managedConfig.getManagedConfigurations;
    setConfigs(managedAppConfig);
    managedConfig.mangedConfigurationsStream.listen((managedAppConfig) {
      // executed when managed config gets updated
      setConfigs(managedAppConfig);
    });
  }

  Future<void> setConfigs(Map<String, dynamic>? managedAppConfig) async {
    // disabeling scam warning --> not needed for managed devices in a MDM system
    bind.mainSetLocalOption(key: "show-scam-warning", value: "N");

    String idServer = "";
    String relayServer = "";
    String serverKey = "";

    managedAppConfig?.forEach((key, value) async {
      switch (key) {
        case kManagedAppKeyPassword:
          bind.mainSetPermanentPassword(password: value);
          break;
        case kManagedAppKeyIdServer:
          idServer = value;
          break;
        case kManagedAppKeyRelayServer:
          relayServer = value;
          break;
        case kManagedAppKeyServerKey:
          serverKey = value;
          break;
        case kManagedAppKeyId:
          bind.mainMdmSetId(newId: value);
          break;

      }
    });
    //start Service Right away
    await gFFI.serverModel.startService();
    bind.pluginSyncUi(syncTo: kAppTypeMain);
    bind.pluginListReload();
    //disable temporary password
    bind.mainSetOption(key: kOptionVerificationMethod, value: kUsePermanentPassword);
    gFFI.serverModel.updatePasswordModel();
    //gFFI.serverModel.toggleInput();
    setServerConfigs(idServer, relayServer, serverKey);

  }
  Future<void> setServerConfigs(String idServer,String relayServer,String serverKey) async {
    RxString idServerMsg = ''.obs;
    RxString relayServerMsg = ''.obs;
    RxString apiServerMsg = ''.obs;

    final errMsgs = [
      idServerMsg,
      relayServerMsg,
      apiServerMsg,
    ];
    await setServerConfig(
    null,
    errMsgs,
    ServerConfig(
        idServer: idServer.trim(),
        relayServer: relayServer.trim(),
        apiServer: "".trim(),
        key: serverKey.trim()));
  }
}
