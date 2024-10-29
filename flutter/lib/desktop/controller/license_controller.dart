import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/utils/license_service.dart';
import 'package:get/get.dart';
import 'package:get_storage/get_storage.dart';

class LicenseController extends GetxController {
  static LicenseController get to => Get.find();
  var isCheckingActivation = true.obs;
  var isLicenseValid = false.obs;
  final storage = GetStorage();
  String? deviceId;
  String? storedLicenseKey;

  @override
  void onInit() {
    super.onInit();
    // checkLicense();
    _initDeviceId();
  }

  void _initDeviceId() async {
    deviceId = await _getDeviceId();
    checkLicense();
  }

  Future<String?> _getDeviceId() async {
    DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
    String? id;
    if (GetPlatform.isAndroid) {
      AndroidDeviceInfo androidInfo = await deviceInfo.androidInfo;
      id = androidInfo.id.hashCode.toString();
    } else if (GetPlatform.isIOS) {
      IosDeviceInfo iosInfo = await deviceInfo.iosInfo;
      id = iosInfo.identifierForVendor.hashCode.toString();
    } else if (isLinux) {
      LinuxDeviceInfo linuxInfo = await deviceInfo.linuxInfo;

      id = linuxInfo.machineId ?? linuxInfo.id;
    } else if (isWindows) {
      try {
        // request windows build number to fix overflow on win7
        windowsBuildNumber = getWindowsTargetBuildNumber();
        WindowsDeviceInfo winInfo = await deviceInfo.windowsInfo;
        id = winInfo.deviceId;
      } catch (e) {
        id = "unknown";
      }
    } else if (isMacOS) {
      MacOsDeviceInfo macOsInfo = await deviceInfo.macOsInfo;
      id = macOsInfo.systemGUID ?? '';
    }
    return id;
  }

  void checkLicense() async {
    try {
      // Start checking
      isCheckingActivation.value = true;
      // Perform the license check (replace with your actual implementation)
      //bool isValid = await LicenseService.checkLicense();
      // 2d43eef4-d5b9-42c3-b2c9-d529e3bd1902
      storedLicenseKey = storage.read('licenseKey');
      // storage.remove('licenseKey');
      //isLicenseValid.value = isValid;
      if (storedLicenseKey == null) {
        print("checkLicense can't find storedLicenseKey");
        isLicenseValid.value = false;
      } else {
        print("checkLicense find ${storedLicenseKey}");
        // Validate the license with the server
        bool isValid = await LicenseService.checkLicense(
          licenseKey: storedLicenseKey!,
          deviceId: deviceId!,
        );
        isLicenseValid.value = isValid;
      }
    } catch (e) {
      isLicenseValid.value = false;
    } finally {
      isCheckingActivation.value = false;
    }
  }
}
