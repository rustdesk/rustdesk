package com.carriez.flutter_hbb

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import android.util.Log
import java.io.FileNotFoundException
import java.io.IOException
import java.util.UUID
import java.util.concurrent.TimeUnit

internal class DocumentLogs(private val context: Context) {
    private val resolver = context.contentResolver
    private val prefs = context.getSharedPreferences(DOCUMENT_PREFS, Context.MODE_PRIVATE)
    private var output: ParcelFileDescriptor.AutoCloseOutputStream? = null
    private var written = 0L
    private var openedDay = 0L
    private var failure: Exception? = null

    private fun grant(): Uri? = prefs.getString(LOG_GRANT, null)?.let(Uri::parse)

    fun hasPermission(): Boolean {
        val uri = grant() ?: return false
        return resolver.persistedUriPermissions.any {
            it.uri == uri && it.isReadPermission && it.isWritePermission
        }
    }

    private fun directory(grant: Uri): Uri =
        DocumentsContract.buildDocumentUriUsingTree(grant, DIRECTORY_ID)

    private fun isDirectory(uri: Uri): Boolean = try {
        resolver.query(uri, arrayOf(Document.COLUMN_MIME_TYPE), null, null, null)?.use {
            it.moveToFirst() && it.getString(it.getColumnIndexOrThrow(
                Document.COLUMN_MIME_TYPE)) == Document.MIME_TYPE_DIR
        } ?: false
    } catch (_: FileNotFoundException) {
        false
    }

    private fun containsLogDirectory(grant: Uri): Boolean {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(grant, PARENT_ID)
        val column = Document.COLUMN_DOCUMENT_ID
        return resolver.query(children, arrayOf(column), null, null, null)?.use { cursor ->
            cursor.extras.getString(DocumentsContract.EXTRA_ERROR)?.let {
                throw IOException("Unable to list Android/media: $it")
            }
            if (cursor.extras.getBoolean(DocumentsContract.EXTRA_LOADING)) {
                throw IOException("Android/media is still loading")
            }
            while (cursor.moveToNext()) {
                if (cursor.getString(cursor.getColumnIndexOrThrow(column)) == DIRECTORY_ID) {
                    return@use true
                }
            }
            false
        } ?: throw IOException("Unable to list Android/media")
    }

    @Synchronized
    fun select(uri: Uri, flags: Int) {
        require(uri.authority == AUTHORITY && DocumentsContract.isTreeUri(uri)) {
            "Select Android/media or Android/media/RustDesk for logs"
        }
        val id = DocumentsContract.getTreeDocumentId(uri)
        require(id == PARENT_ID || id == DIRECTORY_ID) {
            "Select Android/media or Android/media/RustDesk for logs"
        }
        val permissions = flags and GRANT_FLAGS
        require(permissions == GRANT_FLAGS) { "Read and write access is required for logs" }
        resolver.takePersistableUriPermission(uri, permissions)
        if (id == PARENT_ID && !containsLogDirectory(uri)) {
            val parent = DocumentsContract.buildDocumentUriUsingTree(uri, PARENT_ID)
            val created = DocumentsContract.createDocument(
                resolver, parent, Document.MIME_TYPE_DIR, DIRECTORY_NAME)
                ?: throw IOException("Unable to create Android/media/RustDesk")
            check(DocumentsContract.getDocumentId(created) == DIRECTORY_ID) {
                "Android/media/RustDesk is not available as a directory"
            }
        }
        check(isDirectory(directory(uri))) { "The log directory is unavailable" }
        closeOutput()
        check(prefs.edit().putString(LOG_GRANT, uri.toString()).commit()) {
            "Unable to remember the log directory"
        }
        check(ensureReady()) { "Log directory access was revoked" }
    }

    @Synchronized
    fun ensureReady(): Boolean {
        if (!hasPermission()) return false
        val uri = grant() ?: return false
        if (DocumentsContract.getTreeDocumentId(uri) == PARENT_ID &&
            !containsLogDirectory(uri)) return false
        if (!isDirectory(directory(uri))) return false
        failure = null
        if (output == null) openNext(uri)
        return true
    }

    @Synchronized
    fun write(message: String): Boolean {
        val uri = grant() ?: return false
        failure?.let { throw IOException("File logging stopped: ${it.message}", it) }
        try {
            if (output == null && !hasPermission()) {
                throw IOException("Log directory access was revoked")
            }
            if (output == null || written >= MAX_LOG_BYTES || openedDay != currentDay()) {
                openNext(uri)
            }
            val bytes = message.toByteArray(Charsets.UTF_8)
            val stream = output ?: throw IOException("The log file is not open")
            stream.write(bytes)
            written += bytes.size
            return true
        } catch (error: Exception) {
            failure = error
            try {
                closeOutput()
            } catch (closeError: Exception) {
                error.addSuppressed(closeError)
            }
            Log.e("DocumentLogs", "File logging stopped", error)
            throw error
        }
    }

