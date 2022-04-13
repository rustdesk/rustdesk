package com.carriez.flutter_hbb

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.Context
import android.graphics.Path
import android.os.Build
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import androidx.annotation.Keep
import androidx.annotation.RequiresApi

class InputService : AccessibilityService() {

    companion object {
        var ctx: InputService? = null
        val isOpen: Boolean
            get() = ctx != null
    }

    private external fun init(ctx: Context)

    init {
        System.loadLibrary("rustdesk")
    }

    private val logTag = "input service"
    private var leftIsDown = false
    private var mPath = Path()
    private var mLastGestureStartTime = 0L
    private var mouseX = 0
    private var mouseY = 0

    @Keep
    @RequiresApi(Build.VERSION_CODES.N)
    fun rustMouseInput(mask: Int, _x: Int, _y: Int) {
        val x = if (_x < 0) {
            0
        } else {
            _x
        }

        val y = if (_y < 0) {
            0
        } else {
            _y
        }

        if (!(mask == 9 || mask == 10)) {
            mouseX = x * INFO.scale
            mouseY = y * INFO.scale
        }

        // left button down ,was up
        if (mask == 9) {
            leftIsDown = true
            startGesture(mouseX, mouseY)
        }

        // left down ,was down
        if (leftIsDown) {
            continueGesture(mouseX, mouseY)
        }

        // left up ,was down
        if (mask == 10) {
            leftIsDown = false
            endGesture(mouseX, mouseY)
        }
    }

    private fun startGesture(x: Int, y: Int) {
        mPath = Path()
        mPath.moveTo(x.toFloat(), y.toFloat())
        mLastGestureStartTime = System.currentTimeMillis()
    }

    private fun continueGesture(x: Int, y: Int) {
        mPath.lineTo(x.toFloat(), y.toFloat())
    }

    @RequiresApi(Build.VERSION_CODES.N)
    private fun endGesture(x: Int, y: Int) {
        try {
            mPath.lineTo(x.toFloat(), y.toFloat())
            var duration = System.currentTimeMillis() - mLastGestureStartTime
            if (duration <= 0) {
                duration = 1
            }
            val stroke = GestureDescription.StrokeDescription(
                mPath,
                0,
                duration
            )
            val builder = GestureDescription.Builder()
            builder.addStroke(stroke)
            Log.d(logTag, "end gesture x:$x y:$y time:$duration")
            dispatchGesture(builder.build(), null, null)
        } catch (e: Exception) {
            Log.e(logTag, "endGesture error:$e")
        }
    }

    @RequiresApi(Build.VERSION_CODES.O)
    override fun onServiceConnected() {
        super.onServiceConnected()
        ctx = this
        Log.d(logTag, "onServiceConnected!")
        init(this)
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}

    override fun onInterrupt() {}
}
