package com.carriez.flutter_hbb

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat

/**
 * Keeps the Rust/Flutter process alive while an outgoing TCP tunnel is active.
 *
 * The TCP listener and RustDesk connection live in the Rust core. This service
 * deliberately does not use VPNService or MediaProjection.
 */
class TunnelService : Service() {
    companion object {
        private const val CHANNEL_ID = "RustDeskTunnel"
        private const val NOTIFICATION_ID = 43071
        @Volatile
        var isRunning: Boolean = false
            private set
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        isRunning = true
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val description = intent?.getStringExtra("description")
            ?.takeIf { it.isNotBlank() }
            ?: "TCP tunnel is running"
        startForeground(NOTIFICATION_ID, buildNotification(description))
        isRunning = true
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        isRunning = false
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    "RustDesk TCP Tunnel",
                    NotificationManager.IMPORTANCE_LOW
                ).apply {
                    description = "Keeps outgoing RustDesk TCP tunnels active"
                    setShowBadge(false)
                }
            )
        }
    }

    private fun buildNotification(description: String): Notification {
        val launchIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val pendingIntentFlags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            launchIntent,
            pendingIntentFlags
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.mipmap.ic_stat_logo)
            .setContentTitle("RustDesk TCP Tunnel")
            .setContentText(description)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setContentIntent(pendingIntent)
            .build()
    }
}
