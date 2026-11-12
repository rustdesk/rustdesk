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
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

internal class DocumentLogs(private val context: Context) {
    private val resolver = context.contentResolver
    private val prefs = context.getSharedPreferences(DOCUMENT_PREFS, Context.MODE_PRIVATE)
    private val appName = context.applicationInfo.loadLabel(context.packageManager).toString()
    private val appDirectoryId = "$PARENT_ID/$appName"
    private val directoryId = "$appDirectoryId/Logs"
    val directoryPath = directoryId.substringAfter(':')
    private var output: ParcelFileDescriptor.AutoCloseOutputStream? = null
    private var written = 0L
    private var openedDay = 0L
    private var failure: Exception? = null
    private val lock = ReentrantLock()

    init {
        setEnabled(grant() != null)
    }

    @androidx.annotation.Keep
    private external fun setEnabled(enabled: Boolean)

    private fun grant(): Uri? = prefs.getString(LOG_GRANT, null)?.let(Uri::parse)

    fun hasPermission(): Boolean {
        val uri = grant() ?: return false
        if (!isLogGrant(uri)) return false
        return resolver.persistedUriPermissions.any {
            it.uri == uri && it.isReadPermission && it.isWritePermission
        }
    }

    private fun isLogGrant(uri: Uri): Boolean =
        uri.authority == AUTHORITY && DocumentsContract.isTreeUri(uri) &&
            DocumentsContract.getTreeDocumentId(uri) in listOf(PARENT_ID, appDirectoryId, directoryId)

    private fun isDirectory(uri: Uri): Boolean = try {
        resolver.query(uri, arrayOf(Document.COLUMN_MIME_TYPE), null, null, null)?.use {
            it.moveToFirst() && it.getString(it.getColumnIndexOrThrow(
                Document.COLUMN_MIME_TYPE)) == Document.MIME_TYPE_DIR
        } ?: false
    } catch (_: FileNotFoundException) {
        false
    }

    private fun containsDirectory(grant: Uri, parentId: String, id: String): Boolean {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(grant, parentId)
        val columns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE)
        return resolver.query(children, columns, null, null, null)?.use { cursor ->
            checkDocumentListing(cursor, "Unable to list ${parentId.substringAfter(':')}",
                "${parentId.substringAfter(':')} is still loading")
            while (cursor.moveToNext()) {
                if (cursor.getString(cursor.getColumnIndexOrThrow(Document.COLUMN_DOCUMENT_ID)) != id) continue
                check(cursor.getString(cursor.getColumnIndexOrThrow(Document.COLUMN_MIME_TYPE)) ==
                    Document.MIME_TYPE_DIR) { "${id.substringAfter(':')} is not a directory" }
                return@use true
            }
            false
        } ?: throw IOException("Unable to list ${parentId.substringAfter(':')}")
    }

    private fun ensureDirectory(grant: Uri): Uri {
        checkDocumentName(appName)
        val rootId = DocumentsContract.getTreeDocumentId(grant)
        var parent = DocumentsContract.buildDocumentUriUsingTree(grant, rootId)
        check(isDirectory(parent)) { "The authorized log folder is unavailable" }
        val children = when (rootId) {
            PARENT_ID -> listOf(appDirectoryId, directoryId)
            appDirectoryId -> listOf(directoryId)
            directoryId -> emptyList()
            else -> throw IOException("The selected folder does not contain $directoryPath")
        }
        for (id in children) {
            val child = DocumentsContract.buildDocumentUriUsingTree(grant, id)
            if (!containsDirectory(grant, DocumentsContract.getDocumentId(parent), id)) {
                closeOutput()
                val created = DocumentsContract.createDocument(
                    resolver, parent, Document.MIME_TYPE_DIR, id.substringAfterLast('/'))
                    ?: throw IOException("Unable to create ${id.substringAfter(':')}")
                check(DocumentsContract.getDocumentId(created) == id) {
                    "${id.substringAfter(':')} is not available as a directory"
                }
            }
            parent = child
        }
        return parent
    }

    fun select(uri: Uri, flags: Int) = lock.withLock {
        require(isLogGrant(uri)) {
            "Please select Android → media."
        }
        val permissions = flags and GRANT_FLAGS
        require(permissions == GRANT_FLAGS) { "Read and write access is required for logs" }
        setEnabled(false)
        resolver.takePersistableUriPermission(uri, permissions)
        ensureDirectory(uri)
        closeOutput()
        check(prefs.edit().putString(LOG_GRANT, uri.toString()).commit()) {
            "Unable to remember the log directory"
        }
        check(ensureReady()) { "Log directory access was revoked" }
    }

    fun ensureReady(): Boolean = lock.withLock {
        setEnabled(false)
        if (!hasPermission()) return false
        val uri = grant() ?: return false
        val root = DocumentsContract.buildDocumentUriUsingTree(uri, DocumentsContract.getTreeDocumentId(uri))
        if (!isDirectory(root)) return false
        ensureDirectory(uri)
        failure = null
        if (output == null) openNext(uri)
        setEnabled(true)
        return true
    }

    fun write(message: String): Boolean {
        if (!lock.tryLock()) return false
        return try {
            writeLocked(message)
        } finally {
            lock.unlock()
        }
    }

    private fun writeLocked(message: String): Boolean {
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
            setEnabled(false)
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
            resolver, ensureDirectory(grant), "text/plain", name)
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
            try {
                if (!DocumentsContract.deleteDocument(resolver, uri)) {
                    throw IOException("Unable to remove an expired log file")
                }
            } catch (error: Exception) {
                Log.w("DocumentLogs", "Unable to remove an expired log file", error)
            }
        }
    }

    private fun files(grant: Uri): List<Pair<Uri, String>> {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(grant, directoryId)
        val columns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_MIME_TYPE)
        return resolver.query(children, columns, null, null, null)?.use { cursor ->
            checkDocumentListing(cursor, "Unable to list logs", "The log directory is still loading")
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
        private const val LOG_GRANT = "log-folder"
        private const val GRANT_FLAGS = Intent.FLAG_GRANT_READ_URI_PERMISSION or
            Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        private const val MAX_LOG_BYTES = 16L * 1024 * 1024
        private const val MAX_LOG_FILES = 90
        private val LOG_NAME = Regex("rustdesk-[0-9]+-[0-9a-f-]{36}\\.txt")
    }
}
