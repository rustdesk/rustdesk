package com.carriez.flutter_hbb

import ffi.FFI

import android.Manifest
import android.content.Context
import android.media.*
import android.content.pm.PackageManager
import android.media.projection.MediaProjection
import androidx.annotation.RequiresApi
import androidx.core.app.ActivityCompat
import android.os.Build
import android.util.Log
import kotlin.concurrent.thread

const val AUDIO_ENCODING = AudioFormat.ENCODING_PCM_FLOAT //  ENCODING_OPUS need API 30
const val AUDIO_SAMPLE_RATE = 48000
const val AUDIO_CHANNEL_MASK = AudioFormat.CHANNEL_IN_STEREO

class AudioRecordHandle(private var context: Context, private var isVideoStart: ()->Boolean, private var isAudioStart: ()->Boolean) {
    companion object {
        private const val LOG_TAG = "LOG_AUDIO_RECORD_HANDLE"
        private const val NO_ACTIVE_PUBLISHERS = 0
        private var activeAudioFramePublishers = NO_ACTIVE_PUBLISHERS

        @Synchronized
        private fun acquireAudioFramePublisher() {
            if (activeAudioFramePublishers == NO_ACTIVE_PUBLISHERS) {
                FFI.setFrameRawEnable("audio", true)
            }
            activeAudioFramePublishers++
        }

        @Synchronized
        private fun releaseAudioFramePublisher() {
            if (activeAudioFramePublishers == NO_ACTIVE_PUBLISHERS) {
                Log.e(LOG_TAG, "No active audio frame publisher to release")
                return
            }
            activeAudioFramePublishers--
            if (activeAudioFramePublishers == NO_ACTIVE_PUBLISHERS) {
                FFI.setFrameRawEnable("audio", false)
            }
        }
    }

    private val logTag = LOG_TAG

    private var audioRecorder: AudioRecord? = null
    private var audioReader: AudioReader? = null
    private var minBufferSize = 0
    private var audioRecordStat = false
    private var audioThread: Thread? = null

