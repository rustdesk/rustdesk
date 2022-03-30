package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.app.*
import android.app.PendingIntent.FLAG_UPDATE_CURRENT
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.media.MediaCodecList
import android.media.MediaFormat
import android.os.Build
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.core.app.NotificationCompat
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

fun checkPermissions(context: Context) {
    XXPermissions.with(context)
        .permission(Permission.RECORD_AUDIO)
        .permission(Permission.MANAGE_EXTERNAL_STORAGE)
        .request { permissions, all ->
            if (all) {
                Log.d("loglog", "获取存储权限成功：$permissions")
            }
        }
}
