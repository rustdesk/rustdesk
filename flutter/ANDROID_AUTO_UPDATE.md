# Android Auto-Update для RustDesk

## 📋 Обзор

Реализовано автоматическое обновление приложения RustDesk на Android через GitHub Releases API.

## ✨ Возможности

✅ Проверка новых версий на GitHub  
✅ Скачивание APK в фоне  
✅ Отслеживание прогресса загрузки  
✅ Автоматическая установка APK  
✅ Работает независимо от маркетплейса  
✅ Поддержка различных архитектур ARM  

## 🏗️ Архитектура

### Android Native (Kotlin)
- **`UpdateService.kt`** - сервис проверки и загрузки обновлений
  - Парсит GitHub Releases API
  - Сравнивает версии
  - Скачивает APK
  - Управляет установкой через Intent

### Flutter
- **`UpdateManager`** (`lib/services/update_manager.dart`) - управляет коммуникацией с Android
- **`UpdateDialog`** (`lib/widgets/update_dialog.dart`) - UI компоненты
  - `UpdateDialog` - модальное окно с информацией об обновлении
  - `CheckUpdateButton` - кнопка для проверки обновлений
  - `VersionInfoWidget` - виджет отображения версии

### Конфигурация Android
- **`AndroidManifest.xml`** - добавлены необходимые permissions и FileProvider
- **`file_paths.xml`** - конфигурация для FileProvider (для установки APK на Android 7+)

## 🚀 Использование

### 1. Базовая интеграция в Settings

```dart
import 'package:flutter_hbb/services/update_manager.dart';
import 'package:flutter_hbb/widgets/update_dialog.dart';

class SettingsPage extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
        actions: [
          CheckUpdateButton(), // Кнопка проверки обновлений
        ],
      ),
    );
  }
}
```

### 2. Программная проверка обновлений

```dart
final updateManager = UpdateManager();

// Настройка callbacks
updateManager.onUpdateAvailable = (updateInfo) {
  print('Доступна версия: ${updateInfo.version}');
  print('Changelog: ${updateInfo.releaseNotes}');
};

updateManager.onDownloadProgress = (progress) {
  print('Прогресс: $progress%');
};

updateManager.onError = (error) {
  print('Ошибка: $error');
};

// Проверить обновления
await updateManager.checkForUpdates();

// Скачать и установить
if (updateInfo != null) {
  await updateManager.downloadAndInstall(updateInfo);
}
```

### 3. Автоматическая проверка при запуске

```dart
@override
void initState() {
  super.initState();
  final updateController = Get.put(UpdateDialogController());
  
  // Проверить обновления при запуске
  WidgetsBinding.instance.addPostFrameCallback((_) {
    updateController.checkForUpdates();
  });
}
```

## 📱 UI Компоненты

### CheckUpdateButton
```dart
CheckUpdateButton(
  controller: updateController,
  padding: const EdgeInsets.all(8.0),
  iconSize: 24.0,
)
```

### VersionInfoWidget
```dart
VersionInfoWidget(
  currentVersion: '1.4.4',
  showCheckButton: true,
)
```

### UpdateDialog (автоматически показывается)
Показывает:
- Номер версии
- Changelog из GitHub
- Прогресс-бар загрузки
- Кнопки "Позже" и "Обновить"

## 🔧 Требования и зависимости

### pubspec.yaml
```yaml
dependencies:
  dio: ^5.3.1
  app_installer: ^0.4.0
  package_info_plus: ^4.2.0  # уже был
  percent_indicator: ^4.2.2  # для UI
```

### Permissions (AndroidManifest.xml)
```xml
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" />
<uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES" />
```

## 🔄 Процесс обновления

1. **Проверка версии**
   - Запрос к GitHub API: `https://api.github.com/repos/rustdesk/rustdesk/releases/latest`
   - Получение тега версии (например: `v1.4.5`)
   - Сравнение с текущей версией

2. **Поиск APK**
   - Ищет APK в ассетах релиза
   - Поддерживает: arm64-v8a, armeabi-v7a, universal
   - Приоритет по архитектуре устройства

3. **Загрузка**
   - Скачивание в `getExternalFilesDir()`
   - Отслеживание прогресса
   - Проверка целостности файла

4. **Установка**
   - Использование FileProvider для Android 7+
   - Запуск встроенного инсталлятора
   - Автоматическое открытие экрана разрешений

## 🐛 Обработка ошибок

Все ошибки передаются через `onError` callback:

```dart
- GitHub API ошибки
- Ошибки загрузки
- Ошибки парсинга версии
- Ошибки установки
```

## 🚦 Поддерживаемые версии Android

- ✅ Android 5.0+ (API 21+)
- ✅ Все архитектуры (ARM, ARM64, x86)
- ✅ Android 10+ с ограничениями файловой системы

## 📝 Notes

1. **GitHub API Rate Limit**: 60 запросов/час для неавторизированных запросов
2. **APK размер**: Загружается в локальное хранилище приложения
3. **Автоматическая очистка**: Старые APK файлы удаляются при новой загрузке

## 🔐 Безопасность

- ✅ HTTPS для всех запросов
- ✅ FileProvider для безопасного доступа к файлам
- ✅ Проверка версии перед установкой
- ✅ Локальное хранилище приложения (недоступно другим приложениям)

## 📚 Примеры использования

See: `flutter/lib/services/update_manager.dart` и `flutter/lib/widgets/update_dialog.dart`

## 🎯 Следующие шаги

1. ✅ Реализовано: Kotlin сервис + Flutter интеграция
2. ⏳ TODO: Настроить GitHub Actions для выпуска APK
3. ⏳ TODO: Добавить автоматическую проверку при запуске приложения
4. ⏳ TODO: Локализация сообщений об ошибках
