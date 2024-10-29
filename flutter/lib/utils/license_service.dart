/*class LicenseService {
  static Future<bool> checkLicense({
    required String licenseKey,
    required String deviceId,
  }) async {
    // Simulate network delay
    await Future.delayed(Duration(seconds: 2));
    // Implement your license checking logic here
    // For example, make a network request to validate the license
    // Return true if the license is valid, false otherwise
    if (licenseKey == 'VALID_LICENSE_KEY') {
      return true;
    } else {
      return false;
    }
  }

  static Future<bool> validateLicense({
    required String licenseKey,
    required String deviceId,
  }) async {
    // Simulate network delay
    await Future.delayed(Duration(seconds: 2));

    // Mock validation logic
    if (licenseKey == 'VALID_LICENSE_KEY') {
      return true;
    } else {
      return false;
    }
  }
}
*/

import 'dart:convert';
import 'package:http/http.dart' as http;

class LicenseService {
  static const String apiUrl =
      'https://rustdesk-license-backend-cwfkahgjctbdexdr.canadacentral-01.azurewebsites.net';

  static Future<bool> validateLicense({
    required String licenseKey,
    required String deviceId,
  }) async {
    final response = await http.post(
      Uri.parse('$apiUrl/validate_license'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'licenseKey': licenseKey,
        'deviceId': deviceId,
      }),
    );

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body);
      return data['isValid'] == true;
    } else {
      throw Exception('Failed to validate license');
    }
  }

  static Future<bool> checkLicense({
    required String licenseKey,
    required String deviceId,
  }) async {
    final response = await http.post(
      Uri.parse('$apiUrl/check_license'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'licenseKey': licenseKey,
        'deviceId': deviceId,
      }),
    );

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body);
      print("checkLicense:: $data['isValid']");
      return data['isValid'] == true;
    } else {
      return false;
    }
  }
}
