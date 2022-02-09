package com.carriez.flutter_hbb

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

fun createNormalNotification(
    ctx: Context,
    title: String,
    text: String,
    type: String
): Notification {
    val channelId =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channelId = "RustDeskNormal"
            val channelName = "RustDesk通知消息"
            val channel = NotificationChannel(
                channelId,
                channelName, NotificationManager.IMPORTANCE_DEFAULT
            ).apply {
                description = "Share your Android Screen with RustDeskService"
            }
            channel.lightColor = Color.BLUE
            channel.lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            val service = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            service.createNotificationChannel(channel)
            channelId
        } else {
            ""
        }
    val intent = Intent(ctx, MainActivity::class.java).apply {
        flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED
        action = Intent.ACTION_MAIN // 不设置会造成每次都重新启动一个新的Activity
        addCategory(Intent.CATEGORY_LAUNCHER)
        putExtra("type", type)
    }
    val pendingIntent = PendingIntent.getActivity(ctx, 0, intent, FLAG_UPDATE_CURRENT)
    return NotificationCompat.Builder(ctx, channelId)
        .setSmallIcon(R.mipmap.ic_launcher)
        .setContentTitle(title)
        .setPriority(NotificationCompat.PRIORITY_HIGH) // 这里如果设置为低则不显示
        .setContentText(text)
        .setContentIntent(pendingIntent)
        .setAutoCancel(true)
        .build()
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