package com.carriez.flutter_hbb

import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Base64
import org.json.JSONObject
import java.io.IOException

private const val TREE_ROOT = "/@rustdesk/tree/"
internal const val DOCUMENT_PREFS = "document-storage"
internal const val DEFAULT_LOCATION = "default-location"
private const val URI_FLAGS = Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING
private const val MIN_LOCATION_COMPONENTS = 2

internal fun documentRoot(uri: Uri): String =
    "$TREE_ROOT${Base64.encodeToString(uri.toString().toByteArray(Charsets.UTF_8), URI_FLAGS)}"

internal fun checkDocumentName(name: String): String {
    require(name.isNotEmpty() && name != "." && name != ".." &&
        !name.contains('/') && !name.contains('\\') && !name.contains('\u0000')) {
        "Invalid document name"
    }
    return name
}

internal fun checkDocumentListing(cursor: Cursor, errorMessage: String, loadingMessage: String) {
    val extras = cursor.extras
    extras.getString(DocumentsContract.EXTRA_ERROR)?.let {
        throw IOException("$errorMessage: $it")
    }
    if (extras.getBoolean(DocumentsContract.EXTRA_LOADING)) {
        throw IOException(loadingMessage)
    }
}

internal fun documentEntry(name: String, directory: Boolean, size: Long = 0): JSONObject =
    JSONObject().put("name", name).put("directory", directory)
        .put("size", size).put("modified", 0)

internal class DocumentStorage(private val context: Context) {
    private val prefs = context.getSharedPreferences(DOCUMENT_PREFS, Context.MODE_PRIVATE)
    private val descriptors = DocumentDescriptors()
    val logs = DocumentLogs(context)

    fun home(): String? = prefs.getString(DEFAULT_LOCATION, null)

    fun shareWorkspace(): String {
        check(prefs.edit().remove(DEFAULT_LOCATION).commit()) {
            "Unable to remember the shared folder"
        }
        return (context.getExternalFilesDir(null) ?: context.filesDir).path
    }

    fun select(uri: Uri, flags: Int, share: Boolean): JSONObject {
        require(uri.scheme == "content") { "Invalid document selection" }
        val grants = flags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or
            Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        require(grants != 0) { "The selected location has no access grant" }
        context.contentResolver.takePersistableUriPermission(uri, grants)
        val root = documentRoot(uri)
        val backend = DocumentTree(context, uri, descriptors)
        val name = checkDocumentName(backend.rootName())
        val path = "$root/$name"
        if (share) {
            check(prefs.edit().putString(DEFAULT_LOCATION, path).commit()) {
                "Unable to remember the shared folder"
            }
        }
        return JSONObject().put("path", path).put("name", name)
    }

    fun backend(path: String): DocumentTree {
        require(path.startsWith(TREE_ROOT)) { "Invalid document location" }
        val parts = path.removePrefix(TREE_ROOT).split('/')
        require(parts.size >= MIN_LOCATION_COMPONENTS) { "Invalid document location" }
        parts.forEach(::checkDocumentName)
        val uri = Uri.parse(String(Base64.decode(parts.first(), URI_FLAGS), Charsets.UTF_8))
        require(uri.scheme == "content") { "Invalid document URI" }
        val authorized =
            context.contentResolver.persistedUriPermissions.any { it.uri == uri && it.isReadPermission }
        check(authorized) {
            "Access to this location was revoked. Select it again on the Android device."
        }
        return DocumentTree(context, uri, descriptors)
    }

    fun request(request: String): String = try {
        JSONObject().put("value", execute(JSONObject(request)) ?: JSONObject.NULL).toString()
    } catch (error: Exception) {
        JSONObject().put("error", error.message ?: error.javaClass.simpleName).toString()
    }

    private fun execute(request: JSONObject): Any? {
        val operation = request.getString("op")
        if (operation == "home") return home()
        if (operation == "log_write") return logs.write(request.getString("message"))
        if (operation == "close") return descriptors.close(request)
        val path = request.getString("path")
        val backend = backend(path)
        return when (operation) {
            "allowed" -> true
            "list" -> backend.list(path)
            "stat" -> backend.stat(path)
            "open" -> backend.open(path)
            else -> mutate(request, backend)
        }
    }

    private fun mutate(request: JSONObject, backend: DocumentTree): Any {
        val path = request.getString("path")
        return when (val operation = request.getString("op")) {
            "begin" -> backend.begin(path, request.getBoolean("overwrite"))
            "finish" -> backend.finish(request.getJSONObject("token")).let { true }
            "discard" -> backend.discard(request.getJSONObject("token")).let { true }
            "delete" -> backend.delete(path).let { true }
            "mkdir" -> backend.mkdir(path).let { true }
            "rename" -> backend.rename(path, checkDocumentName(request.getString("name"))).let { true }
            else -> throw IOException("Unsupported document operation: $operation")
        }
    }
}
