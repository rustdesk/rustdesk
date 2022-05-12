import 'dart:io';
import 'dart:typed_data';
import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';
import 'package:device_info/device_info.dart';
import 'package:package_info/package_info.dart';
import 'package:external_path/external_path.dart';
import 'package:flutter/services.dart';
import '../common.dart';

class RgbaFrame extends Struct {
  @Uint32()
  external int len;
  external Pointer<Uint8> data;
}

typedef F2 = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef F3 = void Function(Pointer<Utf8>, Pointer<Utf8>);
typedef F4 = void Function(Pointer<RgbaFrame>);
typedef F5 = Pointer<RgbaFrame> Function();

class PlatformFFI {
  static Pointer<RgbaFrame>? _lastRgbaFrame;
  static String _dir = '';
  static String _homeDir = '';
  static F2? _getByName;
  static F3? _setByName;
  static F4? _freeRgba;
  static F5? _getRgba;

  static void clearRgbaFrame() {
    if (_lastRgbaFrame != null &&
        _lastRgbaFrame != nullptr &&
        _freeRgba != null) _freeRgba!(_lastRgbaFrame!);
  }

  static Uint8List? getRgba() {
    if (_getRgba == null) return null;
    _lastRgbaFrame = _getRgba!();
    if (_lastRgbaFrame == null || _lastRgbaFrame == nullptr) return null;
    final ref = _lastRgbaFrame!.ref;
    return Uint8List.sublistView(ref.data.asTypedList(ref.len));
  }

  static Future<String> getVersion() async {
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    return packageInfo.version;
  }

  static String getByName(String name, [String arg = '']) {
    if (_getByName == null) return '';
    var a = name.toNativeUtf8();
    var b = arg.toNativeUtf8();
    var p = _getByName!(a, b);
    assert(p != nullptr);
    var res = p.toDartString();
    calloc.free(p);
    calloc.free(a);
    calloc.free(b);
    return res;
  }

  static void setByName(String name, [String value = '']) {
    if (_setByName == null) return;
    var a = name.toNativeUtf8();
    var b = value.toNativeUtf8();
    _setByName!(a, b);
    calloc.free(a);
    calloc.free(b);
  }

  static Future<Null> init() async {
    isIOS = Platform.isIOS;
    isAndroid = Platform.isAndroid;
    final dylib = Platform.isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : DynamicLibrary.process();
    print('initializing FFI');
    try {
      _getByName = dylib.lookupFunction<F2, F2>('get_by_name');
      _setByName =
          dylib.lookupFunction<Void Function(Pointer<Utf8>, Pointer<Utf8>), F3>(
              'set_by_name');
      _freeRgba = dylib
          .lookupFunction<Void Function(Pointer<RgbaFrame>), F4>('free_rgba');
      _getRgba = dylib.lookupFunction<F5, F5>('get_rgba');
      _dir = (await getApplicationDocumentsDirectory()).path;
      try {
        _homeDir = (await ExternalPath.getExternalStorageDirectories())[0];
      } catch (e) {
        print(e);
      }
      String id = 'NA';
      String name = 'Flutter';
      DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
      if (Platform.isAndroid) {
        AndroidDeviceInfo androidInfo = await deviceInfo.androidInfo;
        name = '${androidInfo.brand}-${androidInfo.model}';
        id = androidInfo.id.hashCode.toString();
        androidVersion = androidInfo.version.sdkInt;
      } else {
        IosDeviceInfo iosInfo = await deviceInfo.iosInfo;
        name = iosInfo.utsname.machine;
        id = iosInfo.identifierForVendor.hashCode.toString();
      }
      print("info1-id:$id,info2-name:$name,dir:$_dir,homeDir:$_homeDir");
      setByName('info1', id);
      setByName('info2', name);
      setByName('home_dir', _homeDir);
      setByName('init', _dir);
    } catch (e) {
      print(e);
    }
    version = await getVersion();
  }

  static void startDesktopWebListener() {}

  static void stopDesktopWebListener() {}

  static void setMethodCallHandler(FMethod callback) {
    toAndroidChannel.setMethodCallHandler((call) async {
      callback(call.method, call.arguments);
      return null;
    });
  }

  static invokeMethod(String method, [dynamic arguments]) async {
    if (!isAndroid) return Future<bool>(() => false);
    return await toAndroidChannel.invokeMethod(method, arguments);
  }
}

final localeName = Platform.localeName;
final toAndroidChannel = MethodChannel("mChannel");
