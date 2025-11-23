# Code Style and Conventions for RustDesk

## Kotlin (Android)
- **Package structure**: `com.carriez.flutter_hbb` for main app code
- **Naming**: 
  - Classes: PascalCase (e.g., `MainActivity`, `SamsungDexUtils`)
  - Methods: camelCase (e.g., `setMetaKeyCapture`)
  - Constants: UPPER_SNAKE_CASE (e.g., `TAG`, `CHANNEL`)
  - Companion objects for static members
- **Logging**: Use `android.util.Log` with appropriate tags
- **Method channels**: Use descriptive channel names (e.g., "com.rustdesk.rustdesk/android_features")

## Dart/Flutter
- **Naming**:
  - Classes: PascalCase (e.g., `AndroidUtils`, `RdPlatformChannel`)
  - Methods/Variables: camelCase (e.g., `setDexMetaCapture`, `isDexEnabled`)
  - Constants: lowerCamelCase or UPPER_SNAKE_CASE for global constants
  - Private members: prefix with underscore (e.g., `_hostMethodChannel`)
- **File organization**:
  - One class per file generally
  - Utility classes in `flutter/lib/utils/`
  - Common code in `flutter/lib/common/`
  - Platform-specific code separated by directory
- **Error handling**: Use try-catch with PlatformException for method channel calls
- **Platform checks**: Use `Platform.isAndroid`, `Platform.isIOS`, etc.

## Rust
- **Naming**: 
  - Functions/Variables: snake_case
  - Types/Traits: PascalCase
  - Constants: UPPER_SNAKE_CASE
- **Error handling**: Use `Result<T, E>` pattern
- **Async code**: Use tokio/async-await patterns

## General Principles
- Follow existing patterns in the codebase
- Keep changes minimal and focused
- Add logging for debugging purposes with appropriate log levels
- Handle errors gracefully with user-friendly messages
- Use existing utilities and avoid code duplication
- Platform-specific code should be isolated and abstracted when possible
