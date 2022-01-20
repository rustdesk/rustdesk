package com.carriez.flutter_hbb

import android.annotation.SuppressLint
import android.app.*
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager.VIRTUAL_DISPLAY_FLAG_PUBLIC
import android.media.*
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.IBinder
import android.util.Log
import android.view.Surface
import android.widget.Toast
import androidx.annotation.RequiresApi
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationCompat.PRIORITY_MIN
import java.nio.ByteBuffer
import java.util.concurrent.Executors

const val EXTRA_MP_DATA = "mp_intent"
const val START_SERVICE = "start_service"
const val STOP_SERVICE = "stop_service"
const val MIME_TYPE = MediaFormat.MIMETYPE_VIDEO_VP9

// 获取手机尺寸 建立连接时发送尺寸和基础信息
const val FIXED_WIDTH = 500 // 编码器有上限
const val FIXED_HEIGHT = 1000
const val M_KEY_BIT_RATE = 1024_000
const val M_KEY_FRAME_RATE = 30

class MainService : Service() {

    fun rustGetRaw():ByteArray{
        return rawByteArray!!
    }

    external fun init(ctx:Context)

    init {
        System.loadLibrary("rustdesk")
    }

    private val logTag = "LOG_SERVICE"
    private var mMediaProjection: MediaProjection? = null
    private var surface: Surface? = null
    private val singleThread = Executors.newSingleThreadExecutor()
    private var mEncoder: MediaCodec? = null
    private var rawByteArray :ByteArray? = null

    override fun onBind(intent: Intent): IBinder? {
        return null
    }

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d("whichService", "this service:${Thread.currentThread()}")
        init(this) // 注册到rust
        if (intent?.action == START_SERVICE) {
            Log.d(logTag, "service starting:${startId}:${Thread.currentThread()}")
            createNotification()
            val mMediaProjectionManager =
                getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            mMediaProjection = intent.getParcelableExtra<Intent>(EXTRA_MP_DATA)?.let {
                mMediaProjectionManager.getMediaProjection(Activity.RESULT_OK, it)
            }
            Log.d(logTag, "获取mMediaProjection成功$mMediaProjection")
            if (testSupport()) {
                startRecorder()
            } else {
                Toast.makeText(this, "此设备不支持:$MIME_TYPE", Toast.LENGTH_SHORT).show()
                stopSelf(startId)
            }
        } else if (intent?.action == STOP_SERVICE) {
            mEncoder?.let {
                try {
                    Log.d(logTag, "正在释放encoder")
                    it.signalEndOfInputStream()
                    it.stop()
                    it.release()
                } catch (e: Exception) {
                    null
                }
            }
            stopSelf()
        }
        return super.onStartCommand(intent, flags, startId)
    }

    lateinit var mImageReader:ImageReader // * 注意 这里要成为成员变量，防止被回收 https://www.cnblogs.com/yongdaimi/p/11004560.html
    @SuppressLint("WrongConstant")
    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun startRecorder() {
        Log.d(logTag, "startRecorder")
        mMediaProjection?.let { mp ->
            // 使用原始数据
            mImageReader =
                ImageReader.newInstance(FIXED_WIDTH, FIXED_HEIGHT, PixelFormat.RGBA_8888, 2) // 至少是2
            mImageReader.setOnImageAvailableListener({ imageReader: ImageReader ->
                Log.d(logTag, "on image")
                    try {
                        imageReader.acquireLatestImage().use { image ->
                            if (image == null) return@setOnImageAvailableListener
                            val planes = image.planes
                            val buffer = planes[0].buffer
                            buffer.rewind()
                            // 这里注意 处理不当会引发OOM
                            if (rawByteArray == null){
                                rawByteArray = ByteArray(buffer.capacity())
                                buffer.get(rawByteArray!!)
                            }else{
                                buffer.get(rawByteArray!!)
                            }
                        }
                    } catch (ignored: java.lang.Exception) {
                    }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    imageReader.discardFreeBuffers()
                }
            }, null)
            mp.createVirtualDisplay(
                "rustdesk test",
                FIXED_WIDTH, FIXED_HEIGHT, 200, VIRTUAL_DISPLAY_FLAG_PUBLIC,
                mImageReader.surface, null, null
            )


            // 使用内置编码器
//            createMediaCodec()
//            mEncoder?.let {
//                surface = it.createInputSurface()
//                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
//                    surface!!.setFrameRate(1F, FRAME_RATE_COMPATIBILITY_DEFAULT)
//                }
//                it.setCallback(cb)
//                it.start()
//                mp.createVirtualDisplay(
//                    "rustdesk test",
//                    FIXED_WIDTH, FIXED_HEIGHT, 200, VIRTUAL_DISPLAY_FLAG_PUBLIC,
//                    surface, null, null
//                )
//            }
        } ?: let {
            Log.d(logTag, "startRecorder fail,mMediaProjection is null")
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
                singleThread.execute {
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

    external fun sendRaw(buf: ByteBuffer)
    external fun sendVp9(data: ByteArray)

    @RequiresApi(Build.VERSION_CODES.LOLLIPOP)
    private fun testSupport(): Boolean {
        val res = MediaCodecList(MediaCodecList.ALL_CODECS)
            .findEncoderForFormat(
                MediaFormat.createVideoFormat(
                    MediaFormat.MIMETYPE_VIDEO_VP9,
                    FIXED_WIDTH,
                    FIXED_HEIGHT
                )
            )
        return res?.let {
            true
        } ?: let {
            false
        }
    }

    private fun createMediaCodec() {
        Log.d(logTag, "MediaFormat.MIMETYPE_VIDEO_VP9 :$MIME_TYPE")
        mEncoder = MediaCodec.createEncoderByType(MIME_TYPE)
        val mFormat = MediaFormat.createVideoFormat(MIME_TYPE, FIXED_WIDTH, FIXED_HEIGHT)
        mFormat.setInteger(MediaFormat.KEY_BIT_RATE, M_KEY_BIT_RATE)
        mFormat.setInteger(MediaFormat.KEY_FRAME_RATE, M_KEY_FRAME_RATE) // codec的帧率设置无效
        mFormat.setInteger(
            MediaFormat.KEY_COLOR_FORMAT,
            MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible
        )
        mFormat.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 5)
        try {
            mEncoder!!.configure(mFormat, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        } catch (e: Exception) {
            Log.e(logTag, "mEncoder.configure fail!")
        }
    }

    private fun createNotification() {
        val channelId =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                createNotificationChannel("my_service", "My Background Service")
            } else {
                ""
            }
        val notification: Notification = NotificationCompat.Builder(this, channelId)
            .setOngoing(true)
            .setContentTitle("Hello")
            .setPriority(PRIORITY_MIN)
            .setContentText("TEST TEST")
            .build()
        startForeground(11, notification)
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun createNotificationChannel(channelId: String, channelName: String): String {
        val chan = NotificationChannel(
            channelId,
            channelName, NotificationManager.IMPORTANCE_NONE
        )
        chan.lightColor = Color.BLUE
        chan.lockscreenVisibility = Notification.VISIBILITY_PRIVATE
        val service = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        service.createNotificationChannel(chan)
        return channelId
    }

    override fun onDestroy() {
        Log.d(logTag, "service stop:${Thread.currentThread()}")
        Toast.makeText(this, "service done", Toast.LENGTH_SHORT).show()
    }
}