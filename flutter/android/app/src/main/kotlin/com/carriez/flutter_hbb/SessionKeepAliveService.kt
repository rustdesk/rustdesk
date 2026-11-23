package com.carriez.flutter_hbb

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import ffi.FFI

/**
 * Runs while a remote-control page is open. Without a foreground service Android kills
 * the backgrounded process for "excessive cpu" at its 5-minute power check, even when
 * the app is exempt from battery optimization. A session left in the background is
 * closed after [BACKGROUND_LIMIT_MS].
 */
class SessionKeepAliveService : Service() {
    companion object {
        private const val TAG = "SessionKeepAlive"
        private const val CHANNEL_ID = "session_keep_alive"
        private const val NOTIFY_ID = 7301
        private const val ACTION_DISCONNECT = "SESSION_KEEP_ALIVE_DISCONNECT"
        private const val BACKGROUND_LIMIT_MS = 60 * 60 * 1000L

        private var running: SessionKeepAliveService? = null
        private var appHidden = false

        // Not startForegroundService: that crashes the app if the page closes before
        // onCreate runs, and a plain start is allowed because the page opens on screen.
        fun start(context: Context): Boolean = try {
            context.startService(Intent(context, SessionKeepAliveService::class.java))
            true
        } catch (e: IllegalStateException) {
            Log.e(TAG, "cannot start", e)
            false
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, SessionKeepAliveService::class.java))
        }

        fun setAppHidden(hidden: Boolean) {
            appHidden = hidden
            running?.applyVisibility()
        }
    }

    private val handler = Handler(Looper.getMainLooper())
    private val onLimit = Runnable { endSessions("background limit") }
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    translate("Remote session open"),
                    NotificationManager.IMPORTANCE_LOW
                )
            )
        }
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startForeground(
                    NOTIFY_ID,
                    buildNotification(null),
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
                )
            } else {
                startForeground(NOTIFY_ID, buildNotification(null))
            }
        } catch (e: RuntimeException) {
            Log.e(TAG, "cannot enter the foreground", e)
            stopSelf()
            return
        }
        // The CPU must stay up to answer the peer's keep-alives and to count down the limit.
        wakeLock = (getSystemService(Context.POWER_SERVICE) as PowerManager)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "RustDesk:SessionKeepAlive")
            .apply { setReferenceCounted(false) }
        running = this
        if (appHidden) applyVisibility()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_DISCONNECT) endSessions("notification")
        return START_NOT_STICKY
    }

    private fun applyVisibility() {
        handler.removeCallbacks(onLimit)
        var endsAt: Long? = null
        if (appHidden) {
            wakeLock?.acquire(BACKGROUND_LIMIT_MS + 60_000L)
            handler.postDelayed(onLimit, BACKGROUND_LIMIT_MS)
            endsAt = System.currentTimeMillis() + BACKGROUND_LIMIT_MS
        } else {
            releaseWakeLock()
        }
        getSystemService(Context.NOTIFICATION_SERVICE).let {
            (it as NotificationManager).notify(NOTIFY_ID, buildNotification(endsAt))
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let { if (it.isHeld) it.release() }
    }

    private fun endSessions(reason: String) {
        Log.i(TAG, "closing outgoing sessions: $reason")
        // Close in Rust first: a hidden Flutter page isn't disposed until it's shown again.
        FFI.closeAllSessions()
        MainActivity.flutterMethodChannel?.invokeMethod("on_session_keep_alive_ended", null)
        stopSelf()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        endSessions("task removed")
        super.onTaskRemoved(rootIntent)
    }

    override fun onDestroy() {
        handler.removeCallbacks(onLimit)
        releaseWakeLock()
        if (running === this) running = null
        super.onDestroy()
    }

    private fun buildNotification(endsAt: Long?): Notification {
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).apply {
                action = Intent.ACTION_MAIN
                addCategory(Intent.CATEGORY_LAUNCHER)
            },
            flags
        )
        val disconnect = PendingIntent.getService(
            this,
            0,
            Intent(this, SessionKeepAliveService::class.java).setAction(ACTION_DISCONNECT),
            flags
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.mipmap.ic_stat_logo)
            .setContentTitle(translate("Remote session open"))
            .setContentText(
                endsAt?.let {
                    val time = android.text.format.DateFormat.getTimeFormat(this).format(it)
                    translate("session-keep-alive-paused-tip").replace("{}", time)
                }
            )
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .setContentIntent(open)
            .addAction(0, translate("Disconnect"), disconnect)
            .build()
    }
}
