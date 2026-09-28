package com.carriez.flutter_hbb

import android.app.Activity
import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.database.Cursor
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Parcel
import android.os.RemoteException
import android.os.SystemClock
import android.util.Log
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.ViewConfiguration
import hbb.KeyEventConverter
import hbb.MessageOuterClass
import moe.shizuku.api.BinderContainer
import java.util.UUID
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

/**
 * Pointer-input backend for Android 5.1/6.0 (API 22/23 only; the gate is
 * `SDK_INT < 24` and minSdk is 22).
 *
 * InputService injects remote pointer events with
 * AccessibilityService#dispatchGesture, which only exists since API 24, so on
 * legacy devices the regular path cannot work. When Shizuku
 * (https://github.com/RikkaApps/Shizuku) has been started via adb on the
 * device, this backend injects the same events with
 * IInputManager#injectInputEvent as the shell user instead, which the shell
 * uid is privileged to do. Keyboard and text input keep going through
 * InputService, whose accessibility text path works on old APIs.
 *
 * No Shizuku code is vendored; this file implements the small Shizuku v3 wire
 * protocol from scratch. Protocol reference: RikkaApps/Shizuku v3.6.1
 * (server/ShizukuService.java, server/BinderSender.java and the manager's
 * legacy authorization activities):
 *  - the server pushes its service binder to this app with a ContentProvider
 *    call "sendBinder" on the authority "<package>.shizuku"; the binder is
 *    wrapped in a parcelable of the fixed class name
 *    "moe.shizuku.api.BinderContainer" (see moe/shizuku/api/BinderContainer.kt);
 *  - the service binder speaks the "moe.shizuku.server.IShizukuService"
 *    descriptor; the transaction codes below are the explicit AIDL method ids
 *    of the v3 API, opcode 1 is the generic forwarder ("transactRemote":
 *    interface token, target binder, target code, then the raw payload);
 *  - on API < 23 the server additionally requires the client uid to be
 *    unlocked once per server start with setUidToken; on API 23 the runtime
 *    permission moe.shizuku.manager.permission.API_V23 is checked instead.
 */
object ShizukuLegacyInput {
    private const val TAG = "ShizukuLegacyInput"

    // Protocol constants from the Shizuku v3 API (ShizukuApiConstants, last
    // published in-repo at tag v3.5.0-2; unchanged by v3.6.1).
    internal const val MANAGER_PACKAGE = "moe.shizuku.privileged.api"
    internal const val EXTRA_BINDER = "$MANAGER_PACKAGE.intent.extra.BINDER"
    private const val SHIZUKU_DESCRIPTOR = "moe.shizuku.server.IShizukuService"
    private const val SHIZUKU_TRANSACTION_TRANSACT = 1
    // AIDL method ids. NOTE: v3.6.1 shifted every v3.5.0-2 code by +1 because a
    // new unauthenticated method was inserted at code 2: getVersion=3, getUid=4,
    // checkPermission=5, getToken=6, setUidToken=7, newProcess=8,
    // getSELinuxContext=9. Determined empirically against a running v3.6.1
    // server: the v3 API sources were externalized out of the Shizuku repo and
    // the artifact is no longer published anywhere.
    private const val SHIZUKU_TRANSACTION_SET_UID_TOKEN = 7
    private const val ACTION_REQUEST_AUTHORIZATION =
        "$MANAGER_PACKAGE.intent.action.REQUEST_AUTHORIZATION"
    private const val EXTRA_IS_V3 = "$MANAGER_PACKAGE.intent.extra.IS_V3"
    private const val EXTRA_TOKEN_MOST_SIG = "$MANAGER_PACKAGE.intent.extra.TOKEN_MOST_SIG"
    private const val EXTRA_TOKEN_LEAST_SIG = "$MANAGER_PACKAGE.intent.extra.TOKEN_LEAST_SIG"
    private const val PERMISSION_V23 = "moe.shizuku.manager.permission.API_V23"

    // android.hardware.input.IInputManager#injectInputEvent, the 5th method of
    // the AIDL on API 21-25 (frameworks/base/core/java/android/hardware/input/
    // IInputManager.aidl, tags android-5.0.2_r1 .. android-7.1.2_r1:
    // getInputDevice, getInputDeviceIds, hasKeys, tryPointerSpeed,
    // injectInputEvent). Verified empirically against the running server.
    private const val INPUT_DESCRIPTOR = "android.hardware.input.IInputManager"
    private const val INPUT_TRANSACTION_INJECT_INPUT_EVENT = IBinder.FIRST_CALL_TRANSACTION + 4

