package com.carriez.flutter_hbb

import android.Manifest.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
import android.Manifest.permission.SYSTEM_ALERT_WINDOW
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.widget.Toast
import com.hjq.permissions.XXPermissions
import io.flutter.embedding.android.FlutterActivity
import kotlin.concurrent.thread

const val DEBUG_BOOT_COMPLETED = "com.carriez.flutter_hbb.DEBUG_BOOT_COMPLETED"

class BootReceiver : BroadcastReceiver() {
    private val logTag = "tagBootReceiver"

    override fun onReceive(context: Context, intent: Intent) {
        Log.d(logTag, "onReceive ${intent.action}")

        if (Intent.ACTION_BOOT_COMPLETED == intent.action || DEBUG_BOOT_COMPLETED == intent.action) {
            // check SharedPreferences config
            val prefs = context.getSharedPreferences(KEY_SHARED_PREFERENCES, FlutterActivity.MODE_PRIVATE)
            if (!prefs.getBoolean(KEY_START_ON_BOOT_OPT, false)) {
                Log.d(logTag, "KEY_START_ON_BOOT_OPT is false")
                return
            }
            // check pre-permission
            if (!XXPermissions.isGranted(context, REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, SYSTEM_ALERT_WINDOW)){
                Log.d(logTag, "REQUEST_IGNORE_BATTERY_OPTIMIZATIONS or SYSTEM_ALERT_WINDOW is not granted")
                return
            }

            startWithReadyLogs(context) {
                val it = Intent(context, MainService::class.java).apply {
                    action = ACT_INIT_MEDIA_PROJECTION_AND_SERVICE
                    putExtra(EXT_INIT_FROM_BOOT, true)
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(it)
                } else {
                    context.startService(it)
                }
                Handler(Looper.getMainLooper()).post {
                    Toast.makeText(context, "RustDesk is Open", Toast.LENGTH_LONG).show()
                }
            }
        }
    }

    private fun startWithReadyLogs(context: Context, startService: () -> Unit) {
        val pendingResult = goAsync()
        thread {
            try {
                val logs = (context.applicationContext as MainApplication).documentStorage.logs
                if (!logs.ensureReady()) {
                    Log.e(logTag, "Boot startup requires writable logs in ${logs.directoryPath}. Open the app to grant access.")
                    return@thread
                }
                startService()
            } catch (error: Exception) {
                Log.e(logTag, "Unable to prepare logs or start the service at boot", error)
            } finally {
                pendingResult.finish()
            }
        }
    }
}
