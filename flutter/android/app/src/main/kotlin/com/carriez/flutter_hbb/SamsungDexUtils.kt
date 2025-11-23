package com.carriez.flutter_hbb

import android.app.Activity
import android.content.ComponentName
import android.content.Context
import android.util.Log
import java.lang.reflect.Method

/**
 * Samsung DeX utilities for capturing Meta (Windows/Command) keys
 * and checking DeX mode status.
 * 
 * Adapted from termux-x11:
 * https://github.com/termux/termux-x11/blob/master/app/src/main/java/com/termux/x11/utils/SamsungDexUtils.java
 */
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

    /**
     * Check if Samsung DeX utilities are available on this device.
     */
    fun isAvailable(): Boolean = requestMetaKeyEventMethod != null && manager != null

    /**
     * Enable or disable Meta key capture for Samsung DeX.
     * When enabled, Meta (Windows/Command) key events will be sent to the app
     * instead of being intercepted by the system.
     */
    fun setMetaKeyCapture(activity: Activity, enable: Boolean) {
        if (!isAvailable()) return
        
        try {
            requestMetaKeyEventMethod?.invoke(manager, activity.componentName, enable)
            Log.d(TAG, "DeX Meta Key Capture set to: $enable")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to set DeX meta key capture", e)
        }
    }

    /**
     * Check if Samsung DeX mode is currently enabled.
     */
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
