import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:http/http.dart' as http;

class AbModel with ChangeNotifier {
  var abLoading = false;
  var abError = "";
  var tags = [];
  var peers = [];

  WeakReference<FFI> parent;

  AbModel(this.parent);

  FFI? get _ffi => parent.target;

  Future<dynamic> getAb() async {
    abLoading = true;
    notifyListeners();
    // request
    final api = "${await getApiServer()}/api/ab/get";
    debugPrint("request $api with post ${await _getHeaders()}");
    final resp = await http.post(Uri.parse(api), headers: await _getHeaders());
    abLoading = false;
    Map<String, dynamic> json = jsonDecode(resp.body);
    if (json.containsKey('error')) {
      abError = json['error'];
    } else if (json.containsKey('data')) {
      // {"tags":["aaa","bbb"],
      // "peers":[{"id":"aa1234","username":"selfd",
      // "hostname":"PC","platform":"Windows","tags":["aaa"]}]}
      final data = jsonDecode(json['data']);
      tags = data['tags'];
      peers = data['peers'];
    }
    print(json);
    notifyListeners();
    return resp.body;
  }

  Future<String> getApiServer() async {
    return await _ffi?.bind.mainGetApiServer() ?? "";
  }

  void reset() {
    tags.clear();
    peers.clear();
    notifyListeners();
  }

  Future<Map<String, String>>? _getHeaders() {
    return _ffi?.getHttpHeaders();
  }
}
