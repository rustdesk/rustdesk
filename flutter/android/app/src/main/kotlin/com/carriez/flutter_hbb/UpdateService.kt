package com.carriez.flutter_hbb

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.util.Log
import androidx.core.content.FileProvider
import kotlinx.coroutines.*
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

class UpdateService(private val context: Context) {
    companion object {
        private const val logTag = "UpdateService"
        private const val GITHUB_API_URL = "https://api.github.com/repos/rustdesk/rustdesk/releases/latest"
        private const val AUTHORITY = "com.carriez.flutter_hbb.fileprovider"
    }

    private var downloadJob: Job? = null

    /**
     * Проверяет наличие обновлений на GitHub
     * @param currentVersion текущая версия приложения (например "1.4.4")
     * @param onUpdateAvailable callback с информацией об обновлении
     * @param onError callback при ошибке
     */
    fun checkForUpdates(
        currentVersion: String,
        onUpdateAvailable: (UpdateInfo) -> Unit,
        onError: (String) -> Unit
    ) {
        CoroutineScope(Dispatchers.Default).launch {
            try {
                val latestRelease = fetchLatestRelease()
                val latestVersion = latestRelease.version
                
                Log.d(logTag, "Current version: $currentVersion, Latest version: $latestVersion")
                
                if (isNewVersionAvailable(currentVersion, latestVersion)) {
                    val apkUrl = findApkUrl(latestRelease)
                    if (apkUrl != null) {
                        val updateInfo = UpdateInfo(
                            version = latestVersion,
                            downloadUrl = apkUrl,
                            releaseNotes = latestRelease.body,
                            fileName = "rustdesk-$latestVersion.apk"
                        )
                        withContext(Dispatchers.Main) {
                            onUpdateAvailable(updateInfo)
                        }
                    } else {
                        withContext(Dispatchers.Main) {
                            onError("APK not found in release")
                        }
                    }
                }
            } catch (e: Exception) {
                Log.e(logTag, "Error checking for updates: ${e.message}", e)
                withContext(Dispatchers.Main) {
                    onError(e.message ?: "Unknown error")
                }
            }
        }
    }

    /**
     * Скачивает APK и устанавливает его
     * @param updateInfo информация об обновлении
     * @param onProgress callback для отслеживания прогресса (0-100)
     * @param onSuccess callback при успешной загрузке и установке
     * @param onError callback при ошибке
     */
    fun downloadAndInstall(
        updateInfo: UpdateInfo,
        onProgress: (Int) -> Unit,
        onSuccess: () -> Unit,
        onError: (String) -> Unit
    ) {
        downloadJob = CoroutineScope(Dispatchers.Default).launch {
            try {
                val apkFile = downloadApk(updateInfo.downloadUrl, updateInfo.fileName) { progress ->
                    withContext(Dispatchers.Main) {
                        onProgress(progress)
                    }
                }
                
                withContext(Dispatchers.Main) {
                    installApk(apkFile)
                    onSuccess()
                }
            } catch (e: Exception) {
                Log.e(logTag, "Error downloading/installing update: ${e.message}", e)
                withContext(Dispatchers.Main) {
                    onError(e.message ?: "Unknown error")
                }
            }
        }
    }

    /**
     * Отменяет текущую загрузку
     */
    fun cancelDownload() {
        downloadJob?.cancel()
    }

    private fun fetchLatestRelease(): GitHubRelease {
        val url = URL(GITHUB_API_URL)
        val connection = url.openConnection() as HttpURLConnection
        connection.requestMethod = "GET"
        connection.setRequestProperty("Accept", "application/vnd.github.v3+json")
        
        try {
            val responseCode = connection.responseCode
            if (responseCode != HttpURLConnection.HTTP_OK) {
                throw Exception("GitHub API error: $responseCode")
            }

            val response = connection.inputStream.bufferedReader().use { it.readText() }
            return parseGitHubRelease(response)
        } finally {
            connection.disconnect()
        }
    }

    private fun parseGitHubRelease(json: String): GitHubRelease {
        // Simple JSON parsing (без зависимостей)
        val tagVersion = json.substringAfter("\"tag_name\":\"").substringBefore("\"")
        val body = json.substringAfter("\"body\":\"").substringBefore("\"")
        val assets = json.substringAfter("\"assets\":[").substringBefore("]")
        
        return GitHubRelease(
            version = tagVersion.removePrefix("v"),
            body = body.replace("\\n", "\n"),
            assets = assets
        )
    }

    private fun findApkUrl(release: GitHubRelease): String? {
        // Ищем APK для Android (может быть несколько вариантов)
        val patterns = listOf(
            "rustdesk.*arm64-v8a.*\\.apk",
            "rustdesk.*armeabi-v7a.*\\.apk",
            "rustdesk.*universal.*\\.apk",
            "rustdesk.*\\.apk"
        )

        for (pattern in patterns) {
            val match = Regex(pattern).find(release.assets)
            if (match != null) {
                val fileName = match.value
                return "https://github.com/rustdesk/rustdesk/releases/download/${release.version}/$fileName"
            }
        }
        return null
    }

    private fun downloadApk(
        url: String,
        fileName: String,
        onProgress: (Int) -> Unit
    ): File {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.requestMethod = "GET"
        connection.connect()

        try {
            val totalSize = connection.contentLength
            val file = File(context.getExternalFilesDir(null), fileName)

            file.outputStream().use { output ->
                connection.inputStream.use { input ->
                    val buffer = ByteArray(4096)
                    var downloaded = 0
                    var bytesRead: Int

                    while (input.read(buffer).also { bytesRead = it } != -1) {
                        output.write(buffer, 0, bytesRead)
                        downloaded += bytesRead
                        val progress = if (totalSize > 0) (downloaded * 100 / totalSize) else 0
                        onProgress(progress)
                    }
                }
            }

            return file
        } finally {
            connection.disconnect()
        }
    }

    private fun installApk(apkFile: File) {
        val uri = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            FileProvider.getUriForFile(context, AUTHORITY, apkFile)
        } else {
            Uri.fromFile(apkFile)
        }

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
        }

        try {
            context.startActivity(intent)
        } catch (e: Exception) {
            Log.e(logTag, "Error installing APK: ${e.message}", e)
            throw e
        }
    }

    private fun isNewVersionAvailable(current: String, latest: String): Boolean {
        return try {
            val currentParts = current.split(".").map { it.toIntOrNull() ?: 0 }
            val latestParts = latest.split(".").map { it.toIntOrNull() ?: 0 }

            for (i in 0 until maxOf(currentParts.size, latestParts.size)) {
                val curr = currentParts.getOrNull(i) ?: 0
                val lat = latestParts.getOrNull(i) ?: 0
                when {
                    lat > curr -> return true
                    lat < curr -> return false
                }
            }
            false
        } catch (e: Exception) {
            Log.e(logTag, "Error comparing versions", e)
            false
        }
    }

    data class GitHubRelease(
        val version: String,
        val body: String,
        val assets: String
    )

    data class UpdateInfo(
        val version: String,
        val downloadUrl: String,
        val releaseNotes: String,
        val fileName: String
    )
}
