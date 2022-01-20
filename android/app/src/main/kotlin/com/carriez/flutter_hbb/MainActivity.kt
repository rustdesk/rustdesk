package com.carriez.flutter_hbb

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.os.PersistableBundle
import android.util.Log
import androidx.annotation.RequiresApi
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.nio.ByteBuffer
import kotlin.concurrent.thread


class MainActivity : FlutterActivity() {
    private val channelTag = "mChannel"
    private var mediaProjectionResultIntent: Intent? = null
    private val requestCode = 1
    private val buf = ByteBuffer.allocate(16)

    init {
        System.loadLibrary("rustdesk")
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine) // 必要 否则无法正确初始化flutter

        MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            channelTag
        ).setMethodCallHandler { call, result ->
            when (call.method) {
                "getPer" -> {
                    Log.d(channelTag, "event from flutter,getPer")
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                        getMediaProjection()
                    }
                    result.success(true)
                }
                "startSer" -> {
                    mStarService()
                    result.success(true)
                }
                "stopSer" -> {
                    mStopService()
                    result.success(true)
                }
                else -> {}
            }
        }
    }


    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun getMediaProjection() {
        val mMediaProjectionManager =
            getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        val mIntent = mMediaProjectionManager.createScreenCaptureIntent()
        startActivityForResult(mIntent, requestCode)
    }

    private fun mStarService() {
        if (mediaProjectionResultIntent == null) {
            Log.w(channelTag, "mediaProjectionResultIntent is null")
            return
        }
        Log.d(channelTag, "Start a service")
        val serviceIntent = Intent(this, MainService::class.java)
        serviceIntent.action = START_SERVICE
        serviceIntent.putExtra(EXTRA_MP_DATA, mediaProjectionResultIntent)

        // TEST api < O
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent)
        } else {
            startService(serviceIntent)
        }
    }

    private fun mStopService() {
        Log.d(channelTag, "Stop service")
        val serviceIntent = Intent(this, MainService::class.java)

        serviceIntent.action = STOP_SERVICE

        // TEST api < O
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent)
        } else {
            startService(serviceIntent)
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (resultCode == Activity.RESULT_OK && data != null) {
            Log.d(channelTag, "got mediaProjectionResultIntent ok")
            mediaProjectionResultIntent = data
        }
    }
}
