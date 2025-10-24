package com.carriez.flutter_hbb

import android.app.Application
import android.util.Log
import ffi.FFI

class RustDeskApplication : Application() {
    companion object {
        private const val TAG = "RustDeskApplication"
    }

    override fun onCreate() {
        super.onCreate()
        Log.d(TAG, "RustDeskApplication onCreate")
        FFI.onAppStart(applicationContext)
    }
}
