import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/controller/license_validation_controller.dart';
import 'package:get/get.dart';

class LicenseValidationWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final LicenseValidationController controller =
        Get.put(LicenseValidationController());

    return Scaffold(
      appBar: AppBar(
        title: Text('License Validation'),
        centerTitle: true,
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16.0),
        child: Obx(() {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Enter your license key below to activate the application.',
                style: TextStyle(fontSize: 16),
              ),
              SizedBox(height: 20),
              TextField(
                onChanged: (value) => controller.licenseKey.value = value,
                decoration: InputDecoration(
                  labelText: 'License Key',
                  border: OutlineInputBorder(),
                  errorText: controller.errorMessage.value.isNotEmpty
                      ? controller.errorMessage.value
                      : null,
                ),
              ),
              SizedBox(height: 20),
              Center(
                child: controller.isLoading.value
                    ? CircularProgressIndicator()
                    : SizedBox(
                        width: double.infinity,
                        child: ElevatedButton(
                          onPressed: controller.validateLicense,
                          child: Text('Activate'),
                          style: ElevatedButton.styleFrom(
                            padding: EdgeInsets.symmetric(
                                horizontal: 40, vertical: 15),
                            textStyle: TextStyle(fontSize: 16),
                          ),
                        )),
              ),
            ],
          );
        }),
      ),
    );
  }
}