    private fun closeOutput() {
        val stream = output
        output = null
        stream?.close()
    }

    private fun openNext(grant: Uri) {
        closeOutput()
        val name = "rustdesk-${System.currentTimeMillis()}-${UUID.randomUUID()}.txt"
        val document = DocumentsContract.createDocument(
            resolver, directory(grant), "text/plain", name)
            ?: throw IOException("Unable to create a log file")
        try {
            val descriptor = resolver.openFileDescriptor(document, "w")
                ?: throw IOException("Unable to open the log file")
            output = ParcelFileDescriptor.AutoCloseOutputStream(descriptor)
        } catch (error: Exception) {
            try {
                if (!DocumentsContract.deleteDocument(resolver, document)) {
                    throw IOException("Unable to remove the unopened log file")
                }
            } catch (cleanupError: Exception) {
                error.addSuppressed(cleanupError)
            }
            throw error
        }
        written = 0L
        openedDay = currentDay()
        val logs = files(grant).sortedBy { it.second }
        for ((uri, _) in logs.filter { it.first != document }
            .take((logs.size - MAX_LOG_FILES).coerceAtLeast(0))) {
            if (!DocumentsContract.deleteDocument(resolver, uri)) {
                throw IOException("Unable to remove an expired log file")
            }
        }
    }

    private fun files(grant: Uri): List<Pair<Uri, String>> {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(grant, DIRECTORY_ID)
        val columns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_MIME_TYPE)
        return resolver.query(children, columns, null, null, null)?.use { cursor ->
            cursor.extras.getString(DocumentsContract.EXTRA_ERROR)?.let {
                throw IOException("Unable to list logs: $it")
            }
            if (cursor.extras.getBoolean(DocumentsContract.EXTRA_LOADING)) {
                throw IOException("The log directory is still loading")
            }
            val entries = mutableListOf<Pair<Uri, String>>()
            while (cursor.moveToNext()) {
                if (cursor.getString(cursor.getColumnIndexOrThrow(Document.COLUMN_MIME_TYPE)) ==
                    Document.MIME_TYPE_DIR) continue
                val name = cursor.getString(cursor.getColumnIndexOrThrow(Document.COLUMN_DISPLAY_NAME))
                if (!LOG_NAME.matches(name)) continue
                val id = cursor.getString(cursor.getColumnIndexOrThrow(Document.COLUMN_DOCUMENT_ID))
                entries.add(DocumentsContract.buildDocumentUriUsingTree(grant, id) to name)
            }
            entries
        } ?: throw IOException("Unable to list logs")
    }

    fun exportTo(destination: Uri): Int {
        val uri = grant() ?: return 0
        check(hasPermission()) { "Log directory access was revoked" }
        val parent = DocumentsContract.buildDocumentUriUsingTree(
            destination, DocumentsContract.getTreeDocumentId(destination))
        val logs = files(uri)
        for ((source, name) in logs) {
            val target = DocumentsContract.createDocument(resolver, parent, "text/plain", name)
                ?: throw IOException("Unable to create an exported log")
            val input = resolver.openInputStream(source)
                ?: throw IOException("Unable to read log $name")
            input.use {
                val output = resolver.openOutputStream(target, "w")
                    ?: throw IOException("Unable to write log $name")
                output.use { destinationStream -> it.copyTo(destinationStream) }
            }
        }
        return logs.size
    }

    private fun currentDay(): Long = System.currentTimeMillis() / TimeUnit.DAYS.toMillis(1)

    companion object {
        const val AUTHORITY = "com.android.externalstorage.documents"
        const val PARENT_ID = "primary:Android/media"
        private const val DIRECTORY_NAME = "RustDesk"
        private const val DIRECTORY_ID = "$PARENT_ID/$DIRECTORY_NAME"
        private const val LOG_GRANT = "log-folder"
        private const val GRANT_FLAGS = Intent.FLAG_GRANT_READ_URI_PERMISSION or
            Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        private const val MAX_LOG_BYTES = 16L * 1024 * 1024
        private const val MAX_LOG_FILES = 90
        private val LOG_NAME = Regex("rustdesk-[0-9]+-[0-9a-f-]{36}\\.txt")
    }
}
