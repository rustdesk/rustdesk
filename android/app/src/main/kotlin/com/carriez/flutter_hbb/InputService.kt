package com.carriez.flutter_hbb

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.Context
import android.graphics.Path
import android.os.Build
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import androidx.annotation.RequiresApi
import kotlin.concurrent.thread

class InputService : AccessibilityService() {
//    companion object {
//        var inputService:InputService? = null
//    }
    private val logTag = "input service"
    private var leftIsDown = false
    private var mPath = Path()
    private var mLastGestureStartTime = 0L
    private var mouseX = 0
    private var mouseY = 0

    @RequiresApi(Build.VERSION_CODES.N)
    fun mouseInput(mask: Int, _x: Int, _y: Int) {
        Log.w(logTag, "got mouse input:x:$_x ,y:$_y ,mask:$mask ")

        // TODO 临时倍数
        // TODO 按键抬起按下时候 x y 都是0
        if ( !(mask == 9 || mask == 10) ) {
            mouseX = _x * 2
            mouseY = _y * 2
        }

        // left button down ,was up
        if (mask == 9){
            leftIsDown = true
            startGesture(mouseX,mouseY)
        }

        // left down ,was down
        if (mask == 9){
            continueGesture(mouseX,mouseY)
        }

        // left up ,was down
        if (mask == 10){
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
        mPath.lineTo(x.toFloat(), y.toFloat())
        val stroke = GestureDescription.StrokeDescription(
            mPath,
            0,
            System.currentTimeMillis() - mLastGestureStartTime
        )
        val builder = GestureDescription.Builder()
        builder.addStroke(stroke)
        Log.d(logTag, "end gesture $x $y")
        dispatchGesture(builder.build(), object : GestureResultCallback() {
            override fun onCompleted(gestureDescription: GestureDescription) {
                super.onCompleted(gestureDescription)
                Log.d(logTag, "滑动成功")
            }

            override fun onCancelled(gestureDescription: GestureDescription) {
                super.onCancelled(gestureDescription)
                Log.d(logTag, "滑动失败 ")
            }
        }, null)
    }

    external fun init(ctx: Context)

    init {
        System.loadLibrary("rustdesk")
    }

    private val LOG_TAG = "INPUT_LOG"
    @RequiresApi(Build.VERSION_CODES.O)
    override fun onServiceConnected() {
        super.onServiceConnected()
        Log.d(LOG_TAG,"onServiceConnected!")
        init(this)
    }
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
//        TODO("Not yet implemented")
    }

    override fun onInterrupt() {
//        TODO("Not yet implemented")
    }
}