This guide details how to implement **Samsung DeX Meta Key Capture** and **Pointer Immersion (Mouse Capture)** in the RustDesk Android app. These features are derived from the `termux/termux-x11` implementation and adapted for a Flutter environment.

### Notes
* Adapted from repo https://github.com/termux/termux-x11.
* **ALWAYS** consult the original repo for its implementation.
* You **MUST** always save your progress to serena Memory. Intermediate steps also!

### 1. Samsung DeX Key Remapping (Meta Key Capture)

Samsung DeX intercepts the "Meta" (Windows/Command) key for system shortcuts (e.g., opening the app drawer). To pass this key to the remote desktop instead, you must use a Samsung-specific hidden API via Java reflection.

#### How Termux-X11 does it
Termux-X11 uses `SemWindowManager` via reflection to avoid linking against proprietary Samsung SDKs.

**Source File:** `app/src/main/java/com/termux/x11/utils/SamsungDexUtils.java`

#### Implementation for RustDesk (Kotlin)

In RustDesk's Flutter Android project (`flutter/android/app/src/main/kotlin/.../MainActivity.kt`), you need to add a helper object to handle this reflection.

```kotlin
package com.rustdesk.rustdesk

import android.app.Activity
import android.content.ComponentName
import android.content.Context
import android.util.Log
import java.lang.reflect.Method

object SamsungDexUtils {
    private const val TAG = "SamsungDexUtils"
    private var requestMetaKeyEventMethod: Method? = null
    private var manager: Any? = null

    init {
        try {
            // Reflect into Samsung's internal window manager
            val clazz = Class.forName("com.samsung.android.view.SemWindowManager")
            val getInstance = clazz.getMethod("getInstance")
            requestMetaKeyEventMethod = clazz.getDeclaredMethod(
                "requestMetaKeyEvent", 
                ComponentName::class.java, 
                Boolean::class.javaPrimitiveType
            )
            manager = getInstance.invoke(null)
            Log.d(TAG, "SemWindowManager loaded successfully")
        } catch (e: Exception) {
            Log.d(TAG, "SemWindowManager not found: ${e.message}")
        }
    }

    fun isAvailable(): Boolean = requestMetaKeyEventMethod != null && manager != null

    fun setMetaKeyCapture(activity: Activity, enable: Boolean) {
        if (!isAvailable()) return
        
        try {
            requestMetaKeyEventMethod?.invoke(manager, activity.componentName, enable)
            Log.d(TAG, "DeX Meta Key Capture set to: $enable")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to set DeX meta key capture", e)
        }
    }

    fun isDexEnabled(context: Context): Boolean {
        val config = context.resources.configuration
        return try {
            val c = config.javaClass
            // Check specific DeX configuration fields
            val semDesktopModeEnabled = c.getField("semDesktopModeEnabled").getInt(config)
            val SEM_DESKTOP_MODE_ENABLED = c.getField("SEM_DESKTOP_MODE_ENABLED").getInt(c)
            semDesktopModeEnabled == SEM_DESKTOP_MODE_ENABLED
        } catch (e: Exception) {
            false
        }
    }
}
```

### 2. Pointer Immersion (Mouse Capture)

Pointer Capture allows the app to take full control of the mouse pointer, hiding it and receiving raw relative movements (deltas) instead of absolute touch coordinates. This is critical for 3D games or panning remote screens without hitting the edge of the Android screen.

#### How Termux-X11 does it
It calls the standard Android API `view.requestPointerCapture()` on its primary SurfaceView.

**Source File:** `app/src/main/java/com/termux/x11/input/TouchInputHandler.java`

#### Implementation for RustDesk

In Flutter, the main view is a `FlutterView`. You can request pointer capture on the window's root view.

Add this logic to your `MainActivity.kt`:

```kotlin
private fun togglePointerCapture(enable: Boolean) {
    val view = window.decorView
    if (enable) {
        view.requestPointerCapture()
    } else {
        view.releasePointerCapture()
    }
}
```

