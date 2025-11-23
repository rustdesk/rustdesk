# Samsung DeX and Pointer Capture Usage Guide

## Overview
This implementation adds Samsung DeX Meta key capture and Pointer Capture (mouse immersion) features to the RustDesk Android app.

## Features Implemented

### 1. Samsung DeX Meta Key Capture
Allows capturing the Meta (Windows/Command) key on Samsung DeX devices, preventing the system from intercepting it.

### 2. Pointer Capture (Mouse Immersion)
Enables raw relative mouse movement capture, hiding the cursor and providing delta movements instead of absolute coordinates.

## API Reference

### Kotlin (Native Android)

#### SamsungDexUtils
```kotlin
// Check if DeX utilities are available
val available = SamsungDexUtils.isAvailable()

// Enable Meta key capture
SamsungDexUtils.setMetaKeyCapture(activity, true)

// Disable Meta key capture
SamsungDexUtils.setMetaKeyCapture(activity, false)

// Check if DeX mode is enabled
val dexEnabled = SamsungDexUtils.isDexEnabled(context)
```

#### MainActivity
```kotlin
// Toggle pointer capture
togglePointerCapture(true)  // Enable
togglePointerCapture(false) // Disable
```

### Dart/Flutter

#### Import
```dart
import 'package:flutter_hbb/common/android_utils.dart';
```

#### Usage
```dart
// Enable Samsung DeX Meta key capture
await AndroidUtils.setDexMetaCapture(true);

// Disable Samsung DeX Meta key capture
await AndroidUtils.setDexMetaCapture(false);

// Enable pointer capture
await AndroidUtils.togglePointerCapture(true);

// Disable pointer capture
await AndroidUtils.togglePointerCapture(false);

// Check if DeX is enabled
bool dexEnabled = await AndroidUtils.isDexEnabled();
if (dexEnabled) {
  print('Samsung DeX is active');
}
```

## Example: Adding to UI

To add these features to a Flutter UI, you could create toggle buttons:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/android_utils.dart';

class DexControlPanel extends StatefulWidget {
  @override
  _DexControlPanelState createState() => _DexControlPanelState();
}

class _DexControlPanelState extends State<DexControlPanel> {
  bool _metaCaptureEnabled = false;
  bool _pointerCaptureEnabled = false;
  bool _isDexEnabled = false;

  @override
  void initState() {
    super.initState();
    _checkDexStatus();
  }

  Future<void> _checkDexStatus() async {
    final dexEnabled = await AndroidUtils.isDexEnabled();
    setState(() {
      _isDexEnabled = dexEnabled;
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        if (_isDexEnabled)
          SwitchListTile(
            title: Text('DeX Meta Key Capture'),
            subtitle: Text('Capture Windows/Command key'),
            value: _metaCaptureEnabled,
            onChanged: (value) async {
              await AndroidUtils.setDexMetaCapture(value);
              setState(() {
                _metaCaptureEnabled = value;
              });
            },
          ),
        SwitchListTile(
          title: Text('Pointer Capture'),
          subtitle: Text('Immersive mouse control'),
          value: _pointerCaptureEnabled,
          onChanged: (value) async {
            await AndroidUtils.togglePointerCapture(value);
            setState(() {
              _pointerCaptureEnabled = value;
            });
          },
        ),
      ],
    );
  }
}
```

## Handling Pointer Events

When pointer capture is active, you need to handle pointer events differently in your Flutter widget:

```dart
Listener(
  onPointerMove: (PointerMoveEvent event) {
    if (_pointerCaptureEnabled) {
      // Use localDelta for raw relative movements
      final dx = event.localDelta.dx;
      final dy = event.localDelta.dy;
      
      // Send raw delta to remote desktop
      sendMouseDelta(dx, dy);
    } else {
      // Use absolute positioning
      final x = event.localPosition.dx;
      final y = event.localPosition.dy;
      
      // Send absolute position
      sendMousePosition(x, y);
    }
  },
  child: RemoteDesktopView(),
)
```

## Important Notes

1. **Samsung DeX**: Only works on Samsung devices with DeX mode. The implementation gracefully handles non-Samsung devices by checking availability.

2. **Pointer Capture**: 
   - Automatically releases when the window loses focus
   - Use `localDelta` property of pointer events when capture is active
   - Cursor is hidden while capture is active

3. **Error Handling**: All methods include proper error handling and will not crash on unsupported devices.

4. **Platform Check**: The Dart wrapper automatically checks `Platform.isAndroid` and returns early on non-Android platforms.

## Testing

To test these features:

1. **Samsung DeX**:
   - Connect a Samsung device to a monitor (DeX mode)
   - Enable Meta key capture
   - Press the Windows/Command key
   - Verify it's sent to the app instead of opening system menu

2. **Pointer Capture**:
   - Enable pointer capture
   - Move the mouse
   - Verify the cursor disappears and raw movement is captured
   - Click outside the app
   - Verify pointer capture is automatically released

## Troubleshooting

- **DeX features not working**: Ensure you're on a Samsung device with DeX support
- **Pointer capture not releasing**: Check that `onWindowFocusChanged` is being called properly
- **MethodChannel errors**: Verify the channel name matches ('mChannel')
