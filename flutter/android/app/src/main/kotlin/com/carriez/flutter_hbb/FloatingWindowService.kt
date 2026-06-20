package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.PixelFormat
import android.graphics.drawable.Drawable
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.view.WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
import android.view.WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
import android.view.WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
import android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
import android.widget.ImageView
import android.widget.PopupMenu
import androidx.core.app.NotificationCompat
import ffi.FFI
import java.nio.ByteBuffer
import kotlin.math.abs
import kotlin.math.min

class FloatingWindowService : Service(), View.OnTouchListener {

    private lateinit var windowManager: WindowManager
    private lateinit var layoutParams: WindowManager.LayoutParams
    private lateinit var floatingView: ImageView
    private lateinit var originalDrawable: Drawable

    private var dragging = false
    private var lastDownX = 0f
    private var lastDownY = 0f
    private var viewCreated = false
    private var keepScreenOn = KeepScreenOn.DURING_CONTROLLED
    private var hasReceivedFrame = false
    private var frameAspectRatio = 1f
    private var lastTapTime = 0L
    private var isMini = false
    private var sizeBeforeMini = 0

    companion object {
        private val logTag = "floatingService"
        private const val NOTIFY_ID_FLOATING = 200
        private const val PREFS_KEY_SIZE = "floating_window_size"
        private const val DEFAULT_SIZE = 180
        private const val MIN_SIZE = 80
        private const val MAX_SIZE = 600
        private val SIZE_PRESETS = intArrayOf(120, 200, 320)
        private var firstCreate = true
        private var viewUntouchable = false
        private var viewTransparency = 0.85f
        private var lastLayoutX = 0
        private var lastLayoutY = 0
        private var lastOrientation = Configuration.ORIENTATION_UNDEFINED

        var instance: FloatingWindowService? = null
            private set

        fun updateFrame(rgbaBytes: ByteArray, width: Int, height: Int) {
            instance?.updateFrameInternal(rgbaBytes, width, height)
        }
    }

