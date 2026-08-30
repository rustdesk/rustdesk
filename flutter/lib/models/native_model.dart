import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:ui' as ui;

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

const _kOhosHostInputCapable = 'ohos-host-input-capable';

typedef F3 = Pointer<Uint8> Function(Pointer<Utf8>, int);
typedef F3Dart = Pointer<Uint8> Function(Pointer<Utf8>, Int32);
typedef HandleEvent = Future<void> Function(Map<String, dynamic> evt);

/// The Linux bundle keeps the core library at lib/librustdesk.so next to the
/// executable. Prefer that copy, mirroring flutter/linux/main.cc: the plain
/// name relies on the loader search path, which repackaged installs may not
/// cover. https://github.com/rustdesk/rustdesk/discussions/14407
DynamicLibrary _openLinuxCoreLib() {
  final bundled =
      '${File(Platform.resolvedExecutable).parent.path}/lib/librustdesk.so';
  try {
    if (File(bundled).existsSync()) {
      return DynamicLibrary.open(bundled);
    }
  } catch (e) {
    debugPrint("Failed to load '$bundled': $e");
  }
  return DynamicLibrary.open('librustdesk.so');
}

/// FFI wrapper around the native Rust core.
/// Hides the platform differences.
class PlatformFFI {
  static String _ohosLocaleName = '';
  String _dir = '';
  // _homeDir is only needed for Android and IOS.
  String _homeDir = '';
  int _ohosDisplayId = 0;
  int _ohosDisplayWidth = 0;
  int _ohosDisplayHeight = 0;
  final _eventHandlers = <String, Map<String, HandleEvent>>{};
  late RustdeskImpl _ffiBind;
  late String _appType;
  StreamEventHandler? _eventCallback;

  PlatformFFI._();

  static final PlatformFFI instance = PlatformFFI._();
  final _toAndroidChannel = const MethodChannel('mChannel');
  final _toOhosChannel =
      const MethodChannel('top.frankhan.resk.flutter/platform');

  RustdeskImpl get ffiBind => _ffiBind;
  F3? _session_get_rgba;

  static String get localeName => isOhos && _ohosLocaleName.isNotEmpty
      ? _ohosLocaleName
      : Platform.localeName;

  static get isMain => instance._appType == kAppTypeMain;

  static String getByName(String name, [String arg = '']) {
    return '';
  }

  static void setByName(String name, [String value = '']) {}

  static Future<String> getVersion() async {
    if (isOhos) {
      return await instance._ffiBind.mainGetVersion();
    }
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    return packageInfo.version;
  }

