# DeX Pointer Capture Investigation - FINAL FIX APPLIED

## Issue
After enabling DeX Optimization, hardware mouse and keyboard stop working.

## Root Cause (Confirmed from Termux-X11 analysis)

### Key Finding from Termux-X11 (TouchInputHandler.java lines 863-892)
When pointer capture is enabled:
```java
if (!v.hasPointerCapture()) {
    // Normal: Use absolute coordinates
    mInjector.sendCursorMove(scaledX, scaledY, false);
} else if (e.getAction() == MotionEvent.ACTION_MOVE) {
    // Captured: Use AXIS_RELATIVE_X/Y - relative movement deltas
    float x = e.getAxisValue(MotionEvent.AXIS_RELATIVE_X);
    float y = e.getAxisValue(MotionEvent.AXIS_RELATIVE_Y);
    mInjector.sendCursorMove(x, y, true);  // true = relative
}
```

### Why Our Implementation Broke
1. When `requestPointerCapture()` is called, Android changes how mouse events are delivered
2. Flutter expects absolute coordinates in `PointerMoveEvent.localPosition`
3. Pointer capture provides relative deltas instead
4. Result: cursor appears "stuck" because Flutter doesn't handle relative mode

### Key Insight from Termux-X11
**Meta key capture and pointer capture are SEPARATE features!**
- `dexMetaKeyCapture` - Only affects Windows/Meta key routing
- `pointerCapture` - Changes ALL mouse event delivery

## Solution Applied

### Changes Made
1. **toolbar.dart** - Only call `setDexMetaCapture(value)`, removed `togglePointerCapture` calls
2. **platform_channel.dart** - Removed `togglePointerCapture` method with explanatory comment
3. **MainActivity.kt** - Removed `togglePointerCapture` handler and function, removed related `onWindowFocusChanged` override

### What DeX Optimization Now Does
- ✅ Captures Meta/Windows key (prevents DeX from intercepting it)
- ✅ All keyboard keys work normally
- ✅ Mouse works with absolute coordinates (normal Flutter handling)
- ❌ No pointer capture (would require rewriting Flutter input handling)

### Files Modified
- `flutter/lib/common/widgets/toolbar.dart`
- `flutter/lib/utils/platform_channel.dart`
- `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainActivity.kt`

### Why This Is Correct
1. Remote desktop doesn't need pointer capture (cursor should be visible and trackable)
2. Meta key capture alone provides significant value for DeX users
3. Implementing proper pointer capture would require:
   - Native interception of MotionEvents
   - Converting AXIS_RELATIVE_X/Y to Flutter format
   - Modifying InputModel to handle relative movements
   - Too invasive for minimal benefit

## Verification
- Mouse should work normally after DeX Optimization is enabled
- Windows/Meta key should be captured and sent to remote desktop
- All other keyboard keys work as before