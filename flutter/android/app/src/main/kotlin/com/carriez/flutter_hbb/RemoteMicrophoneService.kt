package com.carriez.flutter_hbb

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.*

/** Owns capture independently of the activity and desktop playback capture. */
class RemoteMicrophoneService : Service() {
    companion object {
        @Volatile var error = ""
        @Volatile var running = false
        private const val CHANNEL = "remote_microphone"
        private const val NOTIFICATION = 9042
    }
    private var recorder: AudioRecordHandle? = null
    private var wakeLock: PowerManager.WakeLock? = null
    override fun onBind(intent: Intent?) = null
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == "stop") { stopSelf(); return START_NOT_STICKY }
        if (recorder != null) return START_NOT_STICKY
        error = ""
        try {
            check(!MainService.isStart && !MainService.isAudioStart) { "Stop tablet screen sharing first" }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(NotificationChannel(CHANNEL, "Remote microphone", NotificationManager.IMPORTANCE_LOW))
            val stop = PendingIntent.getService(this, NOTIFICATION,
                Intent(this, RemoteMicrophoneService::class.java).setAction("stop"),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            val notification = Notification.Builder(this, CHANNEL)
                .setContentTitle("RustDesk microphone")
                .setContentText("Sending microphone to your remote computer")
                .setSmallIcon(android.R.drawable.ic_btn_speak_now)
                .addAction(Notification.Action.Builder(null, "Stop", stop).build())
                .setOngoing(true).build()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                startForeground(NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
            } else { startForeground(NOTIFICATION, notification) }
            val capture = AudioRecordHandle(this, { false }, { false })
            if (!capture.onVoiceCallStarted(null)) throw IllegalStateException(capture.getVoiceCallStartError())
            recorder = capture
            running = true
            wakeLock = (getSystemService(POWER_SERVICE) as PowerManager)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "RustDesk:remote_microphone").apply { acquire() }
        } catch (failure: Exception) {
            error = failure.message ?: "Microphone unavailable"
            stopSelf()
        }
        return START_NOT_STICKY
    }
    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
        super.onTaskRemoved(rootIntent)
    }
    override fun onDestroy() {
        running = false
        recorder?.destroy()
        recorder = null
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
        super.onDestroy()
    }
}
