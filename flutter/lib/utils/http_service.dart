import 'dart:convert';
import 'package:http/http.dart' as http;

import '../models/platform_model.dart';

enum HttpMethod { get, post, put, delete }

class HttpService {
  Future<http.Response> sendRequest(
      String url,
      HttpMethod method, {
        Map<String, String>? headers,
        dynamic body,
      }) async {
    headers ??= {'Content-Type': 'application/json'};
    String headersJson = jsonEncode(headers);
    String methodName = method.toString().split('.').last;

    await bind.mainHttpRequest(url: url, method: methodName.toLowerCase(), body: body, header: headersJson);

    var resJson = await _pollForResponse();
    return _parseHttpResponse(resJson);
  }

  Future<String> _pollForResponse() async {
    String responseJson = await bind.mainGetAsyncStatus();
    while (responseJson == " ") {
      await Future.delayed(const Duration(milliseconds: 100));
      responseJson = await bind.mainGetAsyncStatus();
    }
    return responseJson;
  }

  http.Response _parseHttpResponse(String responseJson) {
    try {
      var parsedJson = jsonDecode(responseJson);
      String body = parsedJson['body'];
      Map<String, String> headers = {};
      for (var key in parsedJson['headers'].keys) {
        headers[key] = parsedJson['headers'][key];
      }
      int statusCode = parsedJson['status_code'];
      return http.Response(body, statusCode, headers: headers);
    } catch (e) {
      throw Exception('Failed to parse response: $e');
    }
  }
}