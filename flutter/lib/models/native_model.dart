import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:external_path/external_path.dart';
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';

import '../common.dart';
import '../generated_bridge.dart';

class RgbaFrame extends Struct {
  @Uint32()
  external int len;
  external Pointer<Uint8> data;
}

typedef F2 = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef F3 = void Function(Pointer<Utf8>, Pointer<Utf8>);
typedef HandleEvent = Future<void> Function(Map<String, dynamic> evt);

/// FFI wrapper around the native Rust core.
/// Hides the platform differences.
class PlatformFFI {
  String _dir = '';
  String _homeDir = '';
  F2? _translate;
  final _eventHandlers = <String, Map<String, HandleEvent>>{};
  late RustdeskImpl _ffiBind;
  late String _appType;
  StreamEventHandler? _eventCallback;

  PlatformFFI._();

  static final PlatformFFI instance = PlatformFFI._();
  final _toAndroidChannel = const MethodChannel('mChannel');

  RustdeskImpl get ffiBind => _ffiBind;

  static get localeName => Platform.localeName;

  static Future<String> getVersion() async {
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    return packageInfo.version;
  }

  bool registerEventHandler(
      String eventName, String handlerName, HandleEvent handler) {
    debugPrint('registerEventHandler $eventName $handlerName');
    var handlers = _eventHandlers[eventName];
    if (handlers == null) {
      _eventHandlers[eventName] = {handlerName: handler};
      return true;
    } else {
      if (handlers.containsKey(handlerName)) {
        return false;
      } else {
        handlers[handlerName] = handler;
        return true;
      }
    }
  }

  void unregisterEventHandler(String eventName, String handlerName) {
    debugPrint('unregisterEventHandler $eventName $handlerName');
    var handlers = _eventHandlers[eventName];
    if (handlers != null) {
      handlers.remove(handlerName);
    }
  }

  String translate(String name, String locale) {
    if (_translate == null) return name;
    var a = name.toNativeUtf8();
    var b = locale.toNativeUtf8();
    var p = _translate!(a, b);
    assert(p != nullptr);
    final res = p.toDartString();
    calloc.free(p);
    calloc.free(a);
    calloc.free(b);
    return res;
  }

  /// Init the FFI class, loads the native Rust core library.
  Future<void> init(String appType) async {
    _appType = appType;
    final dylib = Platform.isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : Platform.isLinux
            ? DynamicLibrary.open('librustdesk.so')
            : Platform.isWindows
                ? DynamicLibrary.open('librustdesk.dll')
                : Platform.isMacOS
                    ? DynamicLibrary.open("liblibrustdesk.dylib")
                    : DynamicLibrary.process();
    debugPrint('initializing FFI $_appType');
    try {
      _translate = dylib.lookupFunction<F2, F2>('translate');
      try {
        // SYSTEM user failed
        _dir = (await getApplicationDocumentsDirectory()).path;
      } catch (e) {
        debugPrint('Failed to get documents directory: $e');
      }
      _ffiBind = RustdeskImpl(dylib);
      if (Platform.isLinux) {
        // start dbus service, no need to await
        await _ffiBind.mainStartDbusServer();
      }
      _startListenEvent(_ffiBind); // global event
      try {
        if (isAndroid) {
          // only support for android
          _homeDir = (await ExternalPath.getExternalStorageDirectories())[0];
        } else {
          _homeDir = (await getDownloadsDirectory())?.path ?? '';
        }
      } catch (e) {
        debugPrint('initialize failed: $e');
      }
      String id = 'NA';
      String name = 'Flutter';
      DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
      if (Platform.isAndroid) {
        AndroidDeviceInfo androidInfo = await deviceInfo.androidInfo;
        name = '${androidInfo.brand}-${androidInfo.model}';
        id = androidInfo.id.hashCode.toString();
        androidVersion = androidInfo.version.sdkInt;
      } else if (Platform.isIOS) {
        IosDeviceInfo iosInfo = await deviceInfo.iosInfo;
        name = iosInfo.utsname.machine ?? '';
        id = iosInfo.identifierForVendor.hashCode.toString();
      } else if (Platform.isLinux) {
        LinuxDeviceInfo linuxInfo = await deviceInfo.linuxInfo;
        name = linuxInfo.name;
        id = linuxInfo.machineId ?? linuxInfo.id;
      } else if (Platform.isWindows) {
        try {
          WindowsDeviceInfo winInfo = await deviceInfo.windowsInfo;
          name = winInfo.computerName;
          id = winInfo.computerName;
          windowsBuildNumber = winInfo.buildNumber;
        } catch (e) {
          debugPrint("$e");
          name = "unknown";
          id = "unknown";
        }
      } else if (Platform.isMacOS) {
        MacOsDeviceInfo macOsInfo = await deviceInfo.macOsInfo;
        name = macOsInfo.computerName;
        id = macOsInfo.systemGUID ?? '';
      }
      debugPrint(
          '_appType:$_appType,info1-id:$id,info2-name:$name,dir:$_dir,homeDir:$_homeDir');
      await _ffiBind.mainDeviceId(id: id);
      await _ffiBind.mainDeviceName(name: name);
      await _ffiBind.mainSetHomeDir(home: _homeDir);
      await _ffiBind.mainInit(appDir: _dir);
    } catch (e) {
      debugPrint('initialize failed: $e');
    }
    version = await getVersion();
  }

  Future<bool> _tryHandle(Map<String, dynamic> evt) async {
    final name = evt['name'];
    if (name != null) {
      final handlers = _eventHandlers[name];
      if (handlers != null) {
        if (handlers.isNotEmpty) {
          for (var handler in handlers.values) {
            await handler(evt);
          }
          return true;
        }
      }
    }
    return false;
  }

  /// Start listening to the Rust core's events and frames.
  void _startListenEvent(RustdeskImpl rustdeskImpl) {
    () async {
      await for (final message
          in rustdeskImpl.startGlobalEventStream(appType: _appType)) {
        try {
          Map<String, dynamic> event = json.decode(message);
          // _tryHandle here may be more flexible than _eventCallback
          if (!await _tryHandle(event)) {
            if (_eventCallback != null) {
              await _eventCallback!(event);
            }
          }
        } catch (e) {
          debugPrint('json.decode fail(): $e');
        }
      }
    }();
  }

  void setEventCallback(StreamEventHandler fun) async {
    _eventCallback = fun;
  }

  void setRgbaCallback(void Function(Uint8List) fun) async {}

  void startDesktopWebListener() {}

  void stopDesktopWebListener() {}

  void setMethodCallHandler(FMethod callback) {
    _toAndroidChannel.setMethodCallHandler((call) async {
      callback(call.method, call.arguments);
      return null;
    });
  }

  invokeMethod(String method, [dynamic arguments]) async {
    if (!isAndroid) return Future<bool>(() => false);
    return await _toAndroidChannel.invokeMethod(method, arguments);
  }
}
