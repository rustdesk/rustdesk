package com.carriez.flutter_hbb

import java.nio.ByteBuffer
import java.util.Timer
import java.util.TimerTask

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.util.Log
import androidx.annotation.Keep

import hbb.MessageOuterClass.ClipboardFormat
import hbb.MessageOuterClass.Clipboard
import hbb.MessageOuterClass.MultiClipboards

import ffi.FFI

class RdClipboardManager(private val clipboardManager: ClipboardManager) {
    private val logTag = "RdClipboardManager"
    private val supportedMimeTypes = arrayOf(
        ClipDescription.MIMETYPE_TEXT_PLAIN,
        ClipDescription.MIMETYPE_TEXT_HTML
    )

    // 1. Avoid listening to the same clipboard data updated by `rustUpdateClipboard`.
    // 2. Avoid sending the clipboard data before enabling client clipboard.
    //    1) Disable clipboard
    //    2) Copy text "a"
    //    3) Enable clipboard
    //    4) Switch to another app
    //    5) Switch back to the app
    //    6) "a" should not be sent to the client, because it's copied before enabling clipboard
    //
    // It's okay to that `rustEnableClientClipboard(false)` is called after `rustUpdateClipboard`,
    // though the `lastUpdatedClipData` will be set to null once.
    private var lastUpdatedClipData: ClipData? = null
    private var isClientEnabled = true;
    private var _isListening = false;
    val isListening: Boolean
        get() = _isListening

