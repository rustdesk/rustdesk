import 'package:flutter_hbb/desktop/controller/license_controller.dart';
import 'package:flutter_hbb/utils/license_service.dart';
import 'package:get/get.dart';
import 'package:get_storage/get_storage.dart';

class LicenseValidationController extends GetxController {
  // Observable variables
  var licenseKey = ''.obs;
  var isLoading = false.obs;
  var errorMessage = ''.obs;
  final storage = GetStorage();

  // Method to validate the license
  void validateLicense() async {
    if (licenseKey.value.trim().isEmpty) {
      errorMessage.value = 'Please enter your license key.';
      return;
    }

    // Start loading
    isLoading.value = true;
    errorMessage.value = '';

    try {
      final licenseController = Get.find<LicenseController>();
      // Perform the license validation (replace with your actual implementation)
      bool isValid = await LicenseService.validateLicense(
        licenseKey: licenseKey.value.trim(),
        deviceId: licenseController.deviceId!,
      );

      if (isValid) {
        print("validateLicense ${licenseKey.value}");
        // Store the license key locally
        storage.write('licenseKey', licenseKey.value.trim());
        // Update the license state in the LicenseController
        licenseController.isLicenseValid.value = true;
        licenseController.storedLicenseKey = licenseKey.value.trim();
        // Get.find<LicenseController>().isLicenseValid.value = true;
      } else {
        errorMessage.value = 'Invalid license key. Please try again.';
      }
    } catch (e) {
      errorMessage.value = 'An error occurred during validation.';
    } finally {
      isLoading.value = false;
    }
  }
}
