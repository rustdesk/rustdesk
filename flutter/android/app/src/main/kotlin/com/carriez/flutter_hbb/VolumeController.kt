package com.carriez.flutter_hbb

// Inspired by https://github.com/yosemiteyss/flutter_volume_controller/blob/main/android/src/main/kotlin/com/yosemiteyss/flutter_volume_controller/VolumeController.kt

import android.media.AudioManager
import android.os.Build
import android.util.Log

class VolumeController(private val audioManager: AudioManager) {
    private val logTag = "volume controller"

    fun getVolume(streamType: Int): Double {
        val current = audioManager.getStreamVolume(streamType)
        val max = audioManager.getStreamMaxVolume(streamType)
        return current.toDouble() / max
    }

    fun setVolume(volume: Double, showSystemUI: Boolean, streamType: Int) {
        val max = audioManager.getStreamMaxVolume(streamType)
        audioManager.setStreamVolume(
            streamType,
            (max * volume).toInt(),
            if (showSystemUI) AudioManager.FLAG_SHOW_UI else 0
        )
    }

    fun raiseVolume(step: Double?, showSystemUI: Boolean, streamType: Int) {
        if (step == null) {
            audioManager.adjustStreamVolume(
                streamType,
                AudioManager.ADJUST_RAISE,
                if (showSystemUI) AudioManager.FLAG_SHOW_UI else 0
            )
        } else {
            val target = getVolume(streamType) + step
            setVolume(target, showSystemUI, streamType)
        }
    }

    fun lowerVolume(step: Double?, showSystemUI: Boolean, streamType: Int) {
        if (step == null) {
            audioManager.adjustStreamVolume(
                streamType,
                AudioManager.ADJUST_LOWER,
                if (showSystemUI) AudioManager.FLAG_SHOW_UI else 0
            )
        } else {
            val target = getVolume(streamType) - step
            setVolume(target, showSystemUI, streamType)
        }
    }

    fun getMute(streamType: Int): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            audioManager.isStreamMute(streamType)
        } else {
            audioManager.getStreamVolume(streamType) == 0
        }
    }

    private fun setMute(isMuted: Boolean, showSystemUI: Boolean, streamType: Int) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            audioManager.adjustStreamVolume(
                streamType,
                if (isMuted) AudioManager.ADJUST_MUTE else AudioManager.ADJUST_UNMUTE,
                if (showSystemUI) AudioManager.FLAG_SHOW_UI else 0
            )
        } else {
            audioManager.setStreamMute(streamType, isMuted)
        }
    }

    fun toggleMute(showSystemUI: Boolean, streamType: Int) {
        val isMuted = getMute(streamType)
        setMute(!isMuted, showSystemUI, streamType)
    }
}

