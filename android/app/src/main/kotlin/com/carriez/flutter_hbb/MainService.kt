/**
 * Capture screen,get video and audio,send to rust.
 * Handle notification
 */
package com.carriez.flutter_hbb

import android.Manifest
import android.annotation.SuppressLint
import android.app.*
import android.app.PendingIntent.FLAG_IMMUTABLE
import android.app.PendingIntent.FLAG_UPDATE_CURRENT
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager.VIRTUAL_DISPLAY_FLAG_PUBLIC
import android.hardware.display.VirtualDisplay
import android.media.*
import android.media.AudioRecord.READ_BLOCKING
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.*
import android.util.Log
import android.view.Surface
import android.view.Surface.FRAME_RATE_COMPATIBILITY_DEFAULT
import androidx.annotation.RequiresApi
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.Executors
import kotlin.concurrent.thread
import androidx.media.app.NotificationCompat.MediaStyle

const val EXTRA_MP_DATA = "mp_intent"
const val INIT_SERVICE = "init_service"
const val ACTION_LOGIN_REQ_NOTIFY = "ACTION_LOGIN_REQ_NOTIFY"
const val EXTRA_LOGIN_REQ_NOTIFY = "EXTRA_LOGIN_REQ_NOTIFY"

const val DEFAULT_NOTIFY_TITLE = "RustDesk"
const val DEFAULT_NOTIFY_TEXT = "Service is listening"
const val NOTIFY_ID = 11

const val NOTIFY_TYPE_START_CAPTURE = "NOTIFY_TYPE_START_CAPTURE"

const val MIME_TYPE = MediaFormat.MIMETYPE_VIDEO_VP9

// video const
const val MAX_SCREEN_SIZE = 1200 // 内置编码器有上限 且实际使用中不需要过高的分辨率

const val VIDEO_KEY_BIT_RATE = 1024_000
const val VIDEO_KEY_FRAME_RATE = 30

// audio const
const val AUDIO_ENCODING = AudioFormat.ENCODING_PCM_FLOAT //  ENCODING_OPUS need API 30
const val AUDIO_SAMPLE_RATE = 48000
const val AUDIO_CHANNEL_MASK = AudioFormat.CHANNEL_IN_STEREO

class MainService : Service() {

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

    fun rustSetByName(name: String, arg1: String, arg2: String) {
        when (name) {
            "try_start_without_auth" -> {
                // TODO notify
                loginRequestActionNotification("test","name","id")
            }
            "start_capture" -> {
                Log.d(logTag, "from rust:start_capture")
                if (isStart) {
                    Log.d(logTag, "正在录制")
                    return
                }
                startCapture()
                // TODO notify
            }
            "stop_capture" -> {
                Log.d(logTag, "from rust:stop_capture")
                stopCapture()
            }
            else -> {}
        }
    }

    // jvm call rust
    private external fun init(ctx: Context)
    private external fun startServer()
    // private external fun sendVp9(data: ByteArray)

    private val logTag = "LOG_SERVICE"
    private val useVP9 = false
    private val binder = LocalBinder()
    private var _isReady = false // 是否获取了录屏权限
    private var _isStart = false // 是否正在进行录制
    val isReady: Boolean
        get() = _isReady
    val isStart: Boolean
        get() = _isStart

    // video 注意 这里imageReader要成为成员变量，防止被回收 https://www.cnblogs.com/yongdaimi/p/11004560.html
    private var mediaProjection: MediaProjection? = null
    private var surface: Surface? = null
    private val sendVP9Thread = Executors.newSingleThreadExecutor()
    private var videoEncoder: MediaCodec? = null
    private var videoData: ByteArray? = null
    private var imageReader: ImageReader? = null
    private val videoZeroData = ByteArray(32)
    private var virtualDisplay: VirtualDisplay? = null

    // audio
    private var audioRecorder: AudioRecord? = null
    private var audioData: FloatArray? = null
    private var minBufferSize = 0
    private var isNewData = false
    private val audioZeroData: FloatArray = FloatArray(32)  // 必须是32位 如果只有8位进行ffi传输时会出错
    private var audioRecordStat = false

    // notification
    private lateinit var notificationManager: NotificationManager
    private lateinit var notificationChannel: String
    private lateinit var notificationBuilder: NotificationCompat.Builder

