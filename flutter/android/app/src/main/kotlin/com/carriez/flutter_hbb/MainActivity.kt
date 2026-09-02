package com.carriez.flutter_hbb

/**
 * Handle events from flutter
 * Request MediaProjection permission
 *
 * Inspired by [droidVNC-NG] https://github.com/bk138/droidVNC-NG
 */

import ffi.FFI

import android.app.Activity
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.content.ClipboardManager
import android.os.Bundle
import android.os.Build
import android.os.IBinder
import android.util.Log
import android.view.WindowManager
import android.media.MediaCodecInfo
import android.media.MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface
import android.media.MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420SemiPlanar
import android.media.MediaCodecList
import android.media.MediaFormat
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import android.util.DisplayMetrics
import androidx.annotation.RequiresApi
import org.json.JSONArray
import org.json.JSONObject
import com.hjq.permissions.XXPermissions
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import kotlin.concurrent.thread
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream


class MainActivity : FlutterActivity() {
    companion object {
        var flutterMethodChannel: MethodChannel? = null
        private var _rdClipboardManager: RdClipboardManager? = null
        val rdClipboardManager: RdClipboardManager?
            get() = _rdClipboardManager;
    }

    private val channelTag = "mChannel"
    private val logTag = "mMainActivity"
    private var mainService: MainService? = null
    private sealed class PendingPicker {
        data class ImportFiles(val result: MethodChannel.Result) : PendingPicker()
        data class ExportFile(val source: File, val result: MethodChannel.Result) : PendingPicker()
        data class ImportDirectory(val result: MethodChannel.Result) : PendingPicker()
        data class ExportFiles(
            val sources: List<File>,
            val rejected: Int,
            val result: MethodChannel.Result
        ) : PendingPicker()
    }

    private data class ExportSource(
        val file: File,
        val children: List<ExportSource>?
    )

    private var pendingPicker: PendingPicker? = null