    fun checkPrimaryClip(isClient: Boolean, isSync: Boolean) {
        val clipData = clipboardManager.primaryClip
        if (clipData != null && clipData.itemCount > 0) {
            // Only handle the first item in the clipboard for now.
            val clip = clipData.getItemAt(0)
            val isHostSync = !isClient && isSync
            // Ignore the `isClipboardDataEqual()` check if it's a host sync operation.
            // Because it's a action manually triggered by the user.
            if (!isHostSync) {
                if (lastUpdatedClipData != null && isClipboardDataEqual(clipData, lastUpdatedClipData!!)) {
                    Log.d(logTag, "Clipboard data is the same as last update, ignore")
                    return
                }
            }
            val mimeTypeCount = clipData.description.getMimeTypeCount()
            val mimeTypes = mutableListOf<String>()
            for (i in 0 until mimeTypeCount) {
                mimeTypes.add(clipData.description.getMimeType(i))
            }
            var text: CharSequence? = null;
            var html: String? = null;
            if (isSupportedMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)) {
                text = clip?.text
            }
            if (isSupportedMimeType(ClipDescription.MIMETYPE_TEXT_HTML)) {
                text = clip?.text
                html = clip?.htmlText
            }
            var count = 0
            val clips = MultiClipboards.newBuilder()
            if (text != null) {
                val content = com.google.protobuf.ByteString.copyFromUtf8(text.toString())
                    clips.addClipboards(Clipboard.newBuilder().setFormat(ClipboardFormat.Text).setContent(content).build())
                    count++
                }
            if (html != null) {
                val content = com.google.protobuf.ByteString.copyFromUtf8(html)
                clips.addClipboards(Clipboard.newBuilder().setFormat(ClipboardFormat.Html).setContent(content).build())
                count++
            }
            if (count > 0) {
                val clipsBytes = clips.build().toByteArray()
                val isClientFlag = if (isClient) 1 else 0
                val clipsBuf = ByteBuffer.allocateDirect(clipsBytes.size + 1).apply {
                    put(isClientFlag.toByte())
                    put(clipsBytes)
                }
                clipsBuf.flip()
                lastUpdatedClipData = clipData
                Log.d(logTag, "${if (isClient) "client" else "host"}, send clipboard data to the remote")
                FFI.onClipboardUpdate(clipsBuf)
            }
        }
    }

    private val clipboardListener = object : ClipboardManager.OnPrimaryClipChangedListener {
        override fun onPrimaryClipChanged() {
            Log.d(logTag, "onPrimaryClipChanged")
            checkPrimaryClip(true, false)
        }
    }

    private fun isSupportedMimeType(mimeType: String): Boolean {
        return supportedMimeTypes.contains(mimeType)
    }

    private fun isClipboardDataEqual(left: ClipData, right: ClipData): Boolean {
        if (left.description.getMimeTypeCount() != right.description.getMimeTypeCount()) {
            return false
        }
        val mimeTypeCount = left.description.getMimeTypeCount()
        for (i in 0 until mimeTypeCount) {
            if (left.description.getMimeType(i) != right.description.getMimeType(i)) {
                return false
            }
        }

        if (left.itemCount != right.itemCount) {
            return false
        }
        for (i in 0 until left.itemCount) {
            val mimeType = left.description.getMimeType(i)
            if (!isSupportedMimeType(mimeType)) {
                continue
            }
            val leftItem = left.getItemAt(i)
            val rightItem = right.getItemAt(i)
            if (mimeType == ClipDescription.MIMETYPE_TEXT_PLAIN || mimeType == ClipDescription.MIMETYPE_TEXT_HTML) {
                if (leftItem.text != rightItem.text || leftItem.htmlText != rightItem.htmlText) {
                    return false
                }
            }
        }
        return true
    }

    @Keep
    fun rustEnableServiceClipboard(enable: Boolean) {
        Log.d(logTag, "rustEnableServiceClipboard: enable: $enable, _isListening: $_isListening")
        if (enable) {
            if (!_isListening) {
                clipboardManager.addPrimaryClipChangedListener(clipboardListener)
                _isListening = true
            }
        } else {
            if (_isListening) {
                clipboardManager.removePrimaryClipChangedListener(clipboardListener)
                _isListening = false
                lastUpdatedClipData = null
            }
        }
    }

    @Keep
    fun rustEnableClientClipboard(enable: Boolean) {
        Log.d(logTag, "rustEnableClientClipboard: enable: $enable")
        isClientEnabled = enable
        if (enable) {
            lastUpdatedClipData = clipboardManager.primaryClip
        } else {
            lastUpdatedClipData = null
        }
    }

    fun syncClipboard(isClient: Boolean) {
        Log.d(logTag, "syncClipboard: isClient: $isClient, isClientEnabled: $isClientEnabled, _isListening: $_isListening")
        if (isClient && !isClientEnabled) {
            return
        }
        if (!isClient && !_isListening) {
            return
        }
        checkPrimaryClip(isClient, true)
    }

    @Keep
    fun rustUpdateClipboard(clips: ByteArray) {
        val clips = MultiClipboards.parseFrom(clips)
        var mimeTypes = mutableListOf<String>()
        var text: String? = null
        var html: String? = null
        for (clip in clips.getClipboardsList()) {
            when (clip.format) {
                    ClipboardFormat.Text -> {
                        mimeTypes.add(ClipDescription.MIMETYPE_TEXT_PLAIN)
                    text = String(clip.content.toByteArray(), Charsets.UTF_8)
                }
                ClipboardFormat.Html -> {
                    mimeTypes.add(ClipDescription.MIMETYPE_TEXT_HTML)
                    html = String(clip.content.toByteArray(), Charsets.UTF_8)
                }
                ClipboardFormat.ImageRgba -> {
                }
                ClipboardFormat.ImagePng -> {
                }
                else -> {
                    Log.e(logTag, "Unsupported clipboard format: ${clip.format}")
                }
            }
        }

        val clipDescription = ClipDescription("clipboard", mimeTypes.toTypedArray())
        var item: ClipData.Item? = null
        if (text == null) {
            Log.e(logTag, "No text content in clipboard")
            return
        } else {
            if (html == null) {
                item = ClipData.Item(text)
            } else {
                item = ClipData.Item(text, html)
            }
        }
        if (item == null) {
            Log.e(logTag, "No item in clipboard")
            return
        }
        val clipData = ClipData(clipDescription, item)
        lastUpdatedClipData = clipData
        clipboardManager.setPrimaryClip(clipData)
    }
}
