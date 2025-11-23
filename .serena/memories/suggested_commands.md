# Suggested Commands for RustDesk Development

## Build Commands

### Rust Build
```bash
# Build and run desktop application
cargo run

# Build with specific features
cargo build --release
cargo build --features hwcodec
cargo build --features vram  # Windows only
```

### Flutter Build
```bash
# Build Flutter desktop version
python3 build.py --flutter

# Build Flutter release version
python3 build.py --flutter --release

# Build with hardware codec support
python3 build.py --hwcodec
```

### Android Build
```bash
# Build Android APK
cd flutter && flutter build android

# Build Android APK (using script)
cd flutter && ./build_android.sh

# Build F-Droid version
cd flutter && ./build_fdroid.sh
```

### iOS Build
```bash
# Build iOS app
cd flutter && flutter build ios

# Build iOS app (using script)
cd flutter && ./build_ios.sh
```

## Testing Commands

### Rust Tests
```bash
cargo test
```

### Flutter Tests
```bash
cd flutter && flutter test
```

## Flutter Development Commands
```bash
# Run Flutter app in development mode
cd flutter && flutter run

# Install Flutter dependencies
cd flutter && flutter pub get

# Clean Flutter build
cd flutter && flutter clean

# Analyze Flutter code
cd flutter && flutter analyze
```

## Gradle Commands (Android)
```bash
# Build Android app with Gradle
cd flutter/android && ./gradlew build

# Clean Android build
cd flutter/android && ./gradlew clean
```

## Git Commands
All standard git commands are available:
- `git status` - Check repository status
- `git add .` - Stage changes
- `git commit -m "message"` - Commit changes
- `git diff` - View changes
- `git log` - View commit history

## Utility Commands (Linux)
- `ls` - List files
- `find` - Find files
- `grep` - Search in files
- `cat` - View file contents
