package com.carriez.flutter_hbb

import android.app.Application
import android.util.Log
import ffi.FFI

class MainApplication : Application() {
    companion object {
        private const val TAG = "MainApplication"
    }

    override fun onCreate() {
        super.onCreate()
        Log.d(TAG, "App start")
        try {
            FFI.onAppStart(applicationContext)
            Log.d(TAG, "FFI.onAppStart succeeded")
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "Failed to load native library", e)
            throw e
        } catch (e: Exception) {
            Log.e(TAG, "FFI.onAppStart failed", e)
            throw e
        }
    }
}