    override fun onCreate() {
        super.onCreate()
        initNotification()
        startServer()
    }

    override fun onBind(intent: Intent): IBinder {
        Log.d(logTag, "service onBind")
        return binder
    }

    inner class LocalBinder : Binder() {
        init {
            Log.d(logTag, "LocalBinder init")
        }

        fun getService(): MainService = this@MainService
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d("whichService", "this service:${Thread.currentThread()}")
        // 只有init的时候通过onStartCommand 且开启前台服务
        if (intent?.action == INIT_SERVICE) {
            Log.d(logTag, "service starting:${startId}:${Thread.currentThread()}")
            createForegroundNotification()
            val mMediaProjectionManager =
                getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            intent.getParcelableExtra<Intent>(EXTRA_MP_DATA)?.let {
                mediaProjection =
                    mMediaProjectionManager.getMediaProjection(Activity.RESULT_OK, it)
                Log.d(logTag, "获取mMediaProjection成功$mediaProjection")
                checkMediaPermission()
                surface = createSurface()
                init(this)
                _isReady = true
            } ?: let {
                Log.d(logTag, "获取mMediaProjection失败！")
            }
//        } else if (intent?.action == ACTION_LOGIN_REQ_NOTIFY) {
            // 暂时不开启通知从通知栏确认登录
//            val notifyLoginRes = intent.getBooleanExtra(EXTRA_LOGIN_REQ_NOTIFY, false)
//            Log.d(logTag, "从通知栏点击了:$notifyLoginRes")
        }
        return super.onStartCommand(intent, flags, startId)
    }

    @SuppressLint("WrongConstant")
    private fun createSurface(): Surface? {
        // 暂时只使用原始数据的情况
        return if (useVP9) {
            // TODO
            null
        } else {
            Log.d(logTag,"ImageReader.newInstance:INFO:$INFO")
            imageReader =
                ImageReader.newInstance(
                    INFO.screenWidth,
                    INFO.screenHeight,
                    PixelFormat.RGBA_8888,
                    2 // maxImages 至少是2
                ).apply {
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
            imageReader?.surface
        }
    }

    fun startCapture(): Boolean {
        if (isStart){
            return true
        }
        if (mediaProjection == null) {
            Log.w(logTag, "startCapture fail,mediaProjection is null")
            return false
        }
        Log.d(logTag, "Start Capture")

        if (useVP9) {
            startVP9VideoRecorder(mediaProjection!!)
        } else {
            startRawVideoRecorder(mediaProjection!!)
        }

        // 音频只支持安卓10以及以上
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            startAudioRecorder()
        }
        checkMediaPermission()
        _isStart = true
        return true
    }

    fun stopCapture() {
        Log.d(logTag, "Stop Capture")
        _isStart = false
        // release video
        virtualDisplay?.release()
        videoEncoder?.let {
            it.signalEndOfInputStream()
            it.stop()
            it.release()
        }
        virtualDisplay = null
        videoEncoder = null
        videoData = null

        // release audio
        audioRecordStat = false
        audioRecorder?.release()
        audioRecorder = null
        minBufferSize = 0
    }

    fun destroy() {
        Log.d(logTag, "destroy service")
        _isReady = false

        stopCapture()
        imageReader?.close()
        imageReader = null

        mediaProjection = null
        checkMediaPermission()
        stopService(Intent(this,InputService::class.java)) // close input service maybe not work
        stopForeground(true)
        stopSelf()
    }

    fun checkMediaPermission(): Boolean {
        Handler(Looper.getMainLooper()).post {
            MainActivity.flutterMethodChannel.invokeMethod(
                "on_permission_changed",
                mapOf("name" to "media", "value" to isReady.toString())
            )
        }
        return isReady
    }

    @SuppressLint("WrongConstant")
    private fun startRawVideoRecorder(mp: MediaProjection) {
        Log.d(logTag, "startRawVideoRecorder,screen info:$INFO")
        if(surface==null){
            Log.d(logTag, "startRawVideoRecorder failed,surface is null")
            return
        }
        virtualDisplay = mp.createVirtualDisplay(
            "RustDesk",
            INFO.screenWidth, INFO.screenHeight, 200, VIRTUAL_DISPLAY_FLAG_PUBLIC,
            surface, null, null
        )
    }

