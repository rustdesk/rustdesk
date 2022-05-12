package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioRecord
import android.media.AudioRecord.READ_BLOCKING
import android.media.MediaCodecList
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.RequiresApi
import com.hjq.permissions.Permission
import com.hjq.permissions.XXPermissions
import java.nio.ByteBuffer
import java.util.*

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

fun requestPermission(context: Context, type: String) {
    val permission = when (type) {
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
        .request { permissions, all ->
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

fun checkPermission(context: Context, type: String): Boolean {
    val permission = when (type) {
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
