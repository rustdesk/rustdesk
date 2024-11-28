import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import '../models/platform_model.dart';
export 'package:http/http.dart' show Response;

enum HttpMethod { get, post, put, delete }

class HttpService {
  Future<http.Response> sendRequest(
    Uri url,
    HttpMethod method, {
    Map<String, String>? headers,
    dynamic body,
  }) async {
    headers ??= {'Content-Type': 'application/json'};

    // Determine if there is currently a proxy setting, and if so, use FFI to call the Rust HTTP method.
    final isProxy = await bind.mainGetProxyStatus();

    if (!isProxy) {
      return await _pollFultterHttp(url, method, headers: headers, body: body);
    }

    String headersJson = jsonEncode(headers);
    String methodName = method.toString().split('.').last;
    await bind.mainHttpRequest(
        url: url.toString(),
        method: methodName.toLowerCase(),
        body: body,
        header: headersJson);

    var resJson = await _pollForResponse(url.toString());
    return _parseHttpResponse(resJson);
  }

  http.Client sslClient()  {  
    // HttpClient ioClient = HttpClient();
    // SecurityContext sc = SecurityContext();
    // File crt=File('./cert.pem');
    // bool dir_bool=crt.exists() as bool; //返回真假
    
    // if(dir_bool){
    //   //file为证书路径
    //   sc.setTrustedCertificates("./cert.pem");
    //   //创建一个HttpClient
    //   ioClient = HttpClient(context: sc);
    // }
    //   // var ioClient = HttpClient()
    //   // ..badCertificateCallback = (X509Certificate cert, String host, int port) => true;
    
    // http.Client _client = IOClient(ioClient);
    return new HttpClient()
    ..badCertificateCallback =(X509Certificate cert, String host, int port) => true;
  }


  Future<http.Response> _pollFultterHttp(
    Uri url,
    HttpMethod method, {
    Map<String, String>? headers,
    dynamic body,
  }) async {
    var response = http.Response('', 400);

    switch (method) {
      case HttpMethod.get:
        response = await sslClient().get(url, headers: headers);
        break;
      case HttpMethod.post:
        response = await sslClient().post(url, headers: headers, body: body);
        break;
      case HttpMethod.put:
        response = await sslClient().put(url, headers: headers, body: body);
        break;
      case HttpMethod.delete:
        response = await sslClient().delete(url, headers: headers, body: body);
        break;
      default:
        throw Exception('Unsupported HTTP method');
    }

    return response;
  }

  Future<String> _pollForResponse(String url) async {
    String? responseJson = " ";
    while (responseJson == " ") {
      responseJson = await bind.mainGetHttpStatus(url: url);
      if (responseJson == null) {
        throw Exception('The HTTP request failed');
      }
      if (responseJson == " ") {
        await Future.delayed(const Duration(milliseconds: 100));
      }
    }
    return responseJson!;
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

Future<http.Response> get(Uri url, {Map<String, String>? headers}) async {
  return await HttpService().sendRequest(url, HttpMethod.get, headers: headers);
}

Future<http.Response> post(Uri url,
    {Map<String, String>? headers, Object? body, Encoding? encoding}) async {
  return await HttpService()
      .sendRequest(url, HttpMethod.post, body: body, headers: headers);
}

Future<http.Response> put(Uri url,
    {Map<String, String>? headers, Object? body, Encoding? encoding}) async {
  return await HttpService()
      .sendRequest(url, HttpMethod.put, body: body, headers: headers);
}

Future<http.Response> delete(Uri url,
    {Map<String, String>? headers, Object? body, Encoding? encoding}) async {
  return await HttpService()
      .sendRequest(url, HttpMethod.delete, body: body, headers: headers);
}
