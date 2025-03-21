import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:external_path/external_path.dart';
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';

import '../common.dart';
import '../generated_bridge.dart';

final class RgbaFrame extends Struct {
  @Uint32()
  external int len;
  external Pointer<Uint8> data;
}

typedef F3 = Pointer<Uint8> Function(Pointer<Utf8>, int);
typedef F3Dart = Pointer<Uint8> Function(Pointer<Utf8>, Int32);
typedef HandleEvent = Future<void> Function(Map<String, dynamic> evt);


typedef StartGrabLoopNative = Void Function();
typedef StartGrabLoopDart = void Function();

// Typedefs for FFI-funksjonen som frigjør alle modifikatorer.
typedef ReleaseModifiersNative = Void Function();
typedef ReleaseModifiersDart = void Function();

class NativeBindings {
  static final DynamicLibrary _lib = Platform.isWindows
      ? DynamicLibrary.open('librustdesk.dll')
      : DynamicLibrary.process(); // For andre plattformer 

  static final ReleaseModifiersDart releaseAllModifiers = _lib
      .lookup<NativeFunction<ReleaseModifiersNative>>('release_all_modifiers')
      .asFunction();


      // Ny binding for start_grab_loop
  static final StartGrabLoopDart startGrabLoop = _lib
      .lookup<NativeFunction<StartGrabLoopNative>>('start_grab_loop_ffi')
      .asFunction();
}



/// FFI wrapper around the native Rust core.
/// Hides the platform differences.
class PlatformFFI {
  String _dir = '';
  // _homeDir is only needed for Android and IOS.
  String _homeDir = '';
  final _eventHandlers = <String, Map<String, HandleEvent>>{};
  late RustdeskImpl _ffiBind;
  late String _appType;
  StreamEventHandler? _eventCallback;

  PlatformFFI._();

  // Global instans – bruk denne via f.eks. platform_model.dart.
  static final PlatformFFI instance = PlatformFFI._();

  final _toAndroidChannel = const MethodChannel('mChannel');

  RustdeskImpl get ffiBind => _ffiBind;
  F3? _session_get_rgba;

  static get localeName => Platform.localeName;

  static get isMain => instance._appType == kAppTypeMain;

  static String getByName(String name, [String arg = '']) {
    return '';
  }

  static void setByName(String name, [String value = '']) {}

  static Future<String> getVersion() async {
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    return packageInfo.version;
  }

