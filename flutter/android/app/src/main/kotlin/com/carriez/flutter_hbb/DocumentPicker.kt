package com.carriez.flutter_hbb

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.provider.DocumentsContract
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import kotlin.concurrent.thread

internal class DocumentPicker(private val activity: Activity) {
    private var pending: Pair<MethodChannel.Result, Boolean>? = null
    private var pendingLogFolder: MethodChannel.Result? = null
    private val storage = (activity.application as MainApplication).documentStorage

    fun handle(call: MethodCall, result: MethodChannel.Result): Boolean {
        when (call.method) {
            "document_require_log_folder" ->
                requireLogFolder(result, call.argument<Boolean>("checkOnly") == true)
            "document_workspace" -> if (call.argument<Boolean>("share") == true) {
                run(result) { storage.shareWorkspace() }
            } else {
                result.success((activity.getExternalFilesDir(null) ?: activity.filesDir).path)
            }
            "document_folder" -> pick(result, call.argument<Boolean>("share") == true)
            else -> return false
        }
        return true
    }

    private fun pick(result: MethodChannel.Result, share: Boolean) {
        if (pending != null || pendingLogFolder != null) {
            result.error("picker_in_progress", "Another document picker is already open", null)
            return
        }
        try {
            val intent = treePickerIntent("primary:Documents")
            pending = result to share
            activity.startActivityForResult(intent, REQUEST_DOCUMENT)
        } catch (error: Exception) {
            pending = null
            result.error("picker_unavailable", error.message, null)
        }
    }

    fun onResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean {
        if (requestCode == REQUEST_LOG_FOLDER) {
            val result = pendingLogFolder ?: return true
            pendingLogFolder = null
            if (resultCode != Activity.RESULT_OK || data?.data == null) {
                result.success(false)
            } else {
                run(result) {
                    storage.logs.select(requireNotNull(data.data), data.flags)
                    true
                }
            }
            return true
        }
        if (requestCode != REQUEST_DOCUMENT) return false
        val (result, share) = pending ?: return true
        pending = null
        if (resultCode != Activity.RESULT_OK || data == null) {
            result.success(null)
            return true
        }
        run(result) {
            storage.select(requireNotNull(data.data), data.flags, share).toString()
        }
        return true
    }

    fun close() {
        pendingLogFolder?.error("picker_closed", "Log directory selection was interrupted", null)
        pendingLogFolder = null
        pending?.first?.error("picker_closed", "Document selection was interrupted", null)
        pending = null
    }

    private fun run(result: MethodChannel.Result, action: () -> Any?) {
        thread {
            try {
                val value = action()
                activity.runOnUiThread { result.success(value) }
            } catch (error: Exception) {
                activity.runOnUiThread { result.error("document_failed", error.message, null) }
            }
        }
    }

    private fun requireLogFolder(result: MethodChannel.Result, checkOnly: Boolean) {
        if (pending != null || pendingLogFolder != null) {
            result.error("picker_in_progress", "Another document picker is already open", null)
            return
        }
        pendingLogFolder = result
        thread {
            val ready = runCatching { storage.logs.ensureReady() }
            activity.runOnUiThread { completeLogFolderCheck(result, ready, checkOnly) }
        }
    }

    private fun completeLogFolderCheck(
        result: MethodChannel.Result, ready: Result<Boolean>, checkOnly: Boolean
    ) {
        if (pendingLogFolder !== result) return
        ready.fold(onSuccess = {
            if (it || checkOnly) {
                pendingLogFolder = null
                result.success(it)
            } else {
                pickLogFolder(result)
            }
        }, onFailure = {
            pendingLogFolder = null
            result.error("log_directory_failed", it.message, null)
        })
    }

    private fun treePickerIntent(initialId: String): Intent =
        Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or
                Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                putExtra(DocumentsContract.EXTRA_INITIAL_URI,
                    DocumentsContract.buildDocumentUri(DocumentLogs.AUTHORITY, initialId))
            }
        }

    private fun pickLogFolder(result: MethodChannel.Result) {
        try {
            activity.startActivityForResult(treePickerIntent(DocumentLogs.PARENT_ID), REQUEST_LOG_FOLDER)
        } catch (error: Exception) {
            pendingLogFolder = null
            result.error("picker_unavailable", error.message, null)
        }
    }

    companion object {
        private const val REQUEST_DOCUMENT = 4251
        private const val REQUEST_LOG_FOLDER = 4252
    }
}
