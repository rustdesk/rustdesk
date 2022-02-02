/**
 * video_service and audio_service
 */
package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.app.*
import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager.VIRTUAL_DISPLAY_FLAG_PUBLIC
import android.hardware.display.VirtualDisplay
import android.media.*
import android.media.AudioRecord.READ_BLOCKING
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import android.view.Surface
import android.view.Surface.FRAME_RATE_COMPATIBILITY_DEFAULT
import android.widget.Toast
import androidx.annotation.RequiresApi
import java.util.concurrent.Executors
import kotlin.concurrent.thread

const val EXTRA_MP_DATA = "mp_intent"
const val INIT_SERVICE = "init_service"
const val START_CAPTURE = "start_capture"
const val STOP_CAPTURE = "stop_capture"
const val STOP_SERVICE = "stop_service"

const val NOTIFY_TYPE_START_CAPTURE = "NOTIFY_TYPE_START_CAPTURE"

@RequiresApi(Build.VERSION_CODES.LOLLIPOP)
const val MIME_TYPE = MediaFormat.MIMETYPE_VIDEO_VP9

// video const
const val MAX_SCREEN_SIZE = 1200 // 内置编码器有上限 且实际使用中不需要过高的分辨率

const val VIDEO_KEY_BIT_RATE = 1024_000
const val VIDEO_KEY_FRAME_RATE = 30

// audio const
@RequiresApi(Build.VERSION_CODES.LOLLIPOP)
const val AUDIO_ENCODING = AudioFormat.ENCODING_PCM_FLOAT //  ENCODING_OPUS need API 30
const val AUDIO_SAMPLE_RATE = 48000
const val AUDIO_CHANNEL_MASK = AudioFormat.CHANNEL_IN_STEREO

class MainService : Service() {

    companion object {
        private var mediaProjection: MediaProjection? = null
        fun checkMediaPermission(): Boolean {
            val value = mediaProjection != null
            Handler(Looper.getMainLooper()).post {
                MainActivity.flutterMethodChannel.invokeMethod(
                    "on_permission_changed",
                    mapOf("name" to "media", "value" to value.toString())
                )
            }
            return value
        }
    }

    init {
        System.loadLibrary("rustdesk")
    }

    // rust call jvm
    fun rustGetVideoRaw(): ByteArray {
        return if (videoData != null) {
            videoData!!
        } else {
            videoZeroData
        }
    }

    fun rustGetAudioRaw(): FloatArray {
        return if (isNewData && audioData != null) {
            isNewData = false
            audioData!!
        } else {
            audioZeroData
        }
    }

    fun rustGetAudioRawLen(): Int {
        return if (isNewData && audioData != null && audioData!!.isNotEmpty()) {
            audioData!!.size
        } else 0
    }

    fun rustGetByName(name: String): String {
        return when (name) {
            "screen_size" -> "${INFO.screenWidth}:${INFO.screenHeight}"
            else -> ""
        }
    }

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    fun rustSetByName(name: String, arg1: String, arg2: String) {
        when (name) {
            else -> {}
        }
    }

    // jvm call rust
    private external fun init(ctx: Context)
    private external fun sendVp9(data: ByteArray)

    private val logTag = "LOG_SERVICE"
    private val useVP9 = false

    // video
    private var surface: Surface? = null
    private val sendVP9Thread = Executors.newSingleThreadExecutor()
    private var videoEncoder: MediaCodec? = null
    private var videoData: ByteArray? = null
    private var imageReader: ImageReader? =
        null // * 注意 这里要成为成员变量，防止被回收 https://www.cnblogs.com/yongdaimi/p/11004560.html
    private val videoZeroData = ByteArray(32)
    private var virtualDisplay: VirtualDisplay? = null

