package com.carriez.flutter_hbb

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import android.webkit.MimeTypeMap
import org.json.JSONArray
import org.json.JSONObject
import java.io.FileNotFoundException
import java.io.IOException
import java.util.UUID

internal class DocumentTree(
    context: Context,
    private val grant: Uri,
    private val descriptors: DocumentDescriptors
) {
    private val root = documentRoot(grant)
    private val resolver = context.contentResolver
    private val rootUri: Uri get() = DocumentsContract.buildDocumentUriUsingTree(
        grant, DocumentsContract.getTreeDocumentId(grant))
    private val projection = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_MIME_TYPE, Document.COLUMN_SIZE, Document.COLUMN_LAST_MODIFIED)

    fun rootName(): String = metadata(rootUri)?.getString("name")
        ?: throw FileNotFoundException("The selected document no longer exists")

    private fun metadata(uri: Uri): JSONObject? = resolver.query(uri, projection, null, null, null)?.use {
        if (!it.moveToFirst()) return@use null
        val directory = it.getString(MIME_COLUMN) == Document.MIME_TYPE_DIR
        val size = if (it.isNull(SIZE_COLUMN)) -1 else it.getLong(SIZE_COLUMN)
        documentEntry(it.getString(NAME_COLUMN), directory, size)
            .put("modified", it.getLong(MODIFIED_COLUMN) / MILLIS_PER_SECOND).put("uri", uri.toString())
    } ?: throw IOException("Unable to query the selected document")

    private fun children(uri: Uri): List<Pair<Uri, JSONObject>> {
        val query = DocumentsContract.buildChildDocumentsUriUsingTree(
            grant, DocumentsContract.getDocumentId(uri))
        return resolver.query(query, projection, null, null, null)?.use { cursor ->
            checkDocumentListing(cursor, "Unable to list the selected folder",
                "The selected folder is still loading. Try again.")
            val entries = mutableListOf<Pair<Uri, JSONObject>>()
            while (cursor.moveToNext()) {
                val child = DocumentsContract.buildDocumentUriUsingTree(grant, cursor.getString(ID_COLUMN))
                val name = checkDocumentName(cursor.getString(NAME_COLUMN))
                val entry = documentEntry(name, cursor.getString(MIME_COLUMN) == Document.MIME_TYPE_DIR,
                    if (cursor.isNull(SIZE_COLUMN)) -1 else cursor.getLong(SIZE_COLUMN))
                    .put("modified", cursor.getLong(MODIFIED_COLUMN) / MILLIS_PER_SECOND)
                    .put("uri", child.toString())
                entries.add(child to entry)
            }
            entries
        } ?: throw IOException("Unable to list the selected folder")
    }

    private fun relative(path: String): List<String> {
        val parts = path.removePrefix("$root/").split('/')
        require(path.startsWith("$root/") && parts.first() == rootName()) {
            "The selected document was moved or renamed. Select it again."
        }
        parts.forEach(::checkDocumentName)
        return parts.drop(1)
    }

    private fun resolve(path: String, createParents: Boolean = false): Uri? {
        var current = rootUri
        for (name in relative(path)) {
            val matches = children(current).filter { it.second.getString("name") == name }
            check(matches.size <= 1) { "The provider returned multiple documents named $name" }
            val child = matches.singleOrNull()
            current = child?.first ?: if (createParents) {
                val created = DocumentsContract.createDocument(resolver, current, Document.MIME_TYPE_DIR, name)
                    ?: throw IOException("Unable to create folder $name")
                check(metadata(created)?.getString("name") == name) {
                    "The provider cannot preserve folder name: $name"
                }
                created
            } else return null
        }
        return current
    }

    fun uri(path: String): Uri = resolve(path)
        ?: throw FileNotFoundException("Document does not exist: $path")

    fun list(path: String): JSONArray = JSONArray(children(uri(path)).map { it.second })

    fun stat(path: String): JSONObject? = resolve(path)?.let(::metadata)

    fun open(path: String): JSONObject = descriptors.open(
        resolver.openFileDescriptor(uri(path), "r") ?: throw IOException("Unable to open document"))

    fun begin(path: String, overwrite: Boolean): JSONObject {
        val original = resolve(path)
        check(original == null || overwrite) { "Document overwrite requires confirmation" }
        require(original == null || metadata(original)?.getBoolean("directory") == false) {
            "Cannot replace a folder with a file"
        }
        val parent = resolve(path.substringBeforeLast('/'), true)
            ?: throw FileNotFoundException("Destination folder no longer exists")
        val name = checkDocumentName(path.substringAfterLast('/'))
        val temporary = DocumentsContract.createDocument(resolver, parent, mime(name),
            temporaryName(name, "download"))
            ?: throw IOException("Unable to create the receiving file")
        try {
            val descriptor = resolver.openFileDescriptor(temporary, "rw")
                ?: throw IOException("Unable to open the receiving file")
            return descriptors.open(descriptor).put("uri", temporary.toString())
                .put("path", path).put("original", original?.toString() ?: JSONObject.NULL)
        } catch (error: Exception) {
            try {
                check(DocumentsContract.deleteDocument(resolver, temporary)) {
                    "Unable to remove the receiving file"
                }
            } catch (cleanup: Exception) {
                error.addSuppressed(cleanup)
                throw IOException("${error.message ?: error.javaClass.simpleName}; " +
                    "temporary file cleanup failed: ${cleanup.message}", error)
            }
            throw error
        }
    }

    fun finish(token: JSONObject) {
        val path = token.getString("path")
        val name = checkDocumentName(path.substringAfterLast('/'))
        val original = resolve(path)
        val expected = if (token.isNull("original")) null else token.getString("original")
        check(original?.toString() == expected) {
            "The destination changed during the transfer; the received data was retained"
        }
        val backup = original?.let { renameChecked(it, temporaryName(name, "backup")) }
        try {
            renameChecked(Uri.parse(token.getString("uri")), name)
        } catch (error: Exception) {
            if (backup != null) {
                try { renameChecked(backup, name) } catch (restore: Exception) {
                    error.addSuppressed(restore)
                    throw IOException("Publishing failed and the original remains in $backup: ${restore.message}", error)
                }
            }
            throw error
        }
        if (backup != null && !DocumentsContract.deleteDocument(resolver, backup)) {
            throw IOException("File saved, but the previous copy could not be removed: $backup")
        }
    }

    private fun renameChecked(uri: Uri, name: String): Uri {
        val renamed = DocumentsContract.renameDocument(resolver, uri, name)
            ?: throw IOException("Unable to publish the received file; the partial file was retained")
        check(metadata(renamed)?.getString("name") == name) { "The provider changed the saved filename" }
        return renamed
    }

    fun discard(token: JSONObject) {
        check(DocumentsContract.deleteDocument(resolver, Uri.parse(token.getString("uri")))) {
            "Unable to remove the canceled transfer"
        }
    }

    fun delete(path: String) {
        require(relative(path).isNotEmpty()) { "Cannot delete a shared root" }
        check(DocumentsContract.deleteDocument(resolver, uri(path))) { "Unable to delete document" }
    }

    fun mkdir(path: String) {
        val created = resolve(path, true) ?: throw IOException("Unable to create folder")
        check(metadata(created)?.getBoolean("directory") == true) { "The destination is not a folder" }
    }

    fun rename(path: String, name: String) {
        require(relative(path).isNotEmpty()) { "Cannot rename a shared root" }
        if (resolve(path.substringBeforeLast('/') + "/" + name) != null) throw IOException(EXISTS_MESSAGE)
        renameChecked(uri(path), name)
    }

    companion object {
        private const val ID_COLUMN = 0
        private const val NAME_COLUMN = 1
        private const val MIME_COLUMN = 2
        private const val SIZE_COLUMN = 3
        private const val MODIFIED_COLUMN = 4
        private const val MILLIS_PER_SECOND = 1000L
        const val EXISTS_MESSAGE = "A file already exists at the destination. Rename the file or select another folder."
        fun temporaryName(name: String, purpose: String): String {
            // Providers can append the MIME extension when renaming; preserve it for temporary files.
            val extension = name.substringAfterLast('.', "").let { if (it.isEmpty()) "" else ".$it" }
            return ".rustdesk-${UUID.randomUUID()}.$purpose$extension"
        }

        fun mime(name: String): String = MimeTypeMap.getSingleton()
            .getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase())
            ?: "application/octet-stream"
    }
}
