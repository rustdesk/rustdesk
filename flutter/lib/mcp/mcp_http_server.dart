import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'mcp_dispatcher.dart';

const int kMcpDefaultPort = 21119;
const String kMcpEndpointPath = '/mcp';
const int _kMaxBodyBytes = 1 << 20;

String generateMcpToken() {
  final rng = Random.secure();
  final bytes = List<int>.generate(32, (_) => rng.nextInt(256));
  return base64Url.encode(bytes).replaceAll('=', '');
}

bool _constantTimeEquals(String a, String b) {
  final x = utf8.encode(a), y = utf8.encode(b);
  var diff = x.length ^ y.length;
  for (var i = 0; i < x.length && i < y.length; i++) {
    diff |= x[i] ^ y[i];
  }
  return diff == 0;
}

bool _isLoopbackHost(String host) =>
    host == 'localhost' ||
    host == '127.0.0.1' ||
    host == '[::1]' ||
    host == '::1';

// Defeats DNS rebinding from web pages.
bool isLoopbackHostHeader(String? hostHeader) {
  if (hostHeader == null || hostHeader.isEmpty) return false;
  var host = hostHeader;
  if (host.startsWith('[')) {
    final end = host.indexOf(']');
    if (end < 0) return false;
    host = host.substring(0, end + 1);
  } else {
    final colon = host.lastIndexOf(':');
    if (colon >= 0) host = host.substring(0, colon);
  }
  return _isLoopbackHost(host.toLowerCase());
}

bool isAllowedOrigin(String? origin) {
  if (origin == null) return true;
  final uri = Uri.tryParse(origin);
  return uri != null &&
      uri.hasAuthority &&
      _isLoopbackHost(uri.host.toLowerCase());
}

class McpHttpServer {
  final McpDispatcher dispatcher;
  final String token;
  HttpServer? _server;

  McpHttpServer(this.dispatcher, this.token);

  int? get port => _server?.port;
  bool get running => _server != null;

  Future<void> start({int port = kMcpDefaultPort}) async {
    if (_server != null) return;
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, port);
    _server = server;
    server.listen(handleRequest, onError: (_) {});
  }

  Future<void> stop() async {
    final server = _server;
    _server = null;
    await server?.close(force: true);
  }

  Future<void> handleRequest(HttpRequest req) async {
    final res = req.response;
    try {
      if (!isLoopbackHostHeader(req.headers.value(HttpHeaders.hostHeader)) ||
          !isAllowedOrigin(req.headers.value('origin'))) {
        return _finish(res, HttpStatus.forbidden, 'Forbidden');
      }
      if (req.uri.path != kMcpEndpointPath) {
        return _finish(res, HttpStatus.notFound, 'Not found');
      }
      final auth = req.headers.value(HttpHeaders.authorizationHeader) ?? '';
      const prefix = 'Bearer ';
      if (!auth.startsWith(prefix) ||
          !_constantTimeEquals(auth.substring(prefix.length), token)) {
        res.headers.set(HttpHeaders.wwwAuthenticateHeader, 'Bearer');
        return _finish(res, HttpStatus.unauthorized, 'Unauthorized');
      }
      if (req.method != 'POST') {
        res.headers.set(HttpHeaders.allowHeader, 'POST');
        return _finish(res, HttpStatus.methodNotAllowed, 'Method not allowed');
      }
      if (req.contentLength > _kMaxBodyBytes) {
        return _finish(res, HttpStatus.requestEntityTooLarge, 'Body too large');
      }
      final body = await _readBody(req);
      if (body == null) {
        return _finish(res, HttpStatus.requestEntityTooLarge, 'Body too large');
      }
      final reply = await dispatcher.handleRaw(body);
      if (reply == null) {
        res.statusCode = HttpStatus.accepted;
        await res.close();
        return;
      }
      res.statusCode = HttpStatus.ok;
      res.headers.contentType = ContentType.json;
      res.write(reply);
      await res.close();
    } catch (_) {
      try {
        await _finish(res, HttpStatus.internalServerError, 'Internal error');
      } catch (_) {}
    }
  }

  Future<String?> _readBody(HttpRequest req) async {
    final bytes = <int>[];
    await for (final chunk in req) {
      bytes.addAll(chunk);
      if (bytes.length > _kMaxBodyBytes) return null;
    }
    return utf8.decode(bytes, allowMalformed: true);
  }

  Future<void> _finish(HttpResponse res, int status, String text) async {
    res.statusCode = status;
    res.headers.contentType = ContentType.text;
    res.write(text);
    await res.close();
  }
}
