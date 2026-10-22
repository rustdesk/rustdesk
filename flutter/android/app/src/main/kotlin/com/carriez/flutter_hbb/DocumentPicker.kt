package com.carriez.flutter_hbb

import android.app.Activity
import android.content.Intent
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import kotlin.concurrent.thread

internal class DocumentPicker(private val activity: Activity) {
    private var pending: Pair<MethodChannel.Result, Boolean>? = null
    private val storage = (activity.application as MainApplication).documentStorage

    fun handle(call: MethodCall, result: MethodChannel.Result): Boolean {
        when (call.method) {
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
        if (pending != null) {
            result.error("picker_in_progress", "Another document picker is already open", null)
            return
        }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or
            Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
        try {
            pending = result to share
            activity.startActivityForResult(intent, REQUEST_DOCUMENT)
        } catch (error: Exception) {
            pending = null
            result.error("picker_unavailable", error.message, null)
        }
    }

    fun onResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean {
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

    companion object {
        private const val REQUEST_DOCUMENT = 4251
    }
}