    // audio
    private var audioRecorder: AudioRecord? = null
    private var audioData: FloatArray? = null
    private var minBufferSize = 0
    private var isNewData = false
    private val audioZeroData: FloatArray = FloatArray(32)  // 必须是32位 如果只有8位进行ffi传输时会出错
    private var audioRecordStat = false

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d("whichService", "this service:${Thread.currentThread()}")
        when (intent?.action) {
            INIT_SERVICE -> {
                Log.d(logTag, "service starting:${startId}:${Thread.currentThread()}")
                createForegroundNotification(this)
                val mMediaProjectionManager =
                    getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
                intent.getParcelableExtra<Intent>(EXTRA_MP_DATA)?.let {
                    mediaProjection =
                        mMediaProjectionManager.getMediaProjection(Activity.RESULT_OK, it)
                    Log.d(logTag, "获取mMediaProjection成功$mediaProjection")
                    checkMediaPermission()
                    init(this)
                } ?: let {
                    Log.d(logTag, "获取mMediaProjection失败！")
                }
            }
            START_CAPTURE -> {
                startCapture()
            }
            STOP_CAPTURE -> {
                stopCapture()
            }
            STOP_SERVICE -> {
                stopCapture()
                mediaProjection = null
                checkMediaPermission()
                stopSelf()
            }
        }
        return super.onStartCommand(intent, flags, startId)
    }

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun startCapture(): Boolean {
        if (testVP9Support()) {  // testVP9Support一直返回true 暂时只使用原始数据
            startVideoRecorder()
        } else {
            Toast.makeText(this, "此设备不支持:$MIME_TYPE", Toast.LENGTH_SHORT).show()
            return false
        }
        // 音频只支持安卓10以及以上
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            startAudioRecorder()
        }
        checkMediaPermission()
        return true
    }

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun stopCapture() {
        virtualDisplay?.release()
        imageReader?.close()
        videoEncoder?.let {
            it.signalEndOfInputStream()
            it.stop()
            it.release()
        }
        audioRecorder?.startRecording()
        audioRecordStat = false

        // audioRecorder 如果无法重新创建 保留服务的情况不要释放
//        audioRecorder?.stop()
//        mediaProjection?.stop()

        virtualDisplay = null
        imageReader = null
        videoEncoder = null
        videoData = null
//        audioRecorder = null
//        audioData = null
    }


    @SuppressLint("WrongConstant")
    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun startVideoRecorder() {
        Log.d(logTag, "startVideoRecorder")
        mediaProjection?.let { mp ->
            if (useVP9) {
                startVP9VideoRecorder(mp)
            } else {
                startRawVideoRecorder(mp)
            }
        } ?: let {
            Log.d(logTag, "startRecorder fail,mMediaProjection is null")
        }
    }

    @SuppressLint("WrongConstant")
    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun startRawVideoRecorder(mp: MediaProjection) {
        Log.d(logTag, "startRawVideoRecorder,screen info:$INFO")
        // 使用原始数据
        imageReader =
            ImageReader.newInstance(
                INFO.screenWidth,
                INFO.screenHeight,
                PixelFormat.RGBA_8888,
                2 // maxImages 至少是2
            ).apply {
                // 奇怪的现象，必须从MainActivity调用 无法从MainService中调用 会阻塞在这个函数
                setOnImageAvailableListener({ imageReader: ImageReader ->
                    try {
                        imageReader.acquireLatestImage().use { image ->
                            if (image == null) return@setOnImageAvailableListener
                            val planes = image.planes
                            val buffer = planes[0].buffer
                            buffer.rewind()
                            // Be careful about OOM!
                            if (videoData == null) {
                                videoData = ByteArray(buffer.capacity())
                                buffer.get(videoData!!)
                                Log.d(logTag, "init video ${videoData!!.size}")
                            } else {
                                buffer.get(videoData!!)
                            }
                        }
                    } catch (ignored: java.lang.Exception) {
                    }
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                        imageReader.discardFreeBuffers()
                    }
                }, null)
            }
        Log.d(logTag, "ImageReader.setOnImageAvailableListener done")
        virtualDisplay = mp.createVirtualDisplay(
            "RustDesk",
            INFO.screenWidth, INFO.screenHeight, 200, VIRTUAL_DISPLAY_FLAG_PUBLIC,
            imageReader?.surface, null, null
        )
        Log.d(logTag, "startRawVideoRecorder done")
    }

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun startVP9VideoRecorder(mp: MediaProjection) {
        //使用内置编码器
        createMediaCodec()
        videoEncoder?.let {
            surface = it.createInputSurface()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                surface!!.setFrameRate(1F, FRAME_RATE_COMPATIBILITY_DEFAULT)
            }
            it.setCallback(cb)
            it.start()
            virtualDisplay = mp.createVirtualDisplay(
                "rustdesk test",
                INFO.screenWidth, INFO.screenHeight, 200, VIRTUAL_DISPLAY_FLAG_PUBLIC,
                surface, null, null
            )
        }
    }

    private val cb: MediaCodec.Callback = @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    object : MediaCodec.Callback() {
        override fun onInputBufferAvailable(codec: MediaCodec, index: Int) {}
        override fun onOutputFormatChanged(codec: MediaCodec, format: MediaFormat) {}

        override fun onOutputBufferAvailable(
            codec: MediaCodec,
            index: Int,
            info: MediaCodec.BufferInfo
        ) {
            codec.getOutputBuffer(index)?.let { buf ->
                sendVP9Thread.execute {
                    // TODO 优化内存使用方式
                    val byteArray = ByteArray(buf.limit())
                    buf.get(byteArray)
                    sendVp9(byteArray)
                    codec.releaseOutputBuffer(index, false)
                }
            }
        }

        override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) {
            Log.e(logTag, "MediaCodec.Callback error:$e")
        }
    }


    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun createMediaCodec() {
        Log.d(logTag, "MediaFormat.MIMETYPE_VIDEO_VP9 :$MIME_TYPE")
        videoEncoder = MediaCodec.createEncoderByType(MIME_TYPE)
        val mFormat = MediaFormat.createVideoFormat(MIME_TYPE, INFO.screenWidth, INFO.screenHeight)
        mFormat.setInteger(MediaFormat.KEY_BIT_RATE, VIDEO_KEY_BIT_RATE)
        mFormat.setInteger(MediaFormat.KEY_FRAME_RATE, VIDEO_KEY_FRAME_RATE) // codec的帧率设置无效
        mFormat.setInteger(
            MediaFormat.KEY_COLOR_FORMAT,
            MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible
        )
        mFormat.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 5)
        try {
            videoEncoder!!.configure(mFormat, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        } catch (e: Exception) {
            Log.e(logTag, "mEncoder.configure fail!")
        }
    }

    @RequiresApi(Build.VERSION_CODES.M)
    private fun startAudioRecorder() {
        checkAudioRecorder()
        if (audioData != null && audioRecorder != null && minBufferSize != 0) {
            audioRecorder!!.startRecording()
            audioRecordStat = true
            thread {
                while (audioRecordStat) {
                    val res = audioRecorder!!.read(audioData!!, 0, minBufferSize, READ_BLOCKING)
                    // 录制float 需要使用对应的read float[] 函数
                    if (res != AudioRecord.ERROR_INVALID_OPERATION) {
                        isNewData = true
                    }
                }
                Log.d(logTag, "Exit audio thread")
            }
        } else {
            Log.d(logTag, "startAudioRecorder fail")
        }
    }

    @RequiresApi(Build.VERSION_CODES.M)
    private fun checkAudioRecorder() {
        if (audioData != null && audioRecorder != null && minBufferSize != 0) {
            return
        }
        minBufferSize = 2 * AudioRecord.getMinBufferSize(
            AUDIO_SAMPLE_RATE,
            AUDIO_CHANNEL_MASK,
            AUDIO_ENCODING
        )
        if (minBufferSize == 0) {
            Log.d(logTag, "get min buffer size fail!")
            return
        }
        audioData = FloatArray(minBufferSize)
        Log.d(logTag, "init audioData len:${audioData!!.size}")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            mediaProjection?.let {
                val apcc = AudioPlaybackCaptureConfiguration.Builder(it)
                    .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                    .addMatchingUsage(AudioAttributes.USAGE_ALARM)
                    .addMatchingUsage(AudioAttributes.USAGE_GAME)
                    .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN).build()
                audioRecorder = AudioRecord.Builder()
                    .setAudioFormat(
                        AudioFormat.Builder()
                            .setEncoding(AUDIO_ENCODING)
                            .setSampleRate(AUDIO_SAMPLE_RATE)
                            .setChannelMask(AUDIO_CHANNEL_MASK).build()
                    )
                    .setAudioPlaybackCaptureConfig(apcc)
                    .setBufferSizeInBytes(minBufferSize).build()
                Log.d(logTag, "createAudioRecorder done,minBufferSize:$minBufferSize")
                return
            }
        }
        Log.d(logTag, "createAudioRecorder fail")
    }

    override fun onDestroy() {
        Log.d(logTag, "service stop:${Thread.currentThread()}")
        Toast.makeText(this, "service done", Toast.LENGTH_SHORT).show()
    }

    override fun onBind(intent: Intent): IBinder? {
        return null
    }
}