  bool registerEventHandler(String eventName, String handlerName, HandleEvent handler) {
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


  void releaseLocalModifiers() {
    if (Platform.isWindows) {
      NativeBindings.releaseAllModifiers();
    }
    // For andre plattformer.
  }


    void startGrabLoop() {
    if (Platform.isWindows) {
      NativeBindings.startGrabLoop();
    }
    // Legg til for andre plattformer.
  }

  String translate(String name, String locale) =>
      _ffiBind.translate(name: name, locale: locale);

  Uint8List? getRgba(SessionID sessionId, int display, int bufSize) {
    if (_session_get_rgba == null) return null;
    final sessionIdStr = sessionId.toString();
    var a = sessionIdStr.toNativeUtf8();
    try {
      final buffer = _session_get_rgba!(a, display);
      if (buffer == nullptr) {
        return null;
      }
      final data = buffer.asTypedList(bufSize);
      return data;
    } finally {
      malloc.free(a);
    }
  }

  int getRgbaSize(SessionID sessionId, int display) =>
      _ffiBind.sessionGetRgbaSize(sessionId: sessionId, display: display);
  void nextRgba(SessionID sessionId, int display) =>
      _ffiBind.sessionNextRgba(sessionId: sessionId, display: display);
  void registerPixelbufferTexture(SessionID sessionId, int display, int ptr) =>
      _ffiBind.sessionRegisterPixelbufferTexture(sessionId: sessionId, display: display, ptr: ptr);
  void registerGpuTexture(SessionID sessionId, int display, int ptr) =>
      _ffiBind.sessionRegisterGpuTexture(sessionId: sessionId, display: display, ptr: ptr);

  /// Init the FFI class, loads the native Rust core library.
  Future<void> init(String appType) async {
    _appType = appType;
    final dylib = isAndroid
        ? DynamicLibrary.open('librustdesk.so')
        : isLinux
            ? DynamicLibrary.open('librustdesk.so')
            : isWindows
                ? DynamicLibrary.open('librustdesk.dll')
                : DynamicLibrary.process();
    debugPrint('initializing FFI $_appType');
    try {
      _session_get_rgba = dylib.lookupFunction<F3Dart, F3>("session_get_rgba");
      try {
        _dir = (await getApplicationDocumentsDirectory()).path;
      } catch (e) {
        debugPrint('Failed to get documents directory: $e');
      }
      _ffiBind = RustdeskImpl(dylib);

      if (isLinux) {
        if (isMain) {
          _ffiBind.mainStartDbusServer();
        }
      } else if (isMacOS && isMain) {
        _ffiBind.mainStartIpcUrlServer();
      }
      _startListenEvent(_ffiBind);
      try {
        if (isAndroid) {
          _homeDir = (await ExternalPath.getExternalStorageDirectories())[0];
        } else if (isIOS) {
          _homeDir = _ffiBind.mainGetDataDirIos();
        }
      } catch (e) {
        debugPrintStack(label: 'initialize failed: $e');
      }
      String id = 'NA';
      String name = 'Flutter';
      DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
      if (isAndroid) {
        AndroidDeviceInfo androidInfo = await deviceInfo.androidInfo;
        name = '${androidInfo.brand}-${androidInfo.model}';
        id = androidInfo.id.hashCode.toString();
        androidVersion = androidInfo.version.sdkInt;
      } else if (isIOS) {
        IosDeviceInfo iosInfo = await deviceInfo.iosInfo;
        name = iosInfo.utsname.machine;
        id = iosInfo.identifierForVendor.hashCode.toString();
      } else if (isLinux) {
        LinuxDeviceInfo linuxInfo = await deviceInfo.linuxInfo;
        name = linuxInfo.name;
        id = linuxInfo.machineId ?? linuxInfo.id;
      } else if (isWindows) {
        try {
          windowsBuildNumber = getWindowsTargetBuildNumber();
          WindowsDeviceInfo winInfo = await deviceInfo.windowsInfo;
          name = winInfo.computerName;
          id = winInfo.computerName;
        } catch (e) {
          debugPrintStack(label: "get windows device info failed: $e");
          name = "unknown";
          id = "unknown";
        }
      } else if (isMacOS) {
        MacOsDeviceInfo macOsInfo = await deviceInfo.macOsInfo;
        name = macOsInfo.computerName;
        id = macOsInfo.systemGUID ?? '';
      }
      if (isAndroid || isIOS) {
        debugPrint('_appType:$_appType,info1-id:$id,info2-name:$name,dir:$_dir,homeDir:$_homeDir');
      } else {
        debugPrint('_appType:$_appType,info1-id:$id,info2-name:$name,dir:$_dir');
      }
      if (desktopType == DesktopType.cm) {
        await _ffiBind.cmInit();
      }
      await _ffiBind.mainDeviceId(id: id);
      await _ffiBind.mainDeviceName(name: name);
      await _ffiBind.mainSetHomeDir(home: _homeDir);
      await _ffiBind.mainInit(appDir: _dir, customClientConfig: '');
    } catch (e) {
      debugPrintStack(label: 'initialize failed: $e');
    }
    version = await getVersion();
  }

  Future<bool> tryHandle(Map<String, dynamic> evt) async {
    final name = evt['name'];
    if (name != null) {
      final handlers = _eventHandlers[name];
      if (handlers != null && handlers.isNotEmpty) {
        for (var handler in handlers.values) {
          await handler(evt);
        }
        return true;
      }
    }
    return false;
  }

  /// Start listening to the Rust core's events and frames.
  void _startListenEvent(RustdeskImpl rustdeskImpl) {
    final appType = _appType == kAppTypeDesktopRemote ? '$_appType,$kWindowId' : _appType;
    var sink = rustdeskImpl.startGlobalEventStream(appType: appType);
    sink.listen((message) {
      () async {
        try {
          Map<String, dynamic> event = json.decode(message);
          if (!await tryHandle(event)) {
            if (_eventCallback != null) {
              await _eventCallback!(event);
            }
          }
        } catch (e) {
          debugPrint('json.decode fail(): $e');
        }
      }();
    });
  }

  void setEventCallback(StreamEventHandler fun) async {
    _eventCallback = fun;
  }

  void setRgbaCallback(void Function(int, Uint8List) fun) async {}

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

  void syncAndroidServiceAppDirConfigPath() {
    invokeMethod(AndroidChannel.kSyncAppDirConfigPath, _dir);
  }

  void setFullscreenCallback(void Function(bool) fun) {}
}

/* // Extension på PlatformFFI – må ligge utenfor klassen.
extension PlatformFFIExtensions on PlatformFFI {
  void releaseLocalModifiers() {
    if (Platform.isWindows) {
      NativeBindings.releaseAllModifiers();
    }
    // For andre plattformer: legg til tilsvarende logikk.
  }
} */