    private fun getPrefs() = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)

    private fun loadSize(): Int {
        val stored = getPrefs().getInt(PREFS_KEY_SIZE, DEFAULT_SIZE)
        return stored.coerceIn(MIN_SIZE, MAX_SIZE)
    }

    private fun saveSize(size: Int) {
        getPrefs().edit().putInt(PREFS_KEY_SIZE, size).apply()
    }

    override fun onBind(intent: Intent): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        instance = this
        startForegroundWithNotification()
        windowManager = getSystemService(WINDOW_SERVICE) as WindowManager
        try {
            if (firstCreate) {
                firstCreate = false
                onFirstCreate()
            }
            Log.d(logTag, "onCreate size=${loadSize()} transparency=$viewTransparency pos=($lastLayoutX,$lastLayoutY)")
            createView()
            handler.postDelayed(runnable, 1000)
        } catch (e: Exception) {
            Log.e(logTag, "onCreate failed: $e")
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        instance = null
        if (viewCreated) {
            windowManager.removeView(floatingView)
        }
        handler.removeCallbacks(runnable)
        stopForeground(STOP_FOREGROUND_REMOVE)
    }

    private fun updateFrameInternal(rgbaBytes: ByteArray, width: Int, height: Int) {
        try {
            val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
            bitmap.copyPixelsFromBuffer(ByteBuffer.wrap(rgbaBytes))
            runOnUiThread {
                floatingView.setImageBitmap(bitmap)
                floatingView.alpha = viewTransparency
                if (!hasReceivedFrame) {
                    hasReceivedFrame = true
                    frameAspectRatio = width.toFloat() / height.toFloat()
                    resizeToAspectRatio()
                }
            }
        } catch (e: Exception) {
            Log.e(logTag, "updateFrame failed: $e")
        }
    }

    private fun resizeToAspectRatio() {
        val maxDim = loadSize()
        val displayW: Int
        val displayH: Int
        if (frameAspectRatio >= 1f) {
            displayW = maxDim
            displayH = (maxDim / frameAspectRatio).toInt()
        } else {
            displayH = maxDim
            displayW = (maxDim * frameAspectRatio).toInt()
        }
        layoutParams.width = displayW.coerceAtLeast(MIN_SIZE)
        layoutParams.height = displayH.coerceAtLeast(MIN_SIZE)
        windowManager.updateViewLayout(floatingView, layoutParams)
        Log.d(logTag, "resizeToAspectRatio: aspect=$frameAspectRatio -> ${layoutParams.width}x${layoutParams.height}")
    }

    private fun runOnUiThread(runnable: Runnable) {
        Handler(Looper.getMainLooper()).post(runnable)
    }

    private fun startForegroundWithNotification() {
        val channelId = "rustdesk_floating"
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                channelId,
                translate("RustDesk"),
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = translate("RustDesk")
                lockscreenVisibility = NotificationCompat.VISIBILITY_PUBLIC
            }
            getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }
        val pendingIntent = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )
        val notification = NotificationCompat.Builder(this, channelId)
            .setSmallIcon(R.mipmap.ic_stat_logo)
            .setContentTitle(translate("RustDesk"))
            .setContentText(translate("Service is running"))
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
        startForeground(NOTIFY_ID_FLOATING, notification)
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun createView() {
        floatingView = ImageView(this)
        viewCreated = true
        originalDrawable = resources.getDrawable(R.drawable.floating_window, null)
        floatingView.setImageDrawable(originalDrawable)
        floatingView.setOnTouchListener(this)
        floatingView.alpha = viewTransparency

        var flags = FLAG_LAYOUT_IN_SCREEN or FLAG_NOT_TOUCH_MODAL or FLAG_NOT_FOCUSABLE
        if (viewUntouchable || viewTransparency == 0f) {
            flags = flags or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        }
        val initialSize = loadSize()
        layoutParams = WindowManager.LayoutParams(
            initialSize, initialSize,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O)
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
            else
                WindowManager.LayoutParams.TYPE_PHONE,
            flags,
            PixelFormat.TRANSLUCENT
        )
        layoutParams.gravity = Gravity.TOP or Gravity.START
        layoutParams.x = lastLayoutX
        layoutParams.y = lastLayoutY

        val keepScreenOnOption = FFI.getLocalOption("keep-screen-on").lowercase()
        keepScreenOn = when (keepScreenOnOption) {
            "never" -> KeepScreenOn.NEVER
            "service-on" -> KeepScreenOn.SERVICE_ON
            else -> KeepScreenOn.DURING_CONTROLLED
        }
        updateKeepScreenOnLayoutParams()
        windowManager.addView(floatingView, layoutParams)
    }

    private fun onFirstCreate() {
        val wh = getScreenSize(windowManager)
        viewUntouchable = FFI.getLocalOption("floating-window-untouchable") == "Y"
        FFI.getLocalOption("floating-window-transparency").let {
            if (it.isNotEmpty()) {
                try {
                    val t = it.toInt()
                    if (t in 0..10) viewTransparency = t / 10f
                } catch (_: Exception) {}
            }
        }
        lastLayoutX = 0
        lastLayoutY = (wh.second - loadSize()) / 2
        lastOrientation = resources.configuration.orientation
    }

    override fun onTouch(view: View?, event: MotionEvent?): Boolean {
        when (event?.action) {
            MotionEvent.ACTION_DOWN -> {
                dragging = false
                lastDownX = event.rawX
                lastDownY = event.rawY
            }
            MotionEvent.ACTION_UP -> {
                val clickDragTolerance = 10f
                if (abs(event.rawX - lastDownX) < clickDragTolerance &&
                    abs(event.rawY - lastDownY) < clickDragTolerance
                ) {
                    val now = System.currentTimeMillis()
                    if (now - lastTapTime < 300) {
                        toggleMini()
                        lastTapTime = 0
                    } else {
                        lastTapTime = now
                        // Delay single-tap action in case double-tap follows
                        handler.postDelayed({
                            if (lastTapTime == now.toLong()) {
                                showPopupMenu()
                            }
                        }, 300)
                    }
                }
            }
            MotionEvent.ACTION_MOVE -> {
                val dx = event.rawX - lastDownX
                val dy = event.rawY - lastDownY
                if (!dragging && dx * dx + dy * dy < 25) return false
                dragging = true
                layoutParams.x = (event.rawX - layoutParams.width / 2).toInt()
                layoutParams.y = (event.rawY - layoutParams.height / 2).toInt()
                windowManager.updateViewLayout(view, layoutParams)
                lastLayoutX = layoutParams.x
                lastLayoutY = layoutParams.y
            }
        }
        return false
    }

    private fun toggleMini() {
        if (isMini) {
            isMini = false
            val restore = if (sizeBeforeMini > 0) sizeBeforeMini else loadSize()
            if (hasReceivedFrame && frameAspectRatio > 0f) {
                val maxDim = restore
                if (frameAspectRatio >= 1f) {
                    layoutParams.width = maxDim
                    layoutParams.height = (maxDim / frameAspectRatio).toInt()
                } else {
                    layoutParams.height = maxDim
                    layoutParams.width = (maxDim * frameAspectRatio).toInt()
                }
            } else {
                layoutParams.width = restore
                layoutParams.height = restore
            }
            floatingView.alpha = viewTransparency
            Log.d(logTag, "toggleMini -> restore ${layoutParams.width}x${layoutParams.height}")
        } else {
            isMini = true
            sizeBeforeMini = maxOf(layoutParams.width, layoutParams.height)
            layoutParams.width = 48
            layoutParams.height = 48
            floatingView.alpha = 0.5f
            Log.d(logTag, "toggleMini -> mini 48x48")
        }
        windowManager.updateViewLayout(floatingView, layoutParams)
    }

    private fun showPopupMenu() {
        val popupMenu = PopupMenu(this, floatingView)

        popupMenu.menu.add(0, 0, 0, translate("Show"))

        val sizeSubmenu = popupMenu.menu.addSubMenu(1, 10, 1, "Size")
        val currentSize = loadSize()
        val screenW = getScreenSize(windowManager).first
        val presets = listOf(
            Triple(11, "Small", 120),
            Triple(12, "Medium", 320),
            Triple(13, "Large", screenW / 2),
            Triple(14, "XL", screenW),
        )
        presets.forEach { (id, name, px) ->
            sizeSubmenu.add(1, id, 0, "$name (${px}px)")
            if (currentSize == px) sizeSubmenu.getItem(sizeSubmenu.size() - 1).isChecked = true
        }
        sizeSubmenu.setGroupCheckable(1, true, true)

        val isServiceSyncEnabled = (MainActivity.rdClipboardManager?.isCaptureStarted ?: false)
                && FFI.isServiceClipboardEnabled()
        if (isServiceSyncEnabled) {
            popupMenu.menu.add(0, 1, 2, translate("Update client clipboard"))
        }
        val hideStopService = FFI.getBuildinOption("hide-stop-service") == "Y"
        if (!hideStopService && MainService.isReady) {
            popupMenu.menu.add(0, 2, 3, translate("Stop service"))
        }

        popupMenu.setOnMenuItemClickListener { menuItem ->
            when (menuItem.itemId) {
                0 -> { openMainActivity(); true }
                1 -> { syncClipboard(); true }
                2 -> { stopMainService(); true }
                11 -> { applySize(120); true }
                12 -> { applySize(320); true }
                13 -> { applySize(screenW / 2); true }
                14 -> { applySize(screenW); true }
                else -> false
            }
        }
        popupMenu.show()
    }

    private fun applySize(size: Int) {
        saveSize(size)
        if (hasReceivedFrame && frameAspectRatio > 0f) {
            resizeToAspectRatio()
        } else {
            layoutParams.width = size
            layoutParams.height = size
            windowManager.updateViewLayout(floatingView, layoutParams)
        }
        Log.d(logTag, "applySize -> $size")
    }

    private fun openMainActivity() {
        val intent = Intent(this, MainActivity::class.java)
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val pendingIntent = PendingIntent.getActivity(
            this, 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_ONE_SHOT
        )
        try { pendingIntent.send() } catch (_: PendingIntent.CanceledException) {}
    }

    private fun syncClipboard() {
        MainActivity.rdClipboardManager?.syncClipboard(false)
    }

    private fun stopMainService() {
        MainActivity.flutterMethodChannel?.invokeMethod("stop_service", null)
    }

    enum class KeepScreenOn { NEVER, DURING_CONTROLLED, SERVICE_ON }

    private val handler = Handler(Looper.getMainLooper())
    private val runnable = object : Runnable {
        override fun run() {
            if (updateKeepScreenOnLayoutParams()) {
                windowManager.updateViewLayout(floatingView, layoutParams)
            }
            handler.postDelayed(this, 1000)
        }
    }

    private fun updateKeepScreenOnLayoutParams(): Boolean {
        val oldOn = layoutParams.flags and FLAG_KEEP_SCREEN_ON != 0
        val newOn = keepScreenOn == KeepScreenOn.SERVICE_ON ||
                (keepScreenOn == KeepScreenOn.DURING_CONTROLLED && MainService.isStart)
        if (oldOn != newOn) {
            if (newOn) {
                layoutParams.flags = layoutParams.flags or FLAG_KEEP_SCREEN_ON
            } else {
                layoutParams.flags = layoutParams.flags and FLAG_KEEP_SCREEN_ON.inv()
            }
            return true
        }
        return false
    }
}
