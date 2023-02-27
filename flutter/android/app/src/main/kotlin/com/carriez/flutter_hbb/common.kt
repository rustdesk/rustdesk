package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.media.AudioRecord
import android.media.AudioRecord.READ_BLOCKING
import android.media.MediaCodecList
import android.media.MediaFormat
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings.*
import androidx.annotation.RequiresApi
import androidx.core.content.ContextCompat.getSystemService
import com.hjq.permissions.Permission
import com.hjq.permissions.XXPermissions
import java.nio.ByteBuffer
import java.util.*


// intent action, extra
const val ACT_REQUEST_MEDIA_PROJECTION = "REQUEST_MEDIA_PROJECTION"
const val ACT_INIT_MEDIA_PROJECTION_AND_SERVICE = "INIT_MEDIA_PROJECTION_AND_SERVICE"
const val ACT_LOGIN_REQ_NOTIFY = "LOGIN_REQ_NOTIFY"
const val EXT_MEDIA_PROJECTION_RES_INTENT = "MEDIA_PROJECTION_RES_INTENT"
const val EXT_LOGIN_REQ_NOTIFY = "LOGIN_REQ_NOTIFY"

// Activity requestCode
const val REQ_INVOKE_PERMISSION_ACTIVITY_MEDIA_PROJECTION = 101
const val REQ_REQUEST_MEDIA_PROJECTION = 201

// Activity responseCode
const val RES_FAILED = -100


@SuppressLint("ConstantLocale")
val LOCAL_NAME = Locale.getDefault().toString()
val SCREEN_INFO = Info(0, 0, 1, 200)

data class Info(
    var width: Int, var height: Int, var scale: Int, var dpi: Int
)

@RequiresApi(Build.VERSION_CODES.LOLLIPOP)
fun testVP9Support(): Boolean {
    return true
    val res = MediaCodecList(MediaCodecList.ALL_CODECS)
        .findEncoderForFormat(
            MediaFormat.createVideoFormat(
                MediaFormat.MIMETYPE_VIDEO_VP9,
                SCREEN_INFO.width,
                SCREEN_INFO.width
            )
        )
    return res != null
}

@RequiresApi(Build.VERSION_CODES.M)
fun requestPermission(context: Context, type: String) {
    val permission = when (type) {
        "ignore_battery_optimizations" -> {
            try {
                context.startActivity(Intent(ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                    data = Uri.parse("package:" + context.packageName)
                })
            } catch (e:Exception) {
                e.printStackTrace()
            }
            return
        }
        "application_details_settings" -> {
            try {
                context.startActivity(Intent(ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    data = Uri.parse("package:" + context.packageName)
                })
            } catch (e:Exception) {
                e.printStackTrace()
            }
            return
        }
        "audio" -> {
            Permission.RECORD_AUDIO
        }
        "file" -> {
            Permission.MANAGE_EXTERNAL_STORAGE
        }
        else -> {
            return
        }
    }
    XXPermissions.with(context)
        .permission(permission)
        .request { _, all ->
            if (all) {
                Handler(Looper.getMainLooper()).post {
                    MainActivity.flutterMethodChannel.invokeMethod(
                        "on_android_permission_result",
                        mapOf("type" to type, "result" to all)
                    )
                }
            }
        }
}

@RequiresApi(Build.VERSION_CODES.M)
fun checkPermission(context: Context, type: String): Boolean {
    val permission = when (type) {
        "ignore_battery_optimizations" -> {
            val pw = context.getSystemService(Context.POWER_SERVICE) as PowerManager
            return pw.isIgnoringBatteryOptimizations(context.packageName)
        }
        "audio" -> {
            Permission.RECORD_AUDIO
        }
        "file" -> {
            Permission.MANAGE_EXTERNAL_STORAGE
        }
        else -> {
            return false
        }
    }
    return XXPermissions.isGranted(context, permission)
}

class AudioReader(val bufSize: Int, private val maxFrames: Int) {
    private var currentPos = 0
    private val bufferPool: Array<ByteBuffer>

    init {
        if (maxFrames < 0 || maxFrames > 32) {
            throw Exception("Out of bounds")
        }
        if (bufSize <= 0) {
            throw Exception("Wrong bufSize")
        }
        bufferPool = Array(maxFrames) {
            ByteBuffer.allocateDirect(bufSize)
        }
    }

    private fun next() {
        currentPos++
        if (currentPos >= maxFrames) {
            currentPos = 0
        }
    }

    @RequiresApi(Build.VERSION_CODES.M)
    fun readSync(audioRecord: AudioRecord): ByteBuffer? {
        val buffer = bufferPool[currentPos]
        val res = audioRecord.read(buffer, bufSize, READ_BLOCKING)
        return if (res > 0) {
            next()
            buffer
        } else {
            null
        }
    }
}
