package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.graphics.drawable.BitmapDrawable
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
import com.caverock.androidsvg.SVG
import ffi.FFI
import kotlin.math.abs

class FloatingWindowService : Service(), View.OnTouchListener {

    private lateinit var windowManager: WindowManager
    private lateinit var layoutParams: WindowManager.LayoutParams
    private lateinit var floatingView: ImageView
    private lateinit var originalDrawable: Drawable
    private lateinit var leftHalfDrawable: Drawable
    private lateinit var rightHalfDrawable: Drawable

    private var dragging = false
    private var lastDownX = 0f
    private var lastDownY = 0f
    private var viewCreated = false;
    private var keepScreenOn = KeepScreenOn.DURING_CONTROLLED

    companion object {
        private val logTag = "floatingService"
        private var firstCreate = true
        private var viewWidth = 120
        private var viewHeight = 120
        private const val MIN_VIEW_SIZE = 32 // size 0 does not help prevent the service from being killed
        private const val MAX_VIEW_SIZE = 320
        private var viewUntouchable = false
        private var viewTransparency = 1f // 0 means invisible but can help prevent the service from being killed
        private var customSvg = ""
        private var lastLayoutX = 0
        private var lastLayoutY = 0
        private var lastOrientation = Configuration.ORIENTATION_UNDEFINED
    }

    override fun onBind(intent: Intent): IBinder? {
        return null
    }

    override fun onCreate() {
        super.onCreate()
        windowManager = getSystemService(WINDOW_SERVICE) as WindowManager
        try {
            if (firstCreate) {
                firstCreate = false
                onFirstCreate(windowManager)
            }
            Log.d(logTag, "floating window size: $viewWidth x $viewHeight, transparency: $viewTransparency, lastLayoutX: $lastLayoutX, lastLayoutY: $lastLayoutY, customSvg: $customSvg")
            createView(windowManager)
            handler.postDelayed(runnable, 1000)
            Log.d(logTag, "onCreate success")
        } catch (e: Exception) {
            Log.d(logTag, "onCreate failed: $e")
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        if (viewCreated) {
            windowManager.removeView(floatingView)
        }
        handler.removeCallbacks(runnable)
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun createView(windowManager: WindowManager) {
        floatingView = ImageView(this)
        viewCreated = true
        originalDrawable = resources.getDrawable(R.drawable.floating_window, null)
        if (customSvg.isNotEmpty()) {
            try {
                val svg = SVG.getFromString(customSvg)
                Log.d(logTag, "custom svg info: ${svg.documentWidth} x ${svg.documentHeight}");
                // This make the svg render clear
               svg.documentWidth = viewWidth * 1f
               svg.documentHeight = viewHeight * 1f
                originalDrawable = svg.renderToPicture().let {
                    BitmapDrawable(
                        resources,
                        Bitmap.createBitmap(it.width, it.height, Bitmap.Config.ARGB_8888)
                            .also { bitmap ->
                                it.draw(Canvas(bitmap))
                            })
                }
                floatingView.setLayerType(View.LAYER_TYPE_SOFTWARE, null);
                Log.d(logTag, "custom svg loaded")
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
        val originalBitmap = Bitmap.createBitmap(
            originalDrawable.intrinsicWidth,
            originalDrawable.intrinsicHeight,
            Bitmap.Config.ARGB_8888
        )
        val canvas = Canvas(originalBitmap)
        originalDrawable.setBounds(
            0,
            0,
            originalDrawable.intrinsicWidth,
            originalDrawable.intrinsicHeight
        )
        originalDrawable.draw(canvas)
        val leftHalfBitmap = Bitmap.createBitmap(
            originalBitmap,
            0,
            0,
            originalDrawable.intrinsicWidth / 2,
            originalDrawable.intrinsicHeight
        )
        val rightHalfBitmap = Bitmap.createBitmap(
            originalBitmap,
            originalDrawable.intrinsicWidth / 2,
            0,
            originalDrawable.intrinsicWidth / 2,
            originalDrawable.intrinsicHeight
        )
        leftHalfDrawable = BitmapDrawable(resources, leftHalfBitmap)
        rightHalfDrawable = BitmapDrawable(resources, rightHalfBitmap)

        floatingView.setImageDrawable(rightHalfDrawable)
        floatingView.setOnTouchListener(this)
        floatingView.alpha = viewTransparency * 1f

        var flags = FLAG_LAYOUT_IN_SCREEN or FLAG_NOT_TOUCH_MODAL or FLAG_NOT_FOCUSABLE
        if (viewUntouchable || viewTransparency == 0f) {
            flags = flags or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        }
        layoutParams = WindowManager.LayoutParams(
            viewWidth / 2,
            viewHeight,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY else WindowManager.LayoutParams.TYPE_PHONE,
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
        Log.d(logTag, "keepScreenOn option: $keepScreenOnOption, value: $keepScreenOn")
        updateKeepScreenOnLayoutParams()

        windowManager.addView(floatingView, layoutParams)
        moveToScreenSide()
    }

    
    private fun openMainActivity() {
        val intent = Intent(this, MainActivity::class.java)
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val pendingIntent = PendingIntent.getActivity(
            this, 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_ONE_SHOT
        )
        try {
            pendingIntent.send()
        } catch (e: PendingIntent.CanceledException) {
            e.printStackTrace()
        }
    }

    private fun stopMainService() {
        MainActivity.flutterMethodChannel?.invokeMethod("stop_service", null)
    }

    enum class KeepScreenOn {
        NEVER,
        DURING_CONTROLLED,
        SERVICE_ON,
    }

    private val handler = Handler(Looper.getMainLooper())
    private val runnable = object : Runnable {
        override fun run() {
            if (updateKeepScreenOnLayoutParams()) {
                windowManager.updateViewLayout(floatingView, layoutParams)
            }
            handler.postDelayed(this, 1000) // 1000 milliseconds = 1 second
        }
    }

    private fun updateKeepScreenOnLayoutParams(): Boolean {
        val oldOn = layoutParams.flags and FLAG_KEEP_SCREEN_ON != 0
        val newOn = keepScreenOn == KeepScreenOn.SERVICE_ON ||  (keepScreenOn == KeepScreenOn.DURING_CONTROLLED  &&  MainService.isStart)
        if (oldOn != newOn) {
            Log.d(logTag, "change keep screen on to $newOn")
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