    @RequiresApi(Build.VERSION_CODES.M)
    fun createAudioRecorder(inVoiceCall: Boolean, mediaProjection: MediaProjection?): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return false
        }
        if (ActivityCompat.checkSelfPermission(
            context,
            Manifest.permission.RECORD_AUDIO
        ) != PackageManager.PERMISSION_GRANTED
        ) {
            Log.d(logTag, "createAudioRecorder failed, no RECORD_AUDIO permission")
            return false
        }

        var builder = AudioRecord.Builder()
        .setAudioFormat(
            AudioFormat.Builder()
                .setEncoding(AUDIO_ENCODING)
                .setSampleRate(AUDIO_SAMPLE_RATE)
                .setChannelMask(AUDIO_CHANNEL_MASK).build()
        );
        if (inVoiceCall) {
            builder.setAudioSource(MediaRecorder.AudioSource.VOICE_COMMUNICATION)
        } else {
            mediaProjection?.let {
                var apcc = AudioPlaybackCaptureConfiguration.Builder(it)
                .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                .addMatchingUsage(AudioAttributes.USAGE_ALARM)
                .addMatchingUsage(AudioAttributes.USAGE_GAME)
                .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN).build();
                builder.setAudioPlaybackCaptureConfig(apcc);
            } ?: let {
                Log.d(logTag, "createAudioRecorder failed, mediaProjection null")
                return false
            }
        }
        val recorder = try {
            builder.build()
        } catch (e: Exception) {
            Log.e(logTag, "createAudioRecorder failed", e)
            return false
        }
        audioRecorder = recorder
        Log.d(logTag, "createAudioRecorder done,minBufferSize:$minBufferSize")
        return true
    }

    @RequiresApi(Build.VERSION_CODES.M)
    private fun checkAudioReader() {
        if (audioReader != null && minBufferSize != 0) {
            return
        }
        // read f32 to byte , length * 4
        val bufferSize = 2 * 4 * AudioRecord.getMinBufferSize(
            AUDIO_SAMPLE_RATE,
            AUDIO_CHANNEL_MASK,
            AUDIO_ENCODING
        )
        if (bufferSize <= 0) {
            Log.d(logTag, "get min buffer size fail!")
            return
        }
        audioReader = AudioReader(bufferSize, 4)
        minBufferSize = bufferSize
        Log.d(logTag, "init audioData len:$minBufferSize")
    }

    private fun releaseRecorder(recorder: AudioRecord) {
        try {
            recorder.release()
        } finally {
            if (audioRecorder === recorder) {
                audioRecorder = null
            }
        }
    }

    private fun captureAudio(reader: AudioReader, recorder: AudioRecord) {
        try {
            while (audioRecordStat) {
                reader.readSync(recorder)?.let {
                    FFI.onAudioFrameUpdate(it)
                }
            }
        } finally {
            minBufferSize = 0
            try {
                releaseRecorder(recorder)
            } finally {
                releaseAudioFramePublisher()
                Log.d(logTag, "Exit audio thread")
            }
        }
    }

    @RequiresApi(Build.VERSION_CODES.M)
    fun startAudioRecorder(): Boolean {
        val recorder = audioRecorder
        if (recorder == null) {
            Log.d(logTag, "startAudioRecorder fail")
            return false
        }
        var audioFramePublisherAcquired = false
        return try {
            checkAudioReader()
            val reader = audioReader
            if (reader == null || minBufferSize == 0) {
                releaseRecorder(recorder)
                Log.d(logTag, "startAudioRecorder fail")
                return false
            }
            recorder.startRecording()
            if (recorder.recordingState != AudioRecord.RECORDSTATE_RECORDING) {
                throw IllegalStateException("AudioRecord failed to enter recording state")
            }
            audioRecordStat = true
            val captureThread = thread(start = false) { captureAudio(reader, recorder) }
            acquireAudioFramePublisher()
            audioFramePublisherAcquired = true
            audioThread = captureThread
            captureThread.start()
            true
        } catch (error: Exception) {
            audioRecordStat = false
            audioThread = null
            Log.e(logTag, "startAudioRecorder fail", error)
            try {
                releaseRecorder(recorder)
            } finally {
                if (audioFramePublisherAcquired) {
                    releaseAudioFramePublisher()
                }
            }
            false
        }
    }

    fun isVoiceCallActive(): Boolean {
        return audioRecorder?.audioSource == MediaRecorder.AudioSource.VOICE_COMMUNICATION
    }

    fun onVoiceCallStarted(mediaProjection: MediaProjection?): Boolean {
        if (!isSupportVoiceCall()) {
            return false
        }
        // No need to check if video or audio is started here.
        if (!switchToVoiceCall(mediaProjection)) {
            return false
        }
        return true
    }

    fun onVoiceCallClosed(mediaProjection: MediaProjection?): Boolean {
        // Return true if not supported, because is was not started.
        if (!isSupportVoiceCall()) {
            return true
        }
        val switched = !isVideoStart() || switchOutVoiceCall(mediaProjection)
        tryReleaseAudio()
        return switched
    }

    @RequiresApi(Build.VERSION_CODES.M)
    fun switchToVoiceCall(mediaProjection: MediaProjection?): Boolean {
        audioRecorder?.let {
            if (it.getAudioSource() == MediaRecorder.AudioSource.VOICE_COMMUNICATION) {
                return true
            }
        }
        audioRecordStat = false
        audioThread?.join()
        audioThread = null

        if (!createAudioRecorder(true, mediaProjection)) {
            Log.e(logTag, "createAudioRecorder fail")
            return false
        }
        return startAudioRecorder()
    }

    @RequiresApi(Build.VERSION_CODES.M)
    fun switchOutVoiceCall(mediaProjection: MediaProjection?): Boolean {
        audioRecorder?.let {
            if (it.getAudioSource() != MediaRecorder.AudioSource.VOICE_COMMUNICATION) {
                return true
            }
        }
        audioRecordStat = false
        audioThread?.join()

        if (!createAudioRecorder(false, mediaProjection)) {
            Log.e(logTag, "createAudioRecorder fail")
            return false
        }
        return startAudioRecorder()
    }

    fun tryReleaseAudio() {
        if (isAudioStart() || isVideoStart()) {
            return
        }
        audioRecordStat = false
        audioThread?.join()
        audioThread = null
    }

    fun destroy() {
        Log.d(logTag, "destroy audio record handle")

        audioRecordStat = false
        audioThread?.join()
    }
}
