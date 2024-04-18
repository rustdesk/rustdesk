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

    // Determine if there is currently a proxy setting, and if so, use FFI to call the Rust HTTP method.
    final isProxy = await bind.mainGetProxyStatus();

    if (!isProxy) {
      return await _pollFultterHttp(url, method, headers: headers, body: body);
    }

    if (body is! String) {
      throw Exception('Unsupported HTTP body type');
    }

    await bind.mainHttpRequest(
        url: url,
        method: methodName.toLowerCase(),
        body: body,
        header: headersJson);

    var resJson = await _pollForResponse();
    return _parseHttpResponse(resJson);
  }

  Future<http.Response> _pollFultterHttp(
    String url,
    HttpMethod method, {
    Map<String, String>? headers,
    dynamic body,
  }) async {
    var response = http.Response('', 400); // 默认响应
    Uri uri = Uri.parse(url);

    switch (method) {
      case HttpMethod.get:
        response = await http.get(uri, headers: headers);
        break;
      case HttpMethod.post:
        response = await http.post(uri, headers: headers, body: body);
        break;
      case HttpMethod.put:
        response = await http.put(uri, headers: headers, body: body);
        break;
      case HttpMethod.delete:
        response = await http.delete(uri, headers: headers, body: body);
        break;
      default:
        throw Exception('Unsupported HTTP method');
    }

    return response;
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

Future<http.Response> get(String url, {Map<String, String>? headers}) async {
  return await HttpService().sendRequest(url, HttpMethod.get, headers: headers);
}

Future<http.Response> post(String url,
    {Map<String, String>? headers, Object? body, Encoding? encoding}) async {
  return await HttpService()
      .sendRequest(url, HttpMethod.post, body: body, headers: headers);
}

Future<http.Response> put(String url,
    {Map<String, String>? headers, Object? body, Encoding? encoding}) async {
  return await HttpService()
      .sendRequest(url, HttpMethod.put, body: body, headers: headers);
}

Future<http.Response> delete(String url,
    {Map<String, String>? headers, Object? body, Encoding? encoding}) async {
  return await HttpService()
      .sendRequest(url, HttpMethod.delete, body: body, headers: headers);
}
