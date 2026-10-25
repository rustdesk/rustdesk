package com.carriez.flutter_hbb

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Environment
import android.os.Process
import android.provider.DocumentsContract
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.util.UUID
import java.util.concurrent.TimeUnit

internal class LegacyDocumentLogs(private val context: Context) {
    private val prefs = context.getSharedPreferences(DOCUMENT_PREFS, Context.MODE_PRIVATE)
    private val directory by lazy {
        val name = checkDocumentName(context.applicationInfo.loadLabel(context.packageManager).toString())
        File(Environment.getExternalStorageDirectory(), "Android/media/$name/Logs")
    }
    private var output: FileOutputStream? = null
    private var written = 0L
    private var openedDay = 0L
    private var failure: Exception? = null

    fun hasPermission(): Boolean = context.checkPermission(
        Manifest.permission.WRITE_EXTERNAL_STORAGE, Process.myPid(), Process.myUid()
    ) == PackageManager.PERMISSION_GRANTED

    private fun configured(): Boolean =
        prefs.getString(DocumentLogs.LOG_GRANT, null) == Uri.fromFile(directory).toString()

    fun select() {
        check(hasPermission()) { "Storage permission is required for public logs" }
        openNext()
        check(prefs.edit().putString(DocumentLogs.LOG_GRANT, Uri.fromFile(directory).toString()).commit()) {
            "Unable to remember the log directory"
        }
        failure = null
    }

    fun ensureReady(): Boolean {
        if (!configured() || !hasPermission()) return false
        failure = null
        if (output == null || !directory.isDirectory) openNext()
        return true
    }

    fun write(message: String): Boolean {
        if (!configured() || !hasPermission()) {
            closeOutput()
            return false
        }
        failure?.let { throw IOException("File logging stopped: ${it.message}", it) }
        try {
            if (output == null || written >= DocumentLogs.MAX_LOG_BYTES || openedDay != currentDay()) {
                openNext()
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
            throw error
        }
    }

    private fun closeOutput() {
        val stream = output
        output = null
        stream?.close()
    }

    private fun openNext() {
        closeOutput()
        if (!directory.isDirectory && !directory.mkdirs()) {
            throw IOException("Unable to create $directory")
        }
        val file = File(directory, "rustdesk-${System.currentTimeMillis()}-${UUID.randomUUID()}.txt")
        output = FileOutputStream(file)
        written = 0L
        openedDay = currentDay()
        val logs = files().sortedBy { it.name }
        for (expired in logs.filter { it != file }
            .take((logs.size - DocumentLogs.MAX_LOG_FILES).coerceAtLeast(0))) {
            if (!expired.delete()) throw IOException("Unable to remove an expired log: $expired")
        }
    }

    private fun files(): List<File> = (directory.listFiles()
        ?: throw IOException("Unable to list logs in $directory"))
        .filter { it.isFile && DocumentLogs.LOG_NAME.matches(it.name) }

    fun exportTo(destination: Uri): Int {
        if (!configured() || !hasPermission() || !directory.exists()) return 0
        val resolver = context.contentResolver
        val parent = DocumentsContract.buildDocumentUriUsingTree(
            destination, DocumentsContract.getTreeDocumentId(destination))
        val logs = files()
        for (source in logs) {
            val target = DocumentsContract.createDocument(resolver, parent, "text/plain", source.name)
                ?: throw IOException("Unable to create an exported log")
            source.inputStream().use { input ->
                val output = resolver.openOutputStream(target, "w")
                    ?: throw IOException("Unable to write log ${source.name}")
                output.use { input.copyTo(it) }
            }
        }
        return logs.size
    }

    private fun currentDay(): Long = System.currentTimeMillis() / TimeUnit.DAYS.toMillis(1)
}