### 3. Connecting to Flutter (MethodChannel)

You must bridge these native functions to Dart so the RustDesk UI can toggle them (e.g., via a "Immersive Mode" button in the session menu).

**1. Kotlin Side (`MainActivity.kt`)**

```kotlin
class MainActivity: FlutterActivity() {
    private val CHANNEL = "com.rustdesk.rustdesk/android_features"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL).setMethodCallHandler { call, result ->
            when (call.method) {
                "setDexMetaCapture" -> {
                    val enable = call.argument<Boolean>("enable") ?: false
                    SamsungDexUtils.setMetaKeyCapture(this, enable)
                    result.success(null)
                }
                "togglePointerCapture" -> {
                    val enable = call.argument<Boolean>("enable") ?: false
                    togglePointerCapture(enable)
                    result.success(null)
                }
                "isDexEnabled" -> {
                    result.success(SamsungDexUtils.isDexEnabled(this))
                }
                else -> result.notImplemented()
            }
        }
    }
    
    // Automatically release capture if the user forces focus away
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) {
            // Optional: Tell Flutter that capture was lost so UI can update
        }
    }
}
```

**2. Dart Side (`flutter/lib/common/android_utils.dart`)**

Create a utility class in your Flutter code to call these methods.

```dart
import 'dart:io';
import 'package:flutter/services.dart';

class AndroidUtils {
  static const platform = MethodChannel('com.rustdesk.rustdesk/android_features');

  static Future<void> setDexMetaCapture(bool enable) async {
    if (!Platform.isAndroid) return;
    try {
      await platform.invokeMethod('setDexMetaCapture', {'enable': enable});
    } on PlatformException catch (e) {
      print("Failed to set DeX capture: '${e.message}'.");
    }
  }

  static Future<void> togglePointerCapture(bool enable) async {
    if (!Platform.isAndroid) return;
    try {
      await platform.invokeMethod('togglePointerCapture', {'enable': enable});
    } on PlatformException catch (e) {
      print("Failed to toggle pointer capture: '${e.message}'.");
    }
  }
  
  static Future<bool> isDexEnabled() async {
    if (!Platform.isAndroid) return false;
    try {
      return await platform.invokeMethod('isDexEnabled');
    } catch (_) {
      return false;
    }
  }
}
```

### 4. Handling Mouse Events in Flutter

Once `requestPointerCapture()` is active, Android sends events differently. 

1.  **Relative Events**: Flutter's `PointerHoverEvent` or `PointerMoveEvent` usually report absolute coordinates. When captured, Android reports `AXIS_RELATIVE_X` and `AXIS_RELATIVE_Y`.
2.  **Flutter Support**: Ensure you are using a version of Flutter that supports `PointerSignalEvent` or raw pointer data correctly on Android.
3.  **Listening to Events**:
    In your main connection view (likely a `Listener` or `MouseRegion` widget), you will need to handle the delta movements.

```dart
Listener(
  onPointerMove: (PointerMoveEvent event) {
    // If pointer capture is active, use localDelta
    // localDelta corresponds to raw mouse movement when captured
    final dx = event.localDelta.dx;
    final dy = event.localDelta.dy;
    
    if (isPointerCaptured) {
       // Send raw delta to Rust core
       sendMouseDelta(dx, dy); 
    } else {
       // Standard absolute positioning logic
    }
  },
  child: RemoteDesktopView(),
)
```

### Summary Checklist
1.  [ ] **Copy `SamsungDexUtils` logic** into your Android Kotlin project.
2.  [ ] **Setup MethodChannel** in `MainActivity.kt` to expose DeX capture and Pointer capture.
3.  [ ] **Create Dart Wrapper** to invoke these methods.
4.  [ ] **Update Input Logic**: Modify RustDesk's input listener to handle `localDelta` when capture is active, as absolute coordinates may stop updating or locking to the center during capture.
