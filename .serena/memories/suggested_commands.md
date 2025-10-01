# RustDesk Development Commands

## Build Commands

### Rust/Desktop Builds
```bash
# Basic build and run (requires libsciter)
cargo run

# Release build
cargo build --release

# Build with specific features
cargo build --features hwcodec          # Hardware codec support
cargo build --features vram             # VRAM feature (Windows only)
cargo build --features flutter          # Flutter UI support
```

### Flutter Desktop Builds
```bash
# Development build
python3 build.py --flutter

# Release build
python3 build.py --flutter --release

# With hardware codec
python3 build.py --hwcodec

# With VRAM (Windows only)
python3 build.py --vram
```

### Flutter Mobile Builds
```bash
# Android
cd flutter && flutter build android

# iOS
cd flutter && flutter build ios

# Run in development mode
cd flutter && flutter run
```

## Testing Commands
```bash
# Run Rust tests
cargo test

# Run Flutter tests
cd flutter && flutter test
```

## Platform-Specific Build Scripts
```bash
# Android build script
flutter/build_android.sh

# iOS build script
flutter/build_ios.sh

# F-Droid build script
flutter/build_fdroid.sh
```

## Common System Commands (Linux)
```bash
# Git operations
git status
git add .
git commit -s -m "message"    # Note: Use -s for DCO sign-off
git log

# File operations
ls -la
find . -name "pattern"
grep -r "pattern" .

# Directory navigation
cd path/to/directory
pwd
```

## Development Dependencies Setup
```bash
# Install vcpkg dependencies
vcpkg install libvpx libyuv opus aom

# Set VCPKG_ROOT environment variable
export VCPKG_ROOT=$HOME/vcpkg
```
