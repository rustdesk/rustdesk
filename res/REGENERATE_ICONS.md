# Icon Regeneration Guide

This document describes how to regenerate app icons from the source icon files.

## Source Icons

- **Primary source**: `res/icon.png` (2048x2048 PNG) - Used for iOS and Android
- **SVG source**: `flutter/assets/icon.svg` - Vector format, can be used to generate any resolution
- **macOS source**: `res/mac-icon.png` - Specific macOS variant

## Prerequisites

Install required tools:
```bash
sudo apt-get install librsvg2-bin imagemagick
```

## Regenerating iOS Icons

### Method 1: Using ImageMagick (Manual)

```bash
#!/bin/bash
SOURCE_ICON="res/icon.png"
OUTPUT_DIR="flutter/ios/Runner/Assets.xcassets/AppIcon.appiconset"

# Function to generate icon with alpha removal for iOS
generate_icon() {
    local size=$1
    local filename=$2
    convert "$SOURCE_ICON" -resize ${size}x${size} -background white -alpha remove -alpha off "$OUTPUT_DIR/$filename"
}

# iPhone icons
generate_icon 40 "Icon-App-20x20@2x.png"
generate_icon 60 "Icon-App-20x20@3x.png"
generate_icon 29 "Icon-App-29x29@1x.png"
generate_icon 58 "Icon-App-29x29@2x.png"
generate_icon 87 "Icon-App-29x29@3x.png"
generate_icon 80 "Icon-App-40x40@2x.png"
generate_icon 120 "Icon-App-40x40@3x.png"
generate_icon 120 "Icon-App-60x60@2x.png"
generate_icon 180 "Icon-App-60x60@3x.png"

# iPad icons
generate_icon 20 "Icon-App-20x20@1x.png"
generate_icon 40 "Icon-App-40x40@1x.png"
generate_icon 76 "Icon-App-76x76@1x.png"
generate_icon 152 "Icon-App-76x76@2x.png"
generate_icon 167 "Icon-App-83.5x83.5@2x.png"

# App Store icon
generate_icon 1024 "Icon-App-1024x1024@1x.png"
```

### Method 2: Using flutter_launcher_icons (Automated)

```bash
cd flutter
flutter pub run flutter_launcher_icons
```

This uses the configuration in `flutter/pubspec.yaml`:
```yaml
flutter_icons:
  image_path: "../res/icon.png"
  remove_alpha_ios: true
  ios: true
  android: true
```

## Regenerating Android Icons

```bash
SOURCE_ICON="res/icon.png"
BASE_DIR="flutter/android/app/src/main/res"

convert "$SOURCE_ICON" -resize 48x48 "$BASE_DIR/mipmap-mdpi/ic_launcher.png"
convert "$SOURCE_ICON" -resize 72x72 "$BASE_DIR/mipmap-hdpi/ic_launcher.png"
convert "$SOURCE_ICON" -resize 96x96 "$BASE_DIR/mipmap-xhdpi/ic_launcher.png"
convert "$SOURCE_ICON" -resize 144x144 "$BASE_DIR/mipmap-xxhdpi/ic_launcher.png"
convert "$SOURCE_ICON" -resize 192x192 "$BASE_DIR/mipmap-xxxhdpi/ic_launcher.png"
```

Or use `flutter_launcher_icons` as shown above.

## Updating Source Icon from SVG

If you need to regenerate the source PNG from the SVG at a different resolution:

```bash
# Generate 2048x2048 PNG from SVG
rsvg-convert -w 2048 -h 2048 flutter/assets/icon.svg -o res/icon.png

# Or for even higher resolution (4096x4096)
rsvg-convert -w 4096 -h 4096 flutter/assets/icon.svg -o res/icon.png
```

## iOS Icon Requirements

- All iOS icons must have alpha channel removed (opaque)
- White background is applied to maintain appearance
- Icons are automatically rounded by iOS system
- Retina displays require @2x and @3x variants

## Icon Sizes

### iOS
- 20pt (20x20, 40x40, 60x60)
- 29pt (29x29, 58x58, 87x87)
- 40pt (40x40, 80x80, 120x120)
- 60pt (120x120, 180x180)
- 76pt (76x76, 152x152) - iPad
- 83.5pt (167x167) - iPad Pro
- 1024x1024 - App Store

### Android
- mdpi: 48x48
- hdpi: 72x72
- xhdpi: 96x96
- xxhdpi: 144x144
- xxxhdpi: 192x192

## Quality Tips

1. Always start from the highest resolution source available (SVG preferred)
2. Use at least 2048x2048 PNG as intermediate source
3. Let ImageMagick/flutter_launcher_icons handle downscaling
4. Never upscale low-resolution images - regenerate from vector source
5. Test on actual devices with Retina displays to verify quality

## References

- [Apple Human Interface Guidelines - App Icons](https://developer.apple.com/design/human-interface-guidelines/app-icons)
- [Android App Icon Guidelines](https://developer.android.com/guide/practices/ui_guidelines/icon_design_launcher)
