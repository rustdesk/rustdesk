import 'package:flutter/services.dart';
import 'package:package_info_plus/package_info_plus.dart';

class UpdateManager {
  static const platform = MethodChannel('mChannel');
  
  /// Текущая загружаемая информация об обновлении
  UpdateInfo? _currentUpdateInfo;
  int _downloadProgress = 0;
  
  /// Callback при доступности обновления
  Function(UpdateInfo)? onUpdateAvailable;
  
  /// Callback при ошибке
  Function(String)? onError;
  
  /// Callback при прогрессе загрузки (0-100)
  Function(int)? onDownloadProgress;
  
  /// Callback при успешной установке
  Function()? onUpdateInstalled;

  UpdateManager() {
    platform.setMethodCallHandler(_handleMethodCall);
  }

  /// Проверяет наличие обновлений
  Future<void> checkForUpdates() async {
    try {
      final PackageInfo packageInfo = await PackageInfo.fromPlatform();
      final version = packageInfo.version;
      
      await platform.invokeMethod('check_for_updates', version);
    } catch (e) {
      onError?.call('Ошибка при проверке обновлений: $e');
    }
  }

  /// Скачивает и устанавливает обновление
  Future<void> downloadAndInstall(UpdateInfo updateInfo) async {
    try {
      _currentUpdateInfo = updateInfo;
      _downloadProgress = 0;
      
      await platform.invokeMethod('download_and_install_update', {
        'version': updateInfo.version,
        'download_url': updateInfo.downloadUrl,
        'release_notes': updateInfo.releaseNotes,
        'file_name': updateInfo.fileName,
      });
    } catch (e) {
      onError?.call('Ошибка при загрузке обновления: $e');
    }
  }

  /// Отменяет текущую загрузку
  Future<void> cancelDownload() async {
    try {
      await platform.invokeMethod('cancel_update_download');
    } catch (e) {
      onError?.call('Ошибка при отмене загрузки: $e');
    }
  }

  /// Обработчик callback'ов от Android
  Future<dynamic> _handleMethodCall(MethodCall call) async {
    switch (call.method) {
      case 'on_update_available':
        final args = call.arguments as Map;
        final updateInfo = UpdateInfo(
          version: args['version'] ?? '',
          downloadUrl: args['download_url'] ?? '',
          releaseNotes: args['release_notes'] ?? '',
          fileName: args['file_name'] ?? '',
        );
        onUpdateAvailable?.call(updateInfo);
        break;
        
      case 'on_update_progress':
        final progress = call.arguments['progress'] as int? ?? 0;
        _downloadProgress = progress;
        onDownloadProgress?.call(progress);
        break;
        
      case 'on_update_installed':
        onUpdateInstalled?.call();
        break;
        
      case 'on_update_error':
        final error = call.arguments['error'] as String? ?? 'Unknown error';
        onError?.call(error);
        break;
    }
  }

  /// Получить текущий прогресс загрузки
  int getDownloadProgress() => _downloadProgress;

  /// Получить информацию об обновлении
  UpdateInfo? getCurrentUpdateInfo() => _currentUpdateInfo;
}

class UpdateInfo {
  final String version;
  final String downloadUrl;
  final String releaseNotes;
  final String fileName;

  UpdateInfo({
    required this.version,
    required this.downloadUrl,
    required this.releaseNotes,
    required this.fileName,
  });

  @override
  String toString() => 'UpdateInfo(version: $version)';
}
