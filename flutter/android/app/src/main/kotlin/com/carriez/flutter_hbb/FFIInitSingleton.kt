package com.carriez.flutter_hbb

import ffi.FFI

import android.content.Context
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Static singleton class to initialize NDK contexts in FFI for hardware codecs
 */

enum class FFIInitSingleton {
    INSTANCE;

    private val initCalled = AtomicBoolean(false)

    fun init(ctx: Context) {
        if (initCalled.compareAndSet(false, true)) {
            FFI.init(ctx)
        }
    }
}