    // IInputManager#INJECT_INPUT_EVENT_MODE_ASYNC / #INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH
    private const val INJECT_MODE_ASYNC = 0
    private const val INJECT_MODE_WAIT_FOR_FINISH = 2

    private const val PREF_NAME = "shizuku_legacy_input"
    private const val KEY_TOKEN_MOST_SIG = "token_most_sig"
    private const val KEY_TOKEN_LEAST_SIG = "token_least_sig"

    const val REQ_AUTHORIZATION = 34127

    // src/common.rs MOUSE_TYPE_WHEEL: the controller sends a wheel tick as a
    // MouseEvent with these type bits and the scroll delta in x/y (y < 0 =
    // wheel down). InputService ignores this mask, which is why wheel
    // scrolling does not reach Android peers upstream; this backend handles it.
    private const val MOUSE_TYPE_WHEEL = 3

    @Volatile
    private var appContext: Context? = null

    @Volatile
    private var shizukuBinder: IBinder? = null

    @Volatile
    private var inputBinder: IBinder? = null

    @Volatile
    private var managerInstalledCache: Boolean? = null

    @Volatile
    private var managerCheckTime = 0L

    private var notReadyLogged = false

    private var authPrompted = false

    @Volatile
    private var authActivity: Activity? = null

    private var leftIsDown = false

    // Pointer position, shared between the touch and mouse kinds like in InputService.
    private var mouseX = 0
    private var mouseY = 0
    private var mouseDownTime = 0L
    private var touchDownTime = 0L

    private var recentsTask: ScheduledFuture<*>? = null

    private val deathRecipient = IBinder.DeathRecipient {
        Log.w(TAG, "Shizuku server died; waiting for it to push a new binder")
        shizukuBinder = null
        inputBinder = null
    }

    private val executor: ScheduledExecutorService = Executors.newSingleThreadScheduledExecutor()

    // Mirrors InputService: tap timeout + long-press timeout.
    private val longPressDuration =
        ViewConfiguration.getTapTimeout().toLong() + ViewConfiguration.getLongPressTimeout().toLong()

    fun init(context: Context) {
        appContext = context.applicationContext
    }

    // Control keys better handled by InputService: volume adjusts the system
    // stream through VolumeController, and the power key opens the power
    // dialog through an accessibility global action.
    private val inputServiceOnlyControlKeys = setOf(
        MessageOuterClass.ControlKey.VolumeMute,
        MessageOuterClass.ControlKey.VolumeUp,
        MessageOuterClass.ControlKey.VolumeDown,
        MessageOuterClass.ControlKey.Power,
    )

    /**
     * Routes a remote key event on legacy devices. Key-code based events
     * (control keys) are injected through Shizuku — the accessibility key
     * path of InputService cannot dispatch them reliably below API 24 —
     * while text events (seq/chr) stay on InputService, whose ACTION_SET_TEXT
     * path is the only non-root way to commit arbitrary text (CJK has no key
     * codes). Returns true when the event was handled here.
     */
    fun maybeHandleKeyEvent(data: ByteArray): Boolean {
        val binder = shizukuBinder
        if (binder == null || !binder.isBinderAlive) {
            return false
        }
        val keyEvent = try {
            MessageOuterClass.KeyEvent.parseFrom(data)
        } catch (e: Exception) {
            return false
        }
        if (!keyEvent.hasControlKey() || keyEvent.controlKey in inputServiceOnlyControlKeys) {
            return false
        }
        val converted = KeyEventConverter.toAndroidKeyEvent(keyEvent)
        if (converted.keyCode == KeyEvent.KEYCODE_UNKNOWN) {
            return false
        }
        val downTime = SystemClock.uptimeMillis()
        val keyDown = KeyEvent(
            downTime, SystemClock.uptimeMillis(), converted.action,
            converted.keyCode, converted.repeatCount, converted.metaState
        )
        executor.execute {
            try {
                injectKey(keyDown)
                if (keyEvent.press && converted.action == KeyEvent.ACTION_DOWN) {
                    injectKey(
                        KeyEvent(
                            downTime, SystemClock.uptimeMillis(), KeyEvent.ACTION_UP,
                            converted.keyCode, 0, converted.metaState
                        )
                    )
                }
            } catch (e: Throwable) {
                Log.e(TAG, "control key injection failed, controlKey=${keyEvent.controlKey}", e)
            }
        }
        return true
    }

