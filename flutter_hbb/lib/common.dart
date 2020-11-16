import 'package:flutter/material.dart';
import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:ffi';
import 'dart:async';
import 'dart:convert';

class HexColor extends Color {
  HexColor(final String hexColor) : super(_getColorFromHex(hexColor));

  static int _getColorFromHex(String hexColor) {
    hexColor = hexColor.toUpperCase().replaceAll('#', '');
    if (hexColor.length == 6) {
      hexColor = 'FF' + hexColor;
    }
    return int.parse(hexColor, radix: 16);
  }
}

class MyTheme {
  static const Color grayBg = Color(0xFFEEEEEE);
  static const Color white = Color(0xFFFFFFFF);
}

typedef F1 = void Function(Pointer<Utf8>);
typedef F2 = Pointer<Utf8> Function();

// https://juejin.im/post/6844903864852807694
class FfiModel with ChangeNotifier {
  F1 _connectRemote;
  F2 _getPeers;
  F1 _freeCString;

  FfiModel() {
    initialzeFFI();
  }

  void addRemote() {
    notifyListeners();
  }

  void connect(String id) {
    _connectRemote(Utf8.toUtf8(id));
  }

  String getId() {
    return "";
  }

  void peers() {
    var p = _getPeers();
    try {
      List<dynamic> peers = json.decode(Utf8.fromUtf8(p));
      // https://github.com/brickpop/flutter-rust-ffi
      _freeCString(p);
      peers = peers
          .map((s) => s as List<dynamic>)
          .map((s) =>
              [s[0] as String, Peer.fromJson(s[1] as Map<String, dynamic>)])
          .toList();
    } catch (e) {
      print(e);
    }
  }

  Future<Null> initialzeFFI() async {
    final dylib = Platform.isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : DynamicLibrary.process();
    final initialize = dylib.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('initialize');
    _connectRemote = dylib
        .lookupFunction<Void Function(Pointer<Utf8>), F1>('connect_remote');
    _getPeers = dylib.lookupFunction<F2, F2>('get_peers');
    _freeCString = dylib
        .lookupFunction<Void Function(Pointer<Utf8>), F1>('rust_cstr_free');
    final dir = (await getApplicationDocumentsDirectory()).path;
    initialize(Utf8.toUtf8(dir));
    notifyListeners();
  }
}

class Peer {
  final String name;
  final String email;

  Peer(this.name, this.email);

  Peer.fromJson(Map<String, dynamic> json)
      : name = json['name'],
        email = json['email'];

  Map<String, dynamic> toJson() => {
        'name': name,
        'email': email,
      };
}
