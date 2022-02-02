package com.carriez.flutter_hbb

import android.Manifest
import android.app.*
import android.app.PendingIntent.FLAG_UPDATE_CURRENT
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.drawable.Icon
import android.media.MediaCodecList
import android.media.MediaFormat
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import java.util.*

val INFO = Info("","",0,0)

data class Info(var username:String, var hostname:String, var screenWidth:Int, var screenHeight:Int,
                var scale:Int = 1)


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
    return res!=null
}

fun createForegroundNotification(ctx:Service) {
    // 设置通知渠道 android8开始引入 老版本会被忽略 这个东西的作用相当于为通知分类 给用户选择通知消息的种类
    val channelId =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channelId = "RustDeskForeground"
            val channelName = "RustDesk屏幕分享服务状态"
            val channel = NotificationChannel(
                channelId,
                channelName, NotificationManager.IMPORTANCE_DEFAULT
            ).apply {
                description = "Share your Android Screen with RustDeskService"
            }
            channel.lightColor = Color.BLUE
            channel.lockscreenVisibility = Notification.VISIBILITY_PRIVATE
            val service = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            service.createNotificationChannel(channel)
            channelId
        } else {
            ""
        }

    val notification: Notification = NotificationCompat.Builder(ctx, channelId)
        .setOngoing(true)
        .setPriority(NotificationCompat.PRIORITY_LOW)
        .build()
    ctx.startForeground(11, notification)
}

fun createNormalNotification(ctx: Context,title:String,text:String,type:String): Notification {
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
    val intent = Intent(ctx,MainActivity::class.java).apply {
        flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED
        action =Intent.ACTION_MAIN // 不设置会造成每次都重新启动一个新的Activity
        addCategory(Intent.CATEGORY_LAUNCHER)
        putExtra("type",type)
    }
    val pendingIntent = PendingIntent.getActivity(ctx,0,intent,FLAG_UPDATE_CURRENT)
    return NotificationCompat.Builder(ctx, channelId)
        .setSmallIcon(R.mipmap.ic_launcher)
        .setContentTitle(title)
        .setPriority(NotificationCompat.PRIORITY_HIGH) // 这里如果设置为低则不显示
        .setContentText(text)
        .setContentIntent(pendingIntent)
        .setAutoCancel(true)
        .build()
}


const val MY_PERMISSIONS_REQUEST_READ_EXTERNAL_STORAGE = 1

fun checkPermissions(context: Context) {
    val permissions: MutableList<String> = LinkedList()
    addPermission(context,permissions, Manifest.permission.WRITE_EXTERNAL_STORAGE)
    addPermission(context,permissions, Manifest.permission.RECORD_AUDIO)
    addPermission(context,permissions, Manifest.permission.INTERNET)
    addPermission(context,permissions, Manifest.permission.READ_PHONE_STATE)
    if (permissions.isNotEmpty()) {
        ActivityCompat.requestPermissions(
            context as Activity, permissions.toTypedArray(),
            MY_PERMISSIONS_REQUEST_READ_EXTERNAL_STORAGE
        )
    }
}

private fun addPermission(context:Context,permissionList: MutableList<String>, permission: String) {
    if (ContextCompat.checkSelfPermission(
            context,
            permission
        ) !== PackageManager.PERMISSION_GRANTED
    ) {
        permissionList.add(permission)
    }
}