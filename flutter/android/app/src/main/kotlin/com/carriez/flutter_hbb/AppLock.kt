package com.carriez.flutter_hbb

import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import android.os.SystemClock
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

internal class AppLock(private val activity: Activity) {
    private var pending: MethodChannel.Result? = null
    private var stoppedAt: Long? = null

    fun handle(call: MethodCall, result: MethodChannel.Result): Boolean {
        if (call.method != "app_lock_authenticate") return false
        authenticate(call.argument<String>("title") ?: "", result)
        return true
    }

    // elapsedRealtime keeps counting in deep sleep and ignores changes to the wall clock.
    fun onStop() {
        stoppedAt = SystemClock.elapsedRealtime()
    }

    fun onStart() {
        val since = stoppedAt ?: return
        MainActivity.flutterMethodChannel?.invokeMethod(
            "on_app_lock_idle",
            SystemClock.elapsedRealtime() - since
        )
    }

    fun onResult(requestCode: Int, resultCode: Int): Boolean {
        if (requestCode != REQUEST_APP_LOCK) return false
        finish(if (resultCode == Activity.RESULT_OK) OK else FAILED)
        return true
    }

    @Suppress("DEPRECATION")
    private fun authenticate(title: String, result: MethodChannel.Result) {
        if (pending != null) {
            result.success(FAILED)
            return
        }
        val keyguard = activity.getSystemService(Context.KEYGUARD_SERVICE) as KeyguardManager
        val secure = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            keyguard.isDeviceSecure
        } else {
            keyguard.isKeyguardSecure
        }
        if (!secure) {
            result.success(UNAVAILABLE)
            return
        }
        pending = result
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                BiometricPrompt.Builder(activity)
                    .setTitle(title)
                    .setAllowedAuthenticators(
                        BiometricManager.Authenticators.BIOMETRIC_WEAK or
                            BiometricManager.Authenticators.DEVICE_CREDENTIAL
                    )
                    .build()
                    .authenticate(
                        CancellationSignal(),
                        activity.mainExecutor,
                        object : BiometricPrompt.AuthenticationCallback() {
                            override fun onAuthenticationSucceeded(
                                auth: BiometricPrompt.AuthenticationResult
                            ) = finish(OK)

                            override fun onAuthenticationError(
                                errorCode: Int,
                                errString: CharSequence
                            ) = finish(FAILED)
                        }
                    )
            } else {
                val intent = keyguard.createConfirmDeviceCredentialIntent(title, null)
                if (intent == null) {
                    finish(UNAVAILABLE)
                } else {
                    activity.startActivityForResult(intent, REQUEST_APP_LOCK)
                }
            }
        } catch (error: Exception) {
            finish(FAILED)
        }
    }

    private fun finish(value: String) {
        pending?.success(value)
        pending = null
    }

    private companion object {
        private const val REQUEST_APP_LOCK = 4261
        private const val OK = "ok"
        private const val FAILED = "failed"
        private const val UNAVAILABLE = "unavailable"
    }
}
