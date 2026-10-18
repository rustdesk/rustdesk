package com.carriez.flutter_hbb

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

/**
 * Holds the process in the foreground for an outgoing session.
 *
 * With the screen off, Android suspends network and DNS for a background
 * app. The session then dies on `failed to lookup address information`.
 * A foreground service plus a partial wake lock and a wifi lock keeps that
 * lookup working until the session ends.
 */
class OutgoingConnectionService : Service() {
    private var foreground = false
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "rustdesk:outgoing"
        ).apply { setReferenceCounted(false) }
        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        @Suppress("DEPRECATION")
        val wifiMode = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            WifiManager.WIFI_MODE_FULL_LOW_LATENCY
        } else {
            WifiManager.WIFI_MODE_FULL_HIGH_PERF
        }
        wifiLock = wifiManager?.createWifiLock(wifiMode, "rustdesk:outgoing")?.apply {
            setReferenceCounted(false)
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val active = synchronized(lock) { requested > 0 }
        try {
            if (active) {
                enterForeground()
            } else if (!foreground) {
                // startForegroundService still has to reach startForeground when the
                // session ended before this command ran.
                enterForeground()
                shutdown()
            } else {
                shutdown()
            }
        } catch (error: Exception) {
            Log.e(logTag, "Failed to hold outgoing connection", error)
            shutdown()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        releaseLocks()
        foreground = false
        super.onDestroy()
    }

    private fun enterForeground() {
        val notification = buildNotification()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        foreground = true
        acquireLocks()
    }

    private fun shutdown() {
        releaseLocks()
        if (foreground) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                stopForeground(STOP_FOREGROUND_REMOVE)
            } else {
                @Suppress("DEPRECATION")
                stopForeground(true)
            }
            foreground = false
        }
        stopSelf()
    }

    private fun acquireLocks() {
        wakeLock?.let { if (!it.isHeld) it.acquire() }
        wifiLock?.let { if (!it.isHeld) it.acquire() }
    }

    private fun releaseLocks() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wifiLock?.let { if (it.isHeld) it.release() }
    }

    private fun buildNotification(): Notification {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "RustDesk",
                NotificationManager.IMPORTANCE_LOW
            )
            manager.createNotificationChannel(channel)
        }
        val launch = packageManager.getLaunchIntentForPackage(packageName)
        val pendingFlags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }
        val pending = launch?.let {
            PendingIntent.getActivity(this, 0, it, pendingFlags)
        }
        val text = try {
            translate("Connected")
        } catch (error: Exception) {
            Log.w(logTag, "translate failed", error)
            "Connected"
        }
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setOngoing(true)
            .setSmallIcon(R.mipmap.ic_stat_logo)
            .setContentTitle(DEFAULT_NOTIFY_TITLE)
            .setContentText(text)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setContentIntent(pending)
            .build()
    }

    companion object {
        private const val logTag = "OutgoingConnection"
        private const val ACTION_START = "com.carriez.flutter_hbb.OUTGOING_START"
        private const val ACTION_STOP = "com.carriez.flutter_hbb.OUTGOING_STOP"
        private const val NOTIFICATION_ID = 2
        private const val CHANNEL_ID = "rustdesk_outgoing"
        private val lock = Any()
        private var requested = 0

        fun start(context: Context): Boolean {
            val launch = synchronized(lock) {
                requested += 1
                requested == 1
            }
            if (!launch) return true
            val intent = Intent(context, OutgoingConnectionService::class.java).setAction(ACTION_START)
            return try {
                ContextCompat.startForegroundService(context, intent)
                true
            } catch (error: Exception) {
                synchronized(lock) { requested = (requested - 1).coerceAtLeast(0) }
                Log.e(logTag, "Failed to start outgoing connection service", error)
                false
            }
        }

        fun stop(context: Context): Boolean {
            val launch = synchronized(lock) {
                if (requested == 0) false
                else {
                    requested -= 1
                    requested == 0
                }
            }
            if (!launch) return true
            val intent = Intent(context, OutgoingConnectionService::class.java).setAction(ACTION_STOP)
            return try {
                context.startService(intent)
                true
            } catch (error: Exception) {
                synchronized(lock) { requested += 1 }
                Log.w(logTag, "Failed to stop outgoing connection service", error)
                false
            }
        }
    }
}