    /** The host activity while it is alive, so authorization can be prompted
     *  when the binder arrives (the server pushes it asynchronously, possibly
     *  after the activity has resumed). */
    fun attachAuthorizationHost(activity: Activity) {
        authActivity = activity
    }

    fun detachAuthorizationHost(activity: Activity) {
        if (authActivity === activity) {
            authActivity = null
        }
    }

    /** True when this legacy backend can be used on the current device. */
    val isLegacyBackendAvailable: Boolean
        get() = Build.VERSION.SDK_INT < Build.VERSION_CODES.N && isManagerInstalled

    private val isManagerInstalled: Boolean
        get() {
            // Cache a positive lookup forever; retry a negative one every few
            // seconds so installing the manager later works without an app restart.
            val cached = managerInstalledCache
            val now = SystemClock.elapsedRealtime()
            if (cached != null && (cached || now - managerCheckTime < 5000)) {
                return cached
            }
            val context = appContext
            if (context == null) {
                return false
            }
            val installed = try {
                context.packageManager.getPackageInfo(MANAGER_PACKAGE, 0)
                true
            } catch (e: Exception) {
                false
            }
            managerCheckTime = now
            managerInstalledCache = installed
            return installed
        }

    /** Entry point from MainService.rustPointerInput on legacy devices. */
    fun onPointerInput(kind: Int, mask: Int, x: Int, y: Int) {
        val binder = shizukuBinder
        if (binder == null || !binder.isBinderAlive) {
            if (!notReadyLogged) {
                notReadyLogged = true
                Log.w(
                    TAG,
                    "Shizuku server not connected; start Shizuku (adb) and reopen RustDesk to authorize"
                )
            }
            return
        }
        executor.execute {
            try {
                dispatchPointer(kind, mask, x, y)
            } catch (e: Throwable) {
                Log.e(TAG, "inject failed, kind=$kind mask=$mask", e)
            }
        }
    }