    private var isAudioStart = false
    private val audioRecordHandle = AudioRecordHandle(this, { false }, { isAudioStart })

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        if (MainService.isReady) {
            Intent(activity, MainService::class.java).also {
                bindService(it, serviceConnection, Context.BIND_AUTO_CREATE)
            }
        }
        flutterMethodChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            channelTag
        )
        initFlutterChannel(flutterMethodChannel!!)
        thread {
            try {
                setCodecInfo()
            } catch (e: Exception) {
                Log.e("MainActivity", "Failed to setCodecInfo: ${e.message}", e)
            }
        }
    }

    override fun onResume() {
        super.onResume()
        val inputPer = InputService.isOpen
        activity.runOnUiThread {
            flutterMethodChannel?.invokeMethod(
                "on_state_changed",
                mapOf("name" to "input", "value" to inputPer.toString())
            )
        }
    }

    private fun requestMediaProjection() {
        val intent = Intent(this, PermissionRequestTransparentActivity::class.java).apply {
            action = ACT_REQUEST_MEDIA_PROJECTION
        }
        startActivityForResult(intent, REQ_INVOKE_PERMISSION_ACTIVITY_MEDIA_PROJECTION)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQ_IMPORT_FILES) {
            val pending = pendingPicker as? PendingPicker.ImportFiles ?: return
            pendingPicker = null
            if (resultCode != Activity.RESULT_OK || data == null) {
                pending.result.success(emptyList<Map<String, String>>())
                return
            }

            val uris = linkedSetOf<Uri>()
            data.data?.let { uris.add(it) }
            data.clipData?.let { clipData ->
                for (index in 0 until clipData.itemCount) {
                    uris.add(clipData.getItemAt(index).uri)
                }
            }
            thread {
                val files = uris.map { uri ->
                    mapOf(
                        "uri" to uri.toString(),
                        "name" to (displayName(uri) ?: uri.lastPathSegment.orEmpty())
                    )
                }
                runOnUiThread { pending.result.success(files) }
            }
            return
        }
        if (requestCode == REQ_EXPORT_FILE) {
            val pending = pendingPicker as? PendingPicker.ExportFile ?: return
            pendingPicker = null
            val destination = data?.data

            if (resultCode != Activity.RESULT_OK || destination == null) {
                pending.result.success(false)
                return
            }

            thread {
                try {
                    FileInputStream(pending.source).use { input ->
                        contentResolver.openOutputStream(destination, "wt")?.use { output ->
                            input.copyTo(output)
                        } ?: throw IllegalStateException("Unable to open the selected destination")
                    }
                    runOnUiThread { pending.result.success(true) }
                } catch (e: Exception) {
                    Log.e(logTag, "Failed to export file", e)
                    runOnUiThread {
                        pending.result.error("export_failed", e.message, null)
                    }
                }
            }
            return
        }
        if (requestCode == REQ_IMPORT_DIRECTORY) {
            val pending = pendingPicker as? PendingPicker.ImportDirectory ?: return
            pendingPicker = null
            val treeUri = data?.data
            if (resultCode != Activity.RESULT_OK || treeUri == null) {
                pending.result.success(null)
                return
            }
            thread {
                val selected = mapOf(
                    "uri" to treeUri.toString(),
                    "name" to (treeDisplayName(treeUri) ?: "Imported")
                )
                runOnUiThread { pending.result.success(selected) }
            }
            return
        }
        if (requestCode == REQ_EXPORT_FILES) {
            val pending = pendingPicker as? PendingPicker.ExportFiles ?: return
            pendingPicker = null
            val treeUri = data?.data
            if (resultCode != Activity.RESULT_OK || treeUri == null) {
                pending.result.success(null)
                return
            }
            thread {
                var exported = 0
                var failed = pending.rejected
                var processed = 0
                try {
                    val sources = pending.sources.map { snapshotExportSource(it) }
                    val rootDocId = DocumentsContract.getTreeDocumentId(treeUri)
                    sources.forEach { source ->
                        val ok = source?.let {
                            copyExportSourceToTree(treeUri, rootDocId, it)
                        } ?: false
                        if (ok) exported++ else failed++
                        processed++
                    }
                } catch (e: Exception) {
                    Log.e(logTag, "Failed to export selected files", e)
                    failed += pending.sources.size - processed
                }
                runOnUiThread {
                    pending.result.success(mapOf("exported" to exported, "failed" to failed))
                }
            }
            return
        }
        if (requestCode == REQ_INVOKE_PERMISSION_ACTIVITY_MEDIA_PROJECTION && resultCode == RES_FAILED) {
            flutterMethodChannel?.invokeMethod("on_media_projection_canceled", null)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (_rdClipboardManager == null) {
            _rdClipboardManager = RdClipboardManager(getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
            FFI.setClipboardManager(_rdClipboardManager!!)
        }
    }

    override fun onDestroy() {
        Log.e(logTag, "onDestroy")
        // The process can outlive the UI whenever something keeps it alive:
        // MainService, or the accessibility InputService on its own. Only the
        // former gets onTaskRemoved, so close outgoing sessions here too,
        // otherwise a session survives with no UI left to close it.
        // `isFinishing` distinguishes the user really leaving from a destroy
        // for recreation (configuration change, "don't keep activities"),
        // which must not tear down a live session.
        if (isFinishing) {
            FFI.closeAllSessions()
        }
        mainService?.let {
            unbindService(serviceConnection)
        }
        super.onDestroy()
    }

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            Log.d(logTag, "onServiceConnected")
            val binder = service as MainService.LocalBinder
            mainService = binder.getService()
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.d(logTag, "onServiceDisconnected")
            mainService = null
        }
    }

    private fun initFlutterChannel(flutterMethodChannel: MethodChannel) {
        flutterMethodChannel.setMethodCallHandler { call, result ->
            // make sure result will be invoked, otherwise flutter will await forever
            when (call.method) {
                "init_service" -> {
                    Intent(activity, MainService::class.java).also {
                        bindService(it, serviceConnection, Context.BIND_AUTO_CREATE)
                    }
                    if (MainService.isReady) {
                        result.success(false)
                        return@setMethodCallHandler
                    }
                    requestMediaProjection()
                    result.success(true)
                }
                "start_capture" -> {
                    mainService?.let {
                        result.success(it.startCapture())
                    } ?: let {
                        result.success(false)
                    }
                }
                "stop_service" -> {
                    Log.d(logTag, "Stop service")
                    mainService?.let {
                        it.destroy()
                        result.success(true)
                    } ?: let {
                        result.success(false)
                    }
                }
                "check_permission" -> {
                    if (call.arguments is String) {
                        result.success(XXPermissions.isGranted(context, call.arguments as String))
                    } else {
                        result.success(false)
                    }
                }
                "request_permission" -> {
                    if (call.arguments is String) {
                        requestPermission(context, call.arguments as String)
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                START_ACTION -> {
                    if (call.arguments is String) {
                        startAction(context, call.arguments as String)
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                "check_video_permission" -> {
                    mainService?.let {
                        result.success(it.checkMediaPermission())
                    } ?: let {
                        result.success(false)
                    }
                }
                "check_service" -> {
                    Companion.flutterMethodChannel?.invokeMethod(
                        "on_state_changed",
                        mapOf("name" to "input", "value" to InputService.isOpen.toString())
                    )
                    Companion.flutterMethodChannel?.invokeMethod(
                        "on_state_changed",
                        mapOf("name" to "media", "value" to MainService.isReady.toString())
                    )
                    result.success(true)
                }
                "stop_input" -> {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                        InputService.ctx?.disableSelf()
                    } else {
                        InputService.ctx = null
                        Companion.flutterMethodChannel?.invokeMethod(
                            "on_state_changed",
                            mapOf("name" to "input", "value" to InputService.isOpen.toString())
                        )
                    }
                    result.success(true)
                }
                "cancel_notification" -> {
                    if (call.arguments is Int) {
                        val id = call.arguments as Int
                        mainService?.cancelNotification(id)
                    } else {
                        result.success(true)
                    }
                }
                "enable_soft_keyboard" -> {
                    // https://blog.csdn.net/hanye2020/article/details/105553780
                    if (call.arguments as Boolean) {
                        window.clearFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
                    } else {
                        window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
                    }
                    result.success(true)

                }
                "try_sync_clipboard" -> {
                    rdClipboardManager?.syncClipboard(true)
                    result.success(true)
                }
                GET_START_ON_BOOT_OPT -> {
                    val prefs = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)
                    result.success(prefs.getBoolean(KEY_START_ON_BOOT_OPT, false))
                }
                SET_START_ON_BOOT_OPT -> {
                    if (call.arguments is Boolean) {
                        val prefs = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)
                        val edit = prefs.edit()
                        edit.putBoolean(KEY_START_ON_BOOT_OPT, call.arguments as Boolean)
                        edit.apply()
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                SYNC_APP_DIR_CONFIG_PATH -> {
                    if (call.arguments is String) {
                        val prefs = getSharedPreferences(KEY_SHARED_PREFERENCES, MODE_PRIVATE)
                        val edit = prefs.edit()
                        edit.putString(KEY_APP_DIR_CONFIG_PATH, call.arguments as String)
                        edit.apply()
                        result.success(true)
                    } else {
                        result.success(false)
                    }
                }
                PICK_IMPORT_FILES -> {
                    if (pendingPicker != null) {
                        result.error("picker_in_progress", "Another document picker is already open", null)
                    } else {
                        pendingPicker = PendingPicker.ImportFiles(result)
                        try {
                            startActivityForResult(
                                Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                    addCategory(Intent.CATEGORY_OPENABLE)
                                    type = "*/*"
                                    putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                                },
                                REQ_IMPORT_FILES
                            )
                        } catch (e: Exception) {
                            pendingPicker = null
                            result.error("picker_unavailable", e.message, null)
                        }
                    }
                }
                IMPORT_FILE -> {
                    val arguments = call.arguments as? Map<*, *>
                    val uri = (arguments?.get("uri") as? String)?.let {
                        runCatching { Uri.parse(it) }.getOrNull()
                    }
                    val path = arguments?.get("path") as? String
                    val overwrite = arguments?.get("overwrite") as? Boolean ?: false
                    val destination = path?.let { canonicalAppScopedFile(it) }

                    if (uri?.scheme != "content") {
                        result.error("invalid_uri", "The selected document URI is invalid", null)
                    } else if (destination == null ||
                        destination.isDirectory ||
                        destination.parentFile?.isDirectory != true) {
                        result.error("invalid_destination", "The destination is outside app-scoped storage", null)
                    } else {
                        thread {
                            var temporary: File? = null
                            var reservedDestination = false
                            var errorCode = "import_failed"
                            try {
                                val temporaryFile = File.createTempFile(
                                    ".rustdesk-import-",
                                    ".tmp",
                                    destination.parentFile
                                )
                                temporary = temporaryFile
                                contentResolver.openInputStream(uri)?.use { input ->
                                    FileOutputStream(temporaryFile).use { output ->
                                        input.copyTo(output)
                                    }
                                } ?: throw IllegalStateException("Unable to open the selected document")
                                if (!overwrite) {
                                    reservedDestination = destination.createNewFile()
                                    if (!reservedDestination) {
                                        throw IllegalStateException("The destination already exists")
                                    }
                                }
                                if (!temporaryFile.renameTo(destination)) {
                                    if (reservedDestination) {
                                        destination.delete()
                                    }
                                    errorCode = "rename_failed"
                                    throw IllegalStateException("Unable to replace the destination")
                                }
                                runOnUiThread { result.success(true) }
                            } catch (e: Exception) {
                                Log.e(logTag, "Failed to import file", e)
                                runOnUiThread {
                                    result.error(errorCode, e.message, null)
                                }
                            } finally {
                                temporary?.delete()
                            }
                        }
                    }
                }
                EXPORT_FILE -> {
                    val path = (call.arguments as? Map<*, *>)?.get("path") as? String
                    val source = path?.let { canonicalExportSource(it) }

                    if (source?.isFile != true) {
                        result.error("invalid_source", "The file is outside app-scoped storage", null)
                    } else if (pendingPicker != null) {
                        result.error("picker_in_progress", "Another document picker is already open", null)
                    } else {
                        val mimeType = MimeTypeMap.getSingleton()
                            .getMimeTypeFromExtension(source.extension.lowercase())
                            ?: "application/octet-stream"
                        pendingPicker = PendingPicker.ExportFile(source, result)
                        try {
                            startActivityForResult(
                                Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                                    addCategory(Intent.CATEGORY_OPENABLE)
                                    type = mimeType
                                    putExtra(Intent.EXTRA_TITLE, source.name)
                                },
                                REQ_EXPORT_FILE
                            )
                        } catch (e: Exception) {
                            pendingPicker = null
                            result.error("picker_unavailable", e.message, null)
                        }
                    }
                }
                PICK_IMPORT_DIRECTORY -> {
                    if (pendingPicker != null) {
                        result.error("picker_in_progress", "Another document picker is already open", null)
                    } else {
                        pendingPicker = PendingPicker.ImportDirectory(result)
                        try {
                            startActivityForResult(
                                Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                                    putExtra(Intent.EXTRA_TITLE, "Select the folder to import")
                                },
                                REQ_IMPORT_DIRECTORY
                            )
                        } catch (e: Exception) {
                            pendingPicker = null
                            result.error("picker_unavailable", e.message, null)
                        }
                    }
                }
                IMPORT_DIRECTORY -> {
                    val arguments = call.arguments as? Map<*, *>
                    val uri = (arguments?.get("uri") as? String)?.let {
                        runCatching { Uri.parse(it) }.getOrNull()
                    }
                    val path = arguments?.get("path") as? String
                    val overwrite = arguments?.get("overwrite") as? Boolean ?: false
                    val destination = path?.let { canonicalAppScopedFile(it) }

                    if (uri?.scheme != "content") {
                        result.error("invalid_uri", "The selected document URI is invalid", null)
                    } else if (destination == null ||
                        destination.parentFile?.isDirectory != true ||
                        (destination.exists() && !destination.isDirectory)) {
                        result.error("invalid_destination", "The destination is outside app-scoped storage", null)
                    } else {
                        thread {
                            var temporary: File? = null
                            var backup: File? = null
                            val ok = try {
                                val parent = destination.parentFile
                                    ?: throw IllegalStateException("The destination has no parent")
                                temporary = File.createTempFile(
                                    ".rustdesk-import-dir-",
                                    ".tmp",
                                    parent
                                ).also {
                                    if (!it.delete() || !it.mkdir()) {
                                        throw IllegalStateException("Unable to create a temporary folder")
                                    }
                                }
                                if (!copyDocumentTreeToFile(uri, temporary!!)) {
                                    throw IllegalStateException("Unable to read all folder contents")
                                }
                                if (destination.exists()) {
                                    if (!overwrite) {
                                        throw IllegalStateException("The destination already exists")
                                    }
                                    val backupFile = File.createTempFile(
                                        ".rustdesk-import-backup-",
                                        ".tmp",
                                        parent
                                    )
                                    if (!backupFile.delete()) {
                                        throw IllegalStateException("Unable to prepare the destination backup")
                                    }
                                    backup = backupFile
                                    if (!destination.renameTo(backupFile)) {
                                        throw IllegalStateException("Unable to replace the destination")
                                    }
                                }
                                if (!temporary!!.renameTo(destination)) {
                                    val destinationBackup = backup
                                    if (destinationBackup != null &&
                                        !destinationBackup.renameTo(destination)
                                    ) {
                                        throw IllegalStateException(
                                            "Unable to move the imported folder and restore " +
                                                "the destination from $destinationBackup"
                                        )
                                    }
                                    throw IllegalStateException("Unable to move the imported folder")
                                }
                                temporary = null
                                val destinationBackup = backup
                                if (destinationBackup != null &&
                                    !destinationBackup.deleteRecursively()
                                ) {
                                    throw IllegalStateException(
                                        "Unable to remove the destination backup: $destinationBackup"
                                    )
                                }
                                backup = null
                                true
                            } catch (e: Exception) {
                                Log.e(logTag, "Failed to import directory", e)
                                false
                            } finally {
                                temporary?.deleteRecursively()
                            }
                            runOnUiThread { result.success(ok) }
                        }
                    }
                }
                EXPORT_FILES -> {
                    val paths = (call.arguments as? Map<*, *>)?.get("paths") as? List<*>
                    if (paths.isNullOrEmpty()) {
                        result.error("invalid_source", "The selected files are outside app-scoped storage", null)
                    } else {
                        val sources = paths.mapNotNull {
                            (it as? String)?.let(::canonicalExportSource)
                        }
                        val rejected = paths.size - sources.size
                        if (sources.isEmpty()) {
                            result.success(mapOf("exported" to 0, "failed" to rejected))
                        } else if (pendingPicker != null) {
                            result.error("picker_in_progress", "Another document picker is already open", null)
                        } else {
                            pendingPicker = PendingPicker.ExportFiles(sources, rejected, result)
                            try {
                                startActivityForResult(
                                    Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                                        putExtra(Intent.EXTRA_TITLE, "Select the destination folder")
                                    },
                                    REQ_EXPORT_FILES
                                )
                            } catch (e: Exception) {
                                pendingPicker = null
                                result.error("picker_unavailable", e.message, null)
                            }
                        }
                    }
                }
                GET_VALUE -> {
                    if (call.arguments is String) {
                        if (call.arguments == KEY_IS_SUPPORT_VOICE_CALL) {
                            result.success(isSupportVoiceCall())
                        } else {
                            result.error("-1", "No such key", null)
                        }
                    } else {
                        result.success(null)
                    }
                }
                "on_voice_call_started" -> {
                    onVoiceCallStarted()
                }
                "on_voice_call_closed" -> {
                    onVoiceCallClosed()
                }
                else -> {
                    result.error("-1", "No such method", null)
                }
            }
        }
    }

    private fun canonicalAppScopedFile(path: String): File? {
        val file = runCatching { File(path).canonicalFile }.getOrNull() ?: return null
        val allowedRoots = listOfNotNull(filesDir, getExternalFilesDir(null)).mapNotNull {
            runCatching { it.canonicalFile }.getOrNull()
        }
        return file.takeIf { candidate ->
            allowedRoots.any { root ->
                candidate == root || candidate.path.startsWith(root.path + File.separator)
            }
        }
    }

    private fun canonicalExportSource(path: String): File? {
        val original = File(path).absoluteFile
        val canonical = canonicalAppScopedFile(path) ?: return null
        return canonical.takeIf {
            original.path == canonical.path && (canonical.isFile || canonical.isDirectory)
        }
    }

    private fun snapshotExportSource(source: File): ExportSource? {
        val safeSource = canonicalExportSource(source.path) ?: return null
        if (safeSource.isFile) return ExportSource(safeSource, null)
        val sourceChildren = safeSource.listFiles() ?: return null
        val children = ArrayList<ExportSource>(sourceChildren.size)
        for (child in sourceChildren) {
            val snapshot = snapshotExportSource(child) ?: return null
            children.add(snapshot)
        }
        return ExportSource(safeSource, children)
    }

    private fun copyExportSourceToTree(
        treeUri: Uri,
        parentDocId: String,
        source: ExportSource
    ): Boolean {
        val children = source.children
        return if (children == null) {
            copyFileToTree(treeUri, parentDocId, source.file)
        } else {
            copyDirToTree(treeUri, parentDocId, source)
        }
    }

    private fun treeDisplayName(treeUri: Uri): String? {
        return try {
            val rootDocId = DocumentsContract.getTreeDocumentId(treeUri)
            val docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, rootDocId)
            contentResolver.query(
                docUri,
                arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME),
                null,
                null,
                null
            )?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        } catch (e: Exception) {
            Log.w(logTag, "Failed to read selected folder name", e)
            null
        }
    }

    private fun copyDocumentTreeToFile(treeUri: Uri, destinationDir: File): Boolean {
        val rootDocId = DocumentsContract.getTreeDocumentId(treeUri)
        return copyChildrenToFile(treeUri, rootDocId, destinationDir)
    }

    private fun copyChildrenToFile(
        treeUri: Uri,
        parentDocId: String,
        destinationDir: File
    ): Boolean {
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
        var ok = true
        val destinationNames = HashSet<String>()
        val cursor = contentResolver.query(childrenUri, childColumns, null, null, null)
            ?: return false
        cursor.use {
            while (cursor.moveToNext()) {
                val docId = cursor.getString(0)
                val name = cursor.getString(1)
                val mime = cursor.getString(2)
                val docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId)
                if (name != null && !destinationNames.add(name)) {
                    ok = false
                    continue
                }
                val destination = safeDestinationChild(destinationDir, name)
                if (destination == null || destination.exists()) {
                    ok = false
                    continue
                }
                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    if (!destination.mkdirs() && !destination.isDirectory) {
                        ok = false
                        continue
                    }
                    if (!copyChildrenToFile(treeUri, docId, destination)) {
                        ok = false
                    }
                } else if (!copyDocumentToFile(docUri, destination)) {
                    ok = false
                }
            }
        }
        return ok
    }

    private fun safeDestinationChild(destinationDir: File, name: String?): File? {
        if (name.isNullOrEmpty() || name == "." || name == ".." ||
            name.indexOf('\u0000') >= 0 || name.contains('/') || name.contains('\\')) {
            return null
        }
        val parent = runCatching { destinationDir.canonicalFile }.getOrNull() ?: return null
        val child = runCatching { File(parent, name).canonicalFile }.getOrNull() ?: return null
        return child.takeIf { it.path.startsWith(parent.path + File.separator) }
    }

    private fun copyDocumentToFile(uri: Uri, destination: File): Boolean {
        return try {
            destination.parentFile?.mkdirs()
            if (destination.exists() && !destination.delete()) {
                return false
            }
            contentResolver.openInputStream(uri)?.use { input ->
                FileOutputStream(destination).use { output -> input.copyTo(output) }
            } != null
        } catch (e: Exception) {
            Log.e(logTag, "Failed to copy document to $destination", e)
            false
        }
    }

    private fun copyFileToTree(treeUri: Uri, parentDocId: String, source: File): Boolean {
        val safeSource = canonicalExportSource(source.path)?.takeIf { it.isFile } ?: return false
        return try {
            val mime = MimeTypeMap.getSingleton()
                .getMimeTypeFromExtension(safeSource.extension.lowercase())
                ?: "application/octet-stream"
            val parentUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, parentDocId)
            val docUri = DocumentsContract.createDocument(
                contentResolver,
                parentUri,
                mime,
                safeSource.name
            ) ?: return false
            contentResolver.openOutputStream(docUri, "wt")?.use { output ->
                FileInputStream(safeSource).use { input -> input.copyTo(output) }
            } ?: return false
            true
        } catch (e: Exception) {
            Log.e(logTag, "Failed to export file $safeSource", e)
            false
        }
    }

    private fun copyDirToTree(
        treeUri: Uri,
        parentDocId: String,
        source: ExportSource
    ): Boolean {
        val children = source.children ?: return false
        val safeSource = canonicalExportSource(source.file.path)?.takeIf { it.isDirectory }
            ?: return false
        val parentUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, parentDocId)
        var dirDocId = findChildDocId(treeUri, parentDocId, safeSource.name)
        if (dirDocId == null) {
            dirDocId = try {
                DocumentsContract.createDocument(
                    contentResolver,
                    parentUri,
                    DocumentsContract.Document.MIME_TYPE_DIR,
                    safeSource.name
                )?.let { DocumentsContract.getDocumentId(it) }
            } catch (e: Exception) {
                Log.e(logTag, "Failed to create folder ${safeSource.name}", e)
                null
            }
        }
        if (dirDocId == null) return false

        var ok = true
        children.forEach { child ->
            val childOk = copyExportSourceToTree(treeUri, dirDocId, child)
            if (!childOk) ok = false
        }
        return ok
    }

    private fun findChildDocId(treeUri: Uri, parentDocId: String, name: String): String? {
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
        val cursor = contentResolver.query(childrenUri, childColumns, null, null, null)
            ?: throw IllegalStateException("Unable to query destination folder")
        cursor.use {
            while (cursor.moveToNext()) {
                if (cursor.getString(1) == name &&
                    cursor.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR
                ) {
                    return cursor.getString(0)
                }
            }
        }
        return null
    }

    private val childColumns = arrayOf(
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_MIME_TYPE
    )

    private fun displayName(uri: Uri): String? {
        return try {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        } catch (e: Exception) {
            Log.w(logTag, "Failed to read selected document name", e)
            null
        }
    }

    private fun setCodecInfo() {
        val codecList = MediaCodecList(MediaCodecList.REGULAR_CODECS)
        val codecs = codecList.codecInfos
        val codecArray = JSONArray()

        val windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val wh = getScreenSize(windowManager)
        var w = wh.first
        var h = wh.second
        val align = 64
        w = (w + align - 1) / align * align
        h = (h + align - 1) / align * align
        codecs.forEach { codec ->
            val codecObject = JSONObject()
            codecObject.put("name", codec.name)
            codecObject.put("is_encoder", codec.isEncoder)
            var hw: Boolean? = null;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                hw = codec.isHardwareAccelerated
            } else {
                // https://chromium.googlesource.com/external/webrtc/+/HEAD/sdk/android/src/java/org/webrtc/MediaCodecUtils.java#29
                // https://chromium.googlesource.com/external/webrtc/+/master/sdk/android/api/org/webrtc/HardwareVideoEncoderFactory.java#229
                if (listOf("OMX.google.", "OMX.SEC.", "c2.android").any { codec.name.startsWith(it, true) }) {
                    hw = false
                } else if (listOf("c2.qti", "OMX.qcom.video", "OMX.Exynos", "OMX.hisi", "OMX.MTK", "OMX.Intel", "OMX.Nvidia").any { codec.name.startsWith(it, true) }) {
                    hw = true
                }
            }
            if (hw != true) {
                return@forEach
            }
            codecObject.put("hw", hw)
            var mime_type = ""
            codec.supportedTypes.forEach { type ->
                if (listOf("video/avc", "video/hevc").contains(type)) { // "video/x-vnd.on2.vp8", "video/x-vnd.on2.vp9", "video/av01"
                    mime_type = type;
                }
            }
            if (mime_type.isNotEmpty()) {
                codecObject.put("mime_type", mime_type)
                val caps = codec.getCapabilitiesForType(mime_type)
                if (codec.isEncoder) {
                    // Encoder's max_height and max_width are interchangeable
                    if (!caps.videoCapabilities.isSizeSupported(w,h) && !caps.videoCapabilities.isSizeSupported(h,w)) {
                        return@forEach
                    }
                }
                codecObject.put("min_width", caps.videoCapabilities.supportedWidths.lower)
                codecObject.put("max_width", caps.videoCapabilities.supportedWidths.upper)
                codecObject.put("min_height", caps.videoCapabilities.supportedHeights.lower)
                codecObject.put("max_height", caps.videoCapabilities.supportedHeights.upper)
                val surface = caps.colorFormats.contains(COLOR_FormatSurface);
                codecObject.put("surface", surface)
                val nv12 = caps.colorFormats.contains(COLOR_FormatYUV420SemiPlanar)
                codecObject.put("nv12", nv12)
                if (!(nv12 || surface)) {
                    return@forEach
                }
                codecObject.put("min_bitrate", caps.videoCapabilities.bitrateRange.lower / 1000)
                codecObject.put("max_bitrate", caps.videoCapabilities.bitrateRange.upper / 1000)
                if (!codec.isEncoder) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                        codecObject.put("low_latency", caps.isFeatureSupported(MediaCodecInfo.CodecCapabilities.FEATURE_LowLatency))
                    }
                }
                if (!codec.isEncoder) {
                    return@forEach
                }
                codecArray.put(codecObject)
            }
        }
        val result = JSONObject()
        result.put("version", Build.VERSION.SDK_INT)
        result.put("w", w)
        result.put("h", h)
        result.put("codecs", codecArray)
        FFI.setCodecInfo(result.toString())
    }

    private fun onVoiceCallStarted() {
        var ok = false
        mainService?.let {
            ok = it.onVoiceCallStarted()
        } ?: let {
            isAudioStart = true
            ok = audioRecordHandle.onVoiceCallStarted(null)
        }
        if (!ok) {
            // Rarely happens, So we just add log and msgbox here.
            Log.e(logTag, "onVoiceCallStarted fail")
            flutterMethodChannel?.invokeMethod("msgbox", mapOf(
                "type" to "custom-nook-nocancel-hasclose-error",
                "title" to "Voice call",
                "text" to "Failed to start voice call."))
        } else {
            Log.d(logTag, "onVoiceCallStarted success")
        }
    }

    private fun onVoiceCallClosed() {
        var ok = false
        mainService?.let {
            ok = it.onVoiceCallClosed()
        } ?: let {
            isAudioStart = false
            ok = audioRecordHandle.onVoiceCallClosed(null)
        }
        if (!ok) {
            // Rarely happens, So we just add log and msgbox here.
            Log.e(logTag, "onVoiceCallClosed fail")
            flutterMethodChannel?.invokeMethod("msgbox", mapOf(
                "type" to "custom-nook-nocancel-hasclose-error",
                "title" to "Voice call",
                "text" to "Failed to stop voice call."))
        } else {
            Log.d(logTag, "onVoiceCallClosed success")
        }
    }

    override fun onStop() {
        super.onStop()
        val disableFloatingWindow = FFI.getLocalOption("disable-floating-window") == "Y"
        if (!disableFloatingWindow && MainService.isReady) {
            startService(Intent(this, FloatingWindowService::class.java))
        }
    }

    override fun onStart() {
        super.onStart()
        stopService(Intent(this, FloatingWindowService::class.java))
    }
}
