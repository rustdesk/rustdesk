package com.carriez.flutter_hbb

/**
 * Handle events from flutter
 * Request MediaProjection permission
 *
 * Inspired by [droidVNC-NG] https://github.com/bk138/droidVNC-NG
 */

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Build
import android.os.IBinder
import android.provider.Settings
import android.util.Log
import android.view.WindowManager
import androidx.annotation.RequiresApi
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel


class MainActivity : FlutterActivity() {
    companion object {
        lateinit var flutterMethodChannel: MethodChannel
    }

    private val channelTag = "mChannel"
    private val logTag = "mMainActivity"
    private var mainService: MainService? = null

    @RequiresApi(Build.VERSION_CODES.M)
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        if (MainService.isReady) {
            Intent(activity, MainService::class.java).also {
                bindService(it, serviceConnection, Context.BIND_AUTO_CREATE)
            }
        }
        flutterMethodChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            channelTag
        ).apply {
            // make sure result is set, otherwise flutter will await forever
            setMethodCallHandler { call, result ->
                when (call.method) {
                    "init_service" -> {
                        Intent(activity, MainService::class.java).also {
                            bindService(it, serviceConnection, Context.BIND_AUTO_CREATE)
                        }
                        if (MainService.isReady) {
                            result.success(false)
                            return@setMethodCallHandler
                        }
                        requestMediaProjection()
                        result.success(true)
                    }
                    "start_capture" -> {
                        mainService?.let {
                            result.success(it.startCapture())
                        } ?: let {
                            result.success(false)
                        }
                    }
                    "stop_service" -> {
                        Log.d(logTag, "Stop service")
                        mainService?.let {
                            it.destroy()
                            result.success(true)
                        } ?: let {
                            result.success(false)
                        }
                    }
                    "check_permission" -> {
                        if (call.arguments is String) {
                            result.success(checkPermission(context, call.arguments as String))
                        } else {
                            result.success(false)
                        }
                    }
                    "request_permission" -> {
                        if (call.arguments is String) {
                            requestPermission(context, call.arguments as String)
                            result.success(true)
                        } else {
                            result.success(false)
                        }
                    }
                    "check_video_permission" -> {
                        mainService?.let {
                            result.success(it.checkMediaPermission())
                        } ?: let {
                            result.success(false)
                        }
                    }
                    "check_service" -> {
                        flutterMethodChannel.invokeMethod(
                            "on_state_changed",
                            mapOf("name" to "input", "value" to InputService.isOpen.toString())
                        )
                        flutterMethodChannel.invokeMethod(
                            "on_state_changed",
                            mapOf("name" to "media", "value" to MainService.isReady.toString())
                        )
                        result.success(true)
                    }
                    "init_input" -> {
                        initInput()
                        result.success(true)
                    }
                    "stop_input" -> {
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                            InputService.ctx?.disableSelf()
                        }
                        InputService.ctx = null
                        flutterMethodChannel.invokeMethod(
                            "on_state_changed",
                            mapOf("name" to "input", "value" to InputService.isOpen.toString())
                        )
                        result.success(true)
                    }
                    "cancel_notification" -> {
                        try {
                            val id = call.arguments as Int
                            mainService?.cancelNotification(id)
                        } finally {
                            result.success(true)
                        }
                    }
                    "enable_soft_keyboard" -> {
                        // https://blog.csdn.net/hanye2020/article/details/105553780
                        try {
                            if (call.arguments as Boolean) {
                                window.clearFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
                            } else {
                                window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
                            }
                        } finally {
                            result.success(true)
                        }
                    }
                    else -> {
                        result.error("-1", "No such method", null)
                    }
                }
            }
        }
    }

    private fun initInput() {
        val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)
        if (intent.resolveActivity(packageManager) != null) {
            startActivity(intent)
        }
    }

    override fun onResume() {
        super.onResume()
        val inputPer = InputService.isOpen
        activity.runOnUiThread {
            flutterMethodChannel.invokeMethod(
                "on_state_changed",
                mapOf("name" to "input", "value" to inputPer.toString())
            )
        }
    }

    private fun requestMediaProjection() {
        val intent = Intent(this, PermissionRequestTransparentActivity::class.java).apply {
            action = ACT_REQUEST_MEDIA_PROJECTION
        }
        startActivityForResult(intent, REQ_INVOKE_PERMISSION_ACTIVITY_MEDIA_PROJECTION)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQ_INVOKE_PERMISSION_ACTIVITY_MEDIA_PROJECTION && resultCode == RES_FAILED) {
            flutterMethodChannel.invokeMethod("on_media_projection_canceled", null)
        }
    }

    override fun onDestroy() {
        Log.e(logTag, "onDestroy")
        mainService?.let {
            unbindService(serviceConnection)
        }
        super.onDestroy()
    }

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            Log.d(logTag, "onServiceConnected")
            val binder = service as MainService.LocalBinder
            mainService = binder.getService()
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.d(logTag, "onServiceDisconnected")
            mainService = null
        }
    }
}
