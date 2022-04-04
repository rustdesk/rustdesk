package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.content.Context
import android.media.MediaCodecList
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.RequiresApi
import com.hjq.permissions.Permission
import com.hjq.permissions.XXPermissions
import java.util.*

@SuppressLint("ConstantLocale")
val LOCAL_NAME = Locale.getDefault().toString()

val INFO = Info("", "", 0, 0)

data class Info(
    var username: String, var hostname: String, var screenWidth: Int, var screenHeight: Int,
    var scale: Int = 1
)

@RequiresApi(Build.VERSION_CODES.LOLLIPOP)
fun testVP9Support(): Boolean {
    return true  // 函数内部永远返回true 暂时只使用原始数据
    val res = MediaCodecList(MediaCodecList.ALL_CODECS)
        .findEncoderForFormat(
            MediaFormat.createVideoFormat(
                MediaFormat.MIMETYPE_VIDEO_VP9,
                INFO.screenWidth,
                INFO.screenWidth
            )
        )
    return res != null
}

fun requestPermission(context: Context,type: String){
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
                Log.d("checkPermissions", "获取存储权限成功：$permissions")
                Handler(Looper.getMainLooper()).post {
                    MainActivity.flutterMethodChannel.invokeMethod(
                        "on_android_permission_result",
                        mapOf("type" to type, "result" to all)
                    )
                }
            }
        }
}

fun checkPermission(context: Context,type: String): Boolean {
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
    return XXPermissions.isGranted(context,permission)
}
