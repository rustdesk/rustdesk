package com.carriez.flutter_hbb

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.provider.Settings
import android.util.DisplayMetrics
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.core.app.NotificationManagerCompat
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

const val NOTIFY_TYPE_LOGIN_REQ = "NOTIFY_TYPE_LOGIN_REQ"
const val MEDIA_REQUEST_CODE = 42
const val INPUT_REQUEST_CODE = 43

class MainActivity : FlutterActivity() {
    companion object {
        lateinit var flutterMethodChannel: MethodChannel
    }

    private val channelTag = "mChannel"
    private val logTag = "mMainActivity"
    private var mediaProjectionResultIntent: Intent? = null

    init {
        System.loadLibrary("rustdesk")
    }

    private external fun init(context: Context)
    private external fun close()

    fun rustSetByName(name: String, arg1: String, arg2: String) {
        when (name) {
            "try_start_without_auth" -> {
                // to UI
                Log.d(logTag, "from rust:got try_start_without_auth")
                activity.runOnUiThread {
                    flutterMethodChannel.invokeMethod(name, mapOf("peerID" to arg1, "name" to arg2))
                    Log.d(logTag, "activity.runOnUiThread invokeMethod try_start_without_auth,done")
                }
                val notification = createNormalNotification(
                    this,
                    "请求控制",
                    "来自$arg1:$arg2 请求连接",
                    NOTIFY_TYPE_LOGIN_REQ
                )
                with(NotificationManagerCompat.from(this)) {
                    notify(12, notification)
                }
                Log.d(logTag, "kotlin invokeMethod try_start_without_auth,done")
            }
            "start_capture" -> {
                Log.d(logTag, "from rust:start_capture")
                activity.runOnUiThread {
                    flutterMethodChannel.invokeMethod(name, mapOf("peerID" to arg1, "name" to arg2))
                    Log.d(logTag, "activity.runOnUiThread invokeMethod try_start_without_auth,done")
                }
                // 1.开始捕捉音视频 2.通知栏
                startCapture()
                val notification = createNormalNotification(
                    this,
                    "开始共享屏幕",
                    "From:$arg2:$arg1",
                    NOTIFY_TYPE_START_CAPTURE
                )
                with(NotificationManagerCompat.from(this)) {
                    notify(13, notification)
                }
            }
            "stop_capture" -> {
                Log.d(logTag, "from rust:stop_capture")
                stopCapture()
                activity.runOnUiThread {
                    flutterMethodChannel.invokeMethod(name, null)
                    Log.d(logTag, "activity.runOnUiThread invokeMethod try_start_without_auth,done")
                }
            }
            else -> {}
        }
    }

    override fun onDestroy() {
        Log.e(logTag, "onDestroy")
        close()
        stopCapture()
        stopMainService()
        stopService(Intent(this, MainService::class.java))
        stopService(Intent(this, InputService::class.java))
        super.onDestroy()
    }

    @RequiresApi(Build.VERSION_CODES.M)
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine) // 必要 否则无法正确初始化flutter
        checkPermissions(this)
        updateMachineInfo()
        flutterMethodChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            channelTag
        ).apply {
            setMethodCallHandler { call, result ->
                when (call.method) {
                    "init_service" -> {
                        Log.d(logTag, "event from flutter,getPer")
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                            getMediaProjection()
                        }
                        result.success(true)
                    }
                    "start_capture" -> {
                        startCapture()
                        result.success(true)
                    }
                    "stop_service" -> {
                        stopMainService()
                        result.success(true)
                    }
                    "check_input" -> {
                        checkInput()
                        result.success(true)
                    }
                    "check_video_permission" -> {
                        val res = MainService.checkMediaPermission()
                        result.success(res)
                    }
                    else -> {}
                }
            }
        }
    }

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun getMediaProjection() {
        val mMediaProjectionManager =
            getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        val mIntent = mMediaProjectionManager.createScreenCaptureIntent()
        startActivityForResult(mIntent, MEDIA_REQUEST_CODE)
    }

    // 实际逻辑是开始监听服务 在成功获取到mediaProjection就开始
    private fun initService() {
        if (mediaProjectionResultIntent == null) {
            Log.w(logTag, "initService fail,mediaProjectionResultIntent is null")
            return
        }
        Log.d(logTag, "Init service")
        // call init service to rust
        init(this)
        val serviceIntent = Intent(this, MainService::class.java)
        serviceIntent.action = INIT_SERVICE
        serviceIntent.putExtra(EXTRA_MP_DATA, mediaProjectionResultIntent)

        launchMainService(serviceIntent)
    }

    private fun startCapture() {
        if (mediaProjectionResultIntent == null) {
            Log.w(logTag, "startCapture fail,mediaProjectionResultIntent is null")
            return
        }
        Log.d(logTag, "Start Capture")
        val serviceIntent = Intent(this, MainService::class.java)
        serviceIntent.action = START_CAPTURE
        serviceIntent.putExtra(EXTRA_MP_DATA, mediaProjectionResultIntent)

        launchMainService(serviceIntent)
    }

    private fun stopCapture() {
        Log.d(logTag, "Stop Capture")
        val serviceIntent = Intent(this, MainService::class.java)
        serviceIntent.action = STOP_CAPTURE

        launchMainService(serviceIntent)
    }

    // TODO 关闭逻辑
    private fun stopMainService() {
        Log.d(logTag, "Stop service")
        val serviceIntent = Intent(this, MainService::class.java)
        serviceIntent.action = STOP_SERVICE
        launchMainService(serviceIntent)
    }

    private fun launchMainService(intent: Intent) {
        // TEST api < O
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(intent)
        } else {
            startService(intent)
        }
    }

    private fun checkInput() {
        val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)
        if (intent.resolveActivity(packageManager) != null) {
            startActivity(intent)
        }
    }

    override fun onResume() {
        super.onResume()
        val inputPer = InputService.isOpen()
        Log.d(logTag,"onResume inputPer:$inputPer")
        activity.runOnUiThread {
            flutterMethodChannel.invokeMethod("on_permission_changed",mapOf("name" to "input", "value" to inputPer.toString()))
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == MEDIA_REQUEST_CODE && resultCode == Activity.RESULT_OK && data != null) {
            Log.d(logTag, "got mediaProjectionResultIntent ok")
            mediaProjectionResultIntent = data
            initService()
        }
    }

    private fun updateMachineInfo() {
        // 屏幕尺寸 控制最长边不超过1400 超过则减半并储存缩放比例 实际发送给手机端的尺寸为缩小后的尺寸
        // input控制时再通过缩放比例恢复原始尺寸进行path入参
        val dm = DisplayMetrics()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            display?.getRealMetrics(dm)
        } else {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.JELLY_BEAN_MR1) {
                @Suppress("DEPRECATION")
                windowManager.defaultDisplay.getRealMetrics(dm)
            } else {
                @Suppress("DEPRECATION")
                windowManager.defaultDisplay.getMetrics(dm)
            }
        }
        var w = dm.widthPixels
        var h = dm.heightPixels
        var scale = 1
        if (w != 0 && h != 0) {
            if (w > MAX_SCREEN_SIZE || h > MAX_SCREEN_SIZE) {
                scale = 2
                w /= scale
                h /= scale
            }
            Log.d(logTag, "Real size - width:$w,height:$h")

            INFO.screenWidth = w
            INFO.screenHeight = h
            INFO.scale = scale
            INFO.username = "test"
            INFO.hostname = "hostname"
            // TODO  username hostname

        } else {
            Log.e(logTag, "Got Screen Size Fail!")
        }
    }
}