    @SuppressLint("WrongConstant")
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

    private val cb: MediaCodec.Callback = object : MediaCodec.Callback() {
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
                    // sendVp9(byteArray)
                    codec.releaseOutputBuffer(index, false)
                }
            }
        }

        override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) {
            Log.e(logTag, "MediaCodec.Callback error:$e")
        }
    }


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
                if (ActivityCompat.checkSelfPermission(
                        this,
                        Manifest.permission.RECORD_AUDIO
                    ) != PackageManager.PERMISSION_GRANTED
                ) {
                    return
                }
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

    private fun initNotification() {
        notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        // 设置通知渠道 android8开始引入 老版本会被忽略 这个东西的作用相当于为通知分类 给用户选择通知消息的种类
        notificationChannel = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channelId = "RustDesk"
            val channelName = "RustDesk Service"
            val channel = NotificationChannel(
                channelId,
                channelName, NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "RustDesk Service Channel"
            }
            channel.lightColor = Color.BLUE
            channel.lockscreenVisibility = Notification.VISIBILITY_PRIVATE
            notificationManager.createNotificationChannel(channel)
            channelId
        } else {
            ""
        }
        notificationBuilder = NotificationCompat.Builder(this, notificationChannel)
    }

    private fun createForegroundNotification() {
        val intent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED
            action = Intent.ACTION_MAIN // 不设置会造成每次都重新启动一个新的Activity
            addCategory(Intent.CATEGORY_LAUNCHER)
            putExtra("type", type)
        }
        val pendingIntent = PendingIntent.getActivity(
            this, 0, intent,
            FLAG_UPDATE_CURRENT
        )
        val notification = notificationBuilder
            .setOngoing(true)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setDefaults(Notification.DEFAULT_ALL)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setContentTitle(DEFAULT_NOTIFY_TITLE)
            .setContentText(DEFAULT_NOTIFY_TEXT)
            .setOnlyAlertOnce(true)
            .setContentIntent(pendingIntent)
            .setColor(ContextCompat.getColor(this, R.color.primary))
            .setWhen(System.currentTimeMillis())
            .build()
        // 这里满足前台服务首次启动时5s内设定好通知内容，这里使用startForeground，后续普通调用使用notificationManager即可
        startForeground(NOTIFY_ID, notification)
    }

    private fun loginRequestActionNotification(type: String, name: String, id: String) {
        // notificationBuilder 第一次使用时状态已保存，再次生成时只需要调整需要修改的部分
        val notification = notificationBuilder
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentTitle("收到${type}连接请求")
            .setContentText("来自:$name-$id")

            // 暂时不开启通知栏接受请求，防止用户误操作
//            .setStyle(MediaStyle().setShowActionsInCompactView(0, 1))
//            .addAction(R.drawable.check_blue, "check", genLoginRequestPendingIntent(true))
//            .addAction(R.drawable.close_red, "close", genLoginRequestPendingIntent(false))
            .build()
        // TODO 为每个login req定义id ，notify id 不能是0 可以定义为client id + 100,如101,102,103
        // 登录成功 取消notify时可以直接使用
        notificationManager.notify(NOTIFY_ID + 1, notification)
    }

    private fun genLoginRequestPendingIntent(res: Boolean): PendingIntent {
        val intent = Intent(this, MainService::class.java).apply {
            action = ACTION_LOGIN_REQ_NOTIFY
            putExtra(EXTRA_LOGIN_REQ_NOTIFY, res)
        }
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            PendingIntent.getService(this, 111, intent, FLAG_IMMUTABLE)
        } else {
            PendingIntent.getService(this, 111, intent, 0)
        }
    }

    private fun setTextNotification(_title: String?, _text: String?) {
        val title = _title ?: DEFAULT_NOTIFY_TITLE
        val text = _text ?: DEFAULT_NOTIFY_TEXT
        val notification = notificationBuilder
            .clearActions()
            .setStyle(null)
            .setContentTitle(title)
            .setContentText(text)
            .build()
        notificationManager.notify(NOTIFY_ID, notification)
    }
}