    /** Called by MainActivity.onCreate on legacy devices. */
    fun maybeRequestAuthorization(activity: Activity) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N || !isManagerInstalled) {
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            // API 23: the server only checks the runtime permission API_V23.
            if (activity.checkSelfPermission(PERMISSION_V23) != PackageManager.PERMISSION_GRANTED) {
                activity.requestPermissions(arrayOf(PERMISSION_V23), REQ_AUTHORIZATION)
            }
            return
        }
        val binder = shizukuBinder
        if (binder == null || !binder.isBinderAlive) {
            // The server pushes its binder when the app process comes up;
            // nothing to authorize until then.
            return
        }
        if (loadToken() != null) {
            sendToken(binder)
            return
        }
        val intent = Intent(ACTION_REQUEST_AUTHORIZATION)
            .putExtra(EXTRA_IS_V3, true)
            .setPackage(MANAGER_PACKAGE)
        if (intent.resolveActivity(activity.packageManager) == null) {
            Log.w(TAG, "Shizuku manager has no pre-23 authorization activity")
            return
        }
        authPrompted = true
        activity.startActivityForResult(intent, REQ_AUTHORIZATION)
    }

    fun onAuthorizationResult(resultCode: Int, data: Intent?) {
        when (resultCode) {
            Activity.RESULT_OK -> {
                val mostSig = data?.getLongExtra(EXTRA_TOKEN_MOST_SIG, 0L) ?: 0L
                val leastSig = data?.getLongExtra(EXTRA_TOKEN_LEAST_SIG, 0L) ?: 0L
                if (mostSig == 0L && leastSig == 0L) {
                    Log.e(TAG, "authorization granted but no token returned")
                    return
                }
                saveToken(mostSig, leastSig)
                shizukuBinder?.let { sendToken(it) }
                Log.i(TAG, "Shizuku authorization granted")
            }
            Activity.RESULT_CANCELED -> Log.w(TAG, "Shizuku authorization denied by user")
            else -> Log.e(TAG, "Shizuku authorization failed, resultCode=$resultCode")
        }
    }

    fun onPermissionResult(grantResults: IntArray) {
        val granted = grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED
        Log.i(TAG, "Shizuku API_V23 permission ${if (granted) "granted" else "denied"}")
    }

    /** Called by ShizukuBinderReceiver when the server pushes its binder. */
    fun onBinderReceived(newBinder: IBinder) {
        notReadyLogged = false
        val old = shizukuBinder
        if (newBinder == old) {
            return
        }
        try {
            old?.unlinkToDeath(deathRecipient, 0)
        } catch (e: NoSuchElementException) {
            // Old binder was already dead.
        }
        try {
            newBinder.linkToDeath(deathRecipient, 0)
        } catch (e: RemoteException) {
            Log.w(TAG, "linkToDeath failed on fresh Shizuku binder", e)
        }
        shizukuBinder = newBinder
        inputBinder = null
        Log.i(TAG, "Shizuku server binder received")
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            if (loadToken() != null) {
                sendToken(newBinder)
            } else {
                promptAuthorizationIfNeeded()
            }
        }
    }

    private fun promptAuthorizationIfNeeded(force: Boolean = false) {
        if (authPrompted && !force) {
            return
        }
        val activity = authActivity ?: return
        authPrompted = true
        Handler(Looper.getMainLooper()).post {
            maybeRequestAuthorization(activity)
        }
    }

    private fun dispatchPointer(kind: Int, mask: Int, _x: Int, _y: Int) {
        when (kind) {
            0 -> dispatchTouch(mask, _x, _y)
            1 -> dispatchMouse(mask, _x, _y)
            else -> {}
        }
    }

    // Touch pan, relative updates; mirrors InputService.onTouchInput.
    private fun dispatchTouch(mask: Int, _x: Int, _y: Int) {
        when (mask) {
            TOUCH_PAN_START -> {
                mouseX = maxOf(0, _x) * SCREEN_INFO.scale
                mouseY = maxOf(0, _y) * SCREEN_INFO.scale
                touchDownTime = SystemClock.uptimeMillis()
                injectMotion(
                    motionEvent(MotionEvent.ACTION_DOWN, mouseX, mouseY, touchDownTime)
                )
            }
            TOUCH_PAN_UPDATE -> {
                mouseX -= _x * SCREEN_INFO.scale
                mouseY -= _y * SCREEN_INFO.scale
                mouseX = maxOf(0, mouseX)
                mouseY = maxOf(0, mouseY)
                injectMotion(
                    motionEvent(MotionEvent.ACTION_MOVE, mouseX, mouseY, touchDownTime),
                    INJECT_MODE_ASYNC
                )
            }
            TOUCH_PAN_END -> {
                injectMotion(
                    motionEvent(MotionEvent.ACTION_UP, mouseX, mouseY, touchDownTime)
                )
                mouseX = maxOf(0, _x) * SCREEN_INFO.scale
                mouseY = maxOf(0, _y) * SCREEN_INFO.scale
            }
            else -> {}
        }
    }

    // Mirrors InputService.onMouseInput; keep the semantics (right click =
    // long-press tap, middle click = home, held middle button = recents, wheel
    // = a fast vertical swipe, the same emulation InputService uses because a
    // plain ACTION_SCROLL is not handled by every surface). Events are injected
    // with touchscreen semantics, like the `input` command, so surfaces react
    // exactly as they do to a real finger.
    private fun dispatchMouse(mask: Int, _x: Int, _y: Int) {
        if (mask == 0 || mask == LEFT_MOVE) {
            mouseX = maxOf(0, _x) * SCREEN_INFO.scale
            mouseY = maxOf(0, _y) * SCREEN_INFO.scale
            if (leftIsDown) {
                injectMotion(
                    motionEvent(MotionEvent.ACTION_MOVE, mouseX, mouseY, mouseDownTime),
                    INJECT_MODE_ASYNC
                )
            }
            return
        }
        when (mask) {
            LEFT_DOWN -> {
                leftIsDown = true
                mouseDownTime = SystemClock.uptimeMillis()
                injectMotion(
                    motionEvent(MotionEvent.ACTION_DOWN, mouseX, mouseY, mouseDownTime)
                )
            }
            LEFT_UP -> {
                if (leftIsDown) {
                    leftIsDown = false
                    injectMotion(
                        motionEvent(MotionEvent.ACTION_UP, mouseX, mouseY, mouseDownTime)
                    )
                }
            }
            RIGHT_UP -> longPressTap()
            BACK_UP -> injectKeyClick(KeyEvent.KEYCODE_BACK)
            // Long middle press -> recents, short middle press -> home.
            WHEEL_BUTTON_DOWN -> {
                recentsTask = executor.schedule({
                    recentsTask = null
                    injectKeyClick(KeyEvent.KEYCODE_APP_SWITCH)
                }, LONG_TAP_DELAY, TimeUnit.MILLISECONDS)
            }
            WHEEL_BUTTON_UP -> {
                val task = recentsTask
                if (task != null) {
                    task.cancel(false)
                    recentsTask = null
                    injectKeyClick(KeyEvent.KEYCODE_HOME)
                }
            }
            WHEEL_DOWN -> wheelSwipe(-WHEEL_STEP)
            WHEEL_UP -> wheelSwipe(WHEEL_STEP)
            // Controller wheel ticks (see MOUSE_TYPE_WHEEL): the delta rides
            // in x/y, so these are not screen coordinates.
            MOUSE_TYPE_WHEEL -> {
                if (_y < 0) {
                    wheelSwipe(-WHEEL_STEP)
                } else if (_y > 0) {
                    wheelSwipe(WHEEL_STEP)
                }
            }
        }
    }

    // Wheel emulation, mirroring InputService's 50ms vertical stroke. The
    // intermediate MOVE events matter: a bare down/up pair lands as a tap on
    // click-annotated views because no motion is ever seen.
    private fun wheelSwipe(deltaY: Int) {
        val downTime = SystemClock.uptimeMillis()
        injectMotion(motionEvent(MotionEvent.ACTION_DOWN, mouseX, mouseY, downTime))
        val steps = 3
        for (i in 1..steps) {
            try {
                Thread.sleep(WHEEL_DURATION / steps)
            } catch (e: InterruptedException) {
                Thread.currentThread().interrupt()
                return
            }
            val action = if (i == steps) MotionEvent.ACTION_UP else MotionEvent.ACTION_MOVE
            injectMotion(motionEvent(action, mouseX, mouseY + deltaY * i / steps, downTime))
        }
    }

    private fun longPressTap() {
        val downTime = SystemClock.uptimeMillis()
        injectMotion(motionEvent(MotionEvent.ACTION_DOWN, mouseX, mouseY, downTime))
        executor.schedule({
            try {
                injectMotion(motionEvent(MotionEvent.ACTION_UP, mouseX, mouseY, downTime))
            } catch (e: Throwable) {
                Log.e(TAG, "long-press tap up failed", e)
            }
        }, longPressDuration, TimeUnit.MILLISECONDS)
    }

    private fun injectKeyClick(keyCode: Int) {
        val now = SystemClock.uptimeMillis()
        injectKey(KeyEvent(now, now, KeyEvent.ACTION_DOWN, keyCode, 0, 0))
        injectKey(KeyEvent(now, now, KeyEvent.ACTION_UP, keyCode, 0, 0))
    }

    private fun motionEvent(action: Int, x: Int, y: Int, downTime: Long): MotionEvent {
        val now = SystemClock.uptimeMillis()
        val props = MotionEvent.PointerProperties().apply {
            id = 0
            toolType = MotionEvent.TOOL_TYPE_FINGER
        }
        val coords = MotionEvent.PointerCoords().apply {
            this.x = x.toFloat()
            this.y = y.toFloat()
            pressure = 1f
            size = 1f
        }
        return MotionEvent.obtain(
            if (downTime > 0L) downTime else now,
            now, action, 1,
            arrayOf(props), arrayOf(coords),
            0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0
        )
    }

    private fun injectMotion(event: MotionEvent, mode: Int = INJECT_MODE_WAIT_FOR_FINISH) {
        val payload = Parcel.obtain()
        val reply = Parcel.obtain()
        try {
            payload.writeInterfaceToken(INPUT_DESCRIPTOR)
            // Generated AIDL stubs precede an `in` parcelable parameter with a
            // non-null flag int; the server-side stub reads it before the event.
            payload.writeInt(1)
            event.writeToParcel(payload, 0)
            payload.writeInt(mode)
            transactRemote(inputServiceBinder(), INPUT_TRANSACTION_INJECT_INPUT_EVENT, payload, reply)
            reply.readException()
        } finally {
            payload.recycle()
            reply.recycle()
            event.recycle()
        }
    }

    private fun injectKey(event: KeyEvent) {
        val payload = Parcel.obtain()
        val reply = Parcel.obtain()
        try {
            payload.writeInterfaceToken(INPUT_DESCRIPTOR)
            // Non-null flag int, see injectMotion.
            payload.writeInt(1)
            event.writeToParcel(payload, 0)
            payload.writeInt(INJECT_MODE_WAIT_FOR_FINISH)
            transactRemote(inputServiceBinder(), INPUT_TRANSACTION_INJECT_INPUT_EVENT, payload, reply)
            reply.readException()
        } finally {
            payload.recycle()
            reply.recycle()
        }
    }

    // Sends a transaction to an arbitrary system service as the Shizuku
    // server's uid through the generic opcode 1 forwarder.
    private fun transactRemote(target: IBinder, code: Int, payload: Parcel, reply: Parcel) {
        val binder = shizukuBinder ?: throw IllegalStateException("Shizuku binder not connected")
        val envelope = Parcel.obtain()
        try {
            envelope.writeInterfaceToken(SHIZUKU_DESCRIPTOR)
            envelope.writeStrongBinder(target)
            envelope.writeInt(code)
            payload.setDataPosition(0)
            envelope.appendFrom(payload, 0, payload.dataSize())
            binder.transact(SHIZUKU_TRANSACTION_TRANSACT, envelope, reply, 0)
        } finally {
            envelope.recycle()
        }
    }

    // android.os.ServiceManager is hidden; plain reflection is fine on the
    // legacy Android versions this backend targets.
    private fun inputServiceBinder(): IBinder {
        inputBinder?.let {
            if (it.isBinderAlive) {
                return it
            }
            inputBinder = null
        }
        val serviceManager = Class.forName("android.os.ServiceManager")
        val getService = serviceManager.getMethod("getService", String::class.java)
        // Context.INPUT_SERVICE
        val binder = getService.invoke(null, "input") as IBinder
            ?: throw IllegalStateException("input service binder not found")
        inputBinder = binder
        return binder
    }

    private fun sendToken(binder: IBinder) {
        val token = loadToken() ?: return
        executor.execute {
            try {
                val payload = Parcel.obtain()
                val reply = Parcel.obtain()
                try {
                    payload.writeInterfaceToken(SHIZUKU_DESCRIPTOR)
                    payload.writeString(token)
                    binder.transact(SHIZUKU_TRANSACTION_SET_UID_TOKEN, payload, reply, 0)
                    reply.readException()
                    val ok = reply.readInt() != 0
                    Log.i(TAG, "setUidToken: $ok")
                    if (!ok) {
                        // Stale token: the server was restarted since the last
                        // authorization, so drop it and prompt again.
                        appContext?.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
                            ?.edit()?.clear()?.apply()
                        promptAuthorizationIfNeeded(force = true)
                    }
                } finally {
                    payload.recycle()
                    reply.recycle()
                }
            } catch (e: Throwable) {
                Log.e(TAG, "setUidToken failed; restart Shizuku and reopen RustDesk to authorize again", e)
            }
        }
    }

    private fun loadToken(): String? {
        val context = appContext ?: return null
        val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        val mostSig = prefs.getLong(KEY_TOKEN_MOST_SIG, 0L)
        val leastSig = prefs.getLong(KEY_TOKEN_LEAST_SIG, 0L)
        if (mostSig == 0L && leastSig == 0L) {
            return null
        }
        return UUID(mostSig, leastSig).toString()
    }

    private fun saveToken(mostSig: Long, leastSig: Long) {
        val context = appContext ?: return
        context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
            .edit()
            .putLong(KEY_TOKEN_MOST_SIG, mostSig)
            .putLong(KEY_TOKEN_LEAST_SIG, leastSig)
            .apply()
    }
}

/**
 * Receives the Shizuku server binder. The server locates this provider by the
 * fixed authority "<package>.shizuku" (RikkaApps/Shizuku v3.6.1,
 * ShizukuService.sendBinderToUserApp) and calls it from the shell uid, which
 * is also the only caller able to pass the INTERACT_ACROSS_USERS_FULL check.
 */
class ShizukuBinderReceiver : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun call(method: String, arg: String?, extras: Bundle?): Bundle? {
        if (method == "sendBinder" && extras != null) {
            extras.setClassLoader(BinderContainer::class.java.classLoader)
            @Suppress("DEPRECATION")
            val container = extras.getParcelable<BinderContainer>(ShizukuLegacyInput.EXTRA_BINDER)
            container?.binder?.let { ShizukuLegacyInput.onBinderReceived(it) }
        }
        return null
    }

    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?
    ): Cursor? = null

    override fun getType(uri: Uri): String? = null

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?
    ): Int = 0
}
