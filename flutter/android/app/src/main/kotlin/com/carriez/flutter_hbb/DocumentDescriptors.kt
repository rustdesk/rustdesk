package com.carriez.flutter_hbb

import android.os.ParcelFileDescriptor
import org.json.JSONObject
import java.io.IOException
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap

internal class DocumentDescriptors {
    private val descriptors = ConcurrentHashMap<String, ParcelFileDescriptor>()

    fun open(descriptor: ParcelFileDescriptor): JSONObject {
        val id = UUID.randomUUID().toString()
        try {
            // Only detach the duplicate: the original carries the provider's close/error channel.
            val duplicate = ParcelFileDescriptor.dup(descriptor.fileDescriptor)
            descriptors[id] = descriptor
            return JSONObject().put("fd", duplicate.detachFd()).put("descriptor", id)
        } catch (error: Exception) {
            descriptors.remove(id)
            try {
                descriptor.closeWithError(error.message ?: "Unable to open document")
            } catch (closeError: Exception) {
                error.addSuppressed(closeError)
            }
            throw error
        }
    }

    fun close(request: JSONObject): Boolean {
        val descriptor = descriptors.remove(request.getString("descriptor"))
            ?: throw IOException("Document descriptor was already closed")
        val error = request.optString("error", "")
        if (error.isNotEmpty()) {
            descriptor.closeWithError(error)
        } else {
            descriptor.use { if (it.canDetectErrors()) it.checkError() }
        }
        return true
    }
}
