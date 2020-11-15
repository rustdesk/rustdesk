import 'package:flutter/material.dart';
import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import 'dart:ffi';
import 'dart:async';

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

// https://juejin.im/post/6844903864852807694
class FfiModel with ChangeNotifier {
  var _connectRemote;

  FfiModel() {
    initialzeFFI();
  }

  void addRemote() {
    notifyListeners();
  }

  void connect(String id) {
    _connectRemote(Utf8.toUtf8(id));
  }

  Future<Null> initialzeFFI() async {
    final dylib = Platform.isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : DynamicLibrary.process();
    final initialize = dylib.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('initialize');
    _connectRemote = dylib.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('connect_remote');
    final dir = (await getApplicationDocumentsDirectory()).path;
    initialize(Utf8.toUtf8(dir));
    notifyListeners();
  }
}
