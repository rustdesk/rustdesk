package com.carriez.flutter_hbb

import android.app.Activity
import android.app.PictureInPictureParams
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import android.util.Rational
import androidx.annotation.RequiresApi

/**
 * Picture-in-picture window for an outgoing remote-desktop session.
 *
 * [FloatingWindowService] is a different window: a small icon that keeps the
 * capture service alive while this device is being controlled.
 */
internal class RemotePictureInPicture(
    private val activity: Activity,
) {
    var eligible: Boolean = false
        private set

    fun supported(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return false
        }
        return activity.packageManager.hasSystemFeature(
            PackageManager.FEATURE_PICTURE_IN_PICTURE
        )
    }

    fun setEligible(eligible: Boolean) {
        this.eligible = eligible
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            publishParams()
        }
    }

    /** Button press. Works on Android 8+ whether or not auto-enter is enabled. */
    fun enterFromUser(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return false
        }
        return enter()
    }

    /**
     * Home / recents on Android 8–11. Android 12+ enters from
     * [PictureInPictureParams.Builder.setAutoEnterEnabled] instead; calling
     * both races the transition.
     */
    fun onUserLeaveHint() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O ||
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
        ) {
            return
        }
        enter()
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun enter(): Boolean {
        if (!supported() || !eligible || activity.isInPictureInPictureMode) {
            return false
        }
        return try {
            activity.enterPictureInPictureMode(buildParams())
        } catch (e: IllegalStateException) {
            Log.w(LOG_TAG, "enterPictureInPictureMode failed: ${e.message}")
            false
        }
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun publishParams() {
        if (!supported()) {
            return
        }
        try {
            activity.setPictureInPictureParams(buildParams())
        } catch (e: IllegalStateException) {
            Log.w(LOG_TAG, "setPictureInPictureParams failed: ${e.message}")
        }
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun buildParams(): PictureInPictureParams {
        val builder = PictureInPictureParams.Builder()
            .setAspectRatio(ASPECT_RATIO)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            builder.setAutoEnterEnabled(eligible)
            // Seamless resize only stretches the last frame. The remote image
            // is laid out to the view size, so a pinch must resize the activity.
            builder.setSeamlessResizeEnabled(false)
        }
        return builder.build()
    }

    private companion object {
        private const val LOG_TAG = "remotePip"
        private val ASPECT_RATIO = Rational(16, 9)
    }
}