  bool registerEventHandler(
      String eventName, String handlerName, HandleEvent handler,
      {bool replace = false}) {
    debugPrint('registerEventHandler $eventName $handlerName');
    var handlers = _eventHandlers[eventName];
    if (handlers == null) {
      _eventHandlers[eventName] = {handlerName: handler};
      return true;
    } else {
      if (!replace && handlers.containsKey(handlerName)) {
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
      _ffiBind.sessionRegisterPixelbufferTexture(
          sessionId: sessionId, display: display, ptr: ptr);
  void registerGpuTexture(SessionID sessionId, int display, int ptr) =>
      _ffiBind.sessionRegisterGpuTexture(
          sessionId: sessionId, display: display, ptr: ptr);

  /// Init the FFI class, loads the native Rust core library.
  Future<void> init(String appType) async {
    _appType = appType;
    final dylib = isOhos
        ? DynamicLibrary.open('liblibrustdesk.so')
        : isAndroid
            ? DynamicLibrary.open('librustdesk.so')
            : isLinux
                ? _openLinuxCoreLib()
                : isWindows
                    ? DynamicLibrary.open('librustdesk.dll')
                    :
                    // Use executable itself as the dynamic library for MacOS.
                    // Multiple dylib instances will cause some global instances to be invalid.
                    // eg. `lazy_static` objects in rust side, will be created more than once, which is not expected.
                    //
                    // isMacOS? DynamicLibrary.open("liblibrustdesk.dylib") :
                    DynamicLibrary.process();
    debugPrint('initializing FFI $_appType');
    try {
      _session_get_rgba = dylib.lookupFunction<F3Dart, F3>("session_get_rgba");
      try {
        if (isOhos) {
          _dir = await _toOhosChannel.invokeMethod<String>('getFilesDir') ?? '';
          ohosDeviceType =
              await _toOhosChannel.invokeMethod<String>('getDeviceType') ?? '';
          _ohosLocaleName =
              await _toOhosChannel.invokeMethod<String>('getSystemLocale') ??
                  '';
          final displayInfo = await _toOhosChannel
              .invokeMapMethod<String, dynamic>('getDefaultDisplayInfo');
          _ohosDisplayId = displayInfo?['displayId'] as int? ?? 0;
          _ohosDisplayWidth = displayInfo?['width'] as int? ?? 0;
          _ohosDisplayHeight = displayInfo?['height'] as int? ?? 0;
          if (ohosDeviceType == '2in1') {
            ohosTitleButtonReservedWidth =
                (await _toOhosChannel.invokeMethod<num>('prepareWindow'))
                        ?.toDouble() ??
                    0;
          }
        } else {
          // SYSTEM user failed
          _dir = (await getApplicationDocumentsDirectory()).path;
        }
      } catch (e) {
        debugPrint('Failed to get documents directory: $e');
      }
      if (_dir.isEmpty) {
        throw StateError('Application files directory is unavailable');
      }
      _ffiBind = RustdeskImpl(dylib);

      if (isLinux) {
        if (isMain) {
          // Start a dbus service for uri links, no need to await
          _ffiBind.mainStartDbusServer();
        }
      } else if (isMacOS && isMain) {
        // Start ipc service for uri links.
        _ffiBind.mainStartIpcUrlServer();
      }
      _startListenEvent(_ffiBind); // global event
      try {
        if (isAndroid) {
          // only support for android
          _homeDir = (await ExternalPath.getExternalStorageDirectories())[0];
        } else if (isIOS) {
          // The previous code was `_homeDir = (await getDownloadsDirectory())?.path ?? '';`,
          // which provided the `downloads` path in the sandbox.
          // It is unclear why we now use the `data` directory in the sandbox instead.
          _homeDir = _ffiBind.mainGetDataDirIos(appDir: _dir);
        } else if (isOhos) {
          _homeDir = _dir;
        } else {
          // no need to set home dir
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
      } else if (isOhos) {
        name = Platform.localHostname;
        id = name.hashCode.toString();
      } else if (isLinux) {
        LinuxDeviceInfo linuxInfo = await deviceInfo.linuxInfo;
        name = linuxInfo.name;
        id = linuxInfo.machineId ?? linuxInfo.id;
      } else if (isWindows) {
        try {
          // request windows build number to fix overflow on win7
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
      if (isAndroid || isIOS || isOhos) {
        debugPrint(
            '_appType:$_appType,info1-id:$id,info2-name:$name,dir:$_dir,homeDir:$_homeDir');
      } else {
        debugPrint(
            '_appType:$_appType,info1-id:$id,info2-name:$name,dir:$_dir');
      }
      if (desktopType == DesktopType.cm) {
        await _ffiBind.cmInit();
      }
      await _ffiBind.mainDeviceId(id: id);
      await _ffiBind.mainDeviceName(name: name);
      await _ffiBind.mainSetHomeDir(home: _homeDir);
      await _ffiBind.mainInit(
        appDir: _dir,
        customClientConfig: '',
      );
      if (isOhos && isMain) {
        await _ffiBind.mainSetLocalOption(
          key: _kOhosHostInputCapable,
          value: isOhosDesktop ? 'Y' : 'N',
        );
        final configured = await _ffiBind.mainConfigureOhosHostDisplay(
          width: _ohosDisplayWidth,
          height: _ohosDisplayHeight,
          displayId: _ohosDisplayId,
        );
        debugPrint('OHOS host display configured: $configured');
        final stopError = await _ffiBind.mainStopOhosHost();
        if (stopError.isNotEmpty) {
          debugPrint('Failed to initialize OHOS host state: $stopError');
        }
      }
    } catch (e) {
      debugPrintStack(label: 'initialize failed: $e');
    }
    version = await getVersion();
  }

  Future<bool> tryHandle(Map<String, dynamic> evt) async {
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
    final appType =
        _appType == kAppTypeDesktopRemote ? '$_appType,$kWindowId' : _appType;
    var sink = rustdeskImpl.startGlobalEventStream(appType: appType);
    sink.listen((message) {
      () async {
        try {
          Map<String, dynamic> event = json.decode(message);
          // _tryHandle here may be more flexible than _eventCallback
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

  // web only, decoded WebCodecs frames arriving as ready-made images
  void setVideoFrameCallback(
      Future<void> Function(int, ui.Image, bool Function()) fun) {}

  void clearVideoFrameCallback() {}

  void startDesktopWebListener() {}

  void stopDesktopWebListener() {}

  Future<List<String>> getSoundInputs() async {
    if (isOhos) {
      return (await _toOhosChannel
                  .invokeListMethod<String>('getAudioInputDevices') ??
              <String>[])
          .where((device) => device.isNotEmpty)
          .toList();
    }
    return (await _ffiBind.mainGetSoundInputs())
        .where((device) => device.isNotEmpty)
        .toList();
  }

  Future<void> selectSoundInput(String device) async {
    if (!isOhos) return;
    await _toOhosChannel
        .invokeMethod<void>('selectAudioInputDevice', {'device': device});
  }

  Future<bool> isWindowMaximized() async {
    if (!isOhos) return false;
    return await _toOhosChannel.invokeMethod<bool>('isWindowMaximized') ??
        false;
  }

  Future<void> minimizeWindow() async {
    if (!isOhos) return;
    await _toOhosChannel.invokeMethod<void>('minimizeWindow');
  }

  Future<bool> toggleMaximizeWindow() async {
    if (!isOhos) return false;
    return await _toOhosChannel.invokeMethod<bool>('toggleMaximizeWindow') ??
        false;
  }

  Future<void> startMovingWindow() async {
    if (!isOhos) return;
    await _toOhosChannel.invokeMethod<void>('startMovingWindow');
  }

  Future<void> setKeepScreenOn(bool enabled) async {
    if (!isOhos) return;
    await _toOhosChannel
        .invokeMethod<void>('setKeepScreenOn', {'enabled': enabled});
  }

  Future<String> startOhosHost() async {
    if (!isOhos) return 'OHOS host is unavailable';
    await _ffiBind.mainSetLocalOption(
      key: _kOhosHostInputCapable,
      value: isOhosDesktop ? 'Y' : 'N',
    );
    try {
      final displayInfo = await _toOhosChannel
          .invokeMapMethod<String, dynamic>('getDefaultDisplayInfo');
      final displayId = displayInfo?['displayId'] as int? ?? 0;
      final width = displayInfo?['width'] as int? ?? 0;
      final height = displayInfo?['height'] as int? ?? 0;
      if (width > 0 && height > 0) {
        _ohosDisplayId = displayId;
        _ohosDisplayWidth = width;
        _ohosDisplayHeight = height;
        final configured = await _ffiBind.mainConfigureOhosHostDisplay(
          width: width,
          height: height,
          displayId: displayId,
        );
        if (!configured) {
          return 'Failed to configure the HarmonyOS host display';
        }
      }
    } catch (error) {
      return 'Failed to read the HarmonyOS display: $error';
    }
    try {
      final microphoneGranted = await _toOhosChannel
              .invokeMethod<bool>('ensureMicrophonePermission') ??
          false;
      if (!microphoneGranted) {
        return 'Microphone permission is required for HarmonyOS hosting';
      }
    } catch (error) {
      return 'Failed to request HarmonyOS microphone permission: $error';
    }
    // READ_PASTEBOARD is a restricted ACL permission. Keep host clipboard
    // fail-closed until the Flutter bundle has a matching privileged profile.
    await _ffiBind.mainSetOhosHostClipboardEnabled(enabled: false);
    final error = await _ffiBind.mainStartOhosHost();
    if (error.isNotEmpty) {
      await _ffiBind.mainSetOhosHostClipboardEnabled(enabled: false);
      return error;
    }
    try {
      await _toOhosChannel.invokeMethod<void>('startContinuousTask');
      return '';
    } catch (error) {
      await _ffiBind.mainStopOhosHost();
      return 'Failed to start the HarmonyOS continuous task: $error';
    }
  }

  Future<String> stopOhosHost() async {
    if (!isOhos) return '';
    final error = await _ffiBind.mainStopOhosHost();
    try {
      await _toOhosChannel.invokeMethod<void>('stopContinuousTask');
    } catch (backgroundError) {
      if (error.isEmpty) {
        return 'Failed to stop the HarmonyOS continuous task: $backgroundError';
      }
    }
    return error;
  }

  Future<void> closeWindow() async {
    if (!isOhos) return;
    await _toOhosChannel.invokeMethod<void>('closeWindow');
  }

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
