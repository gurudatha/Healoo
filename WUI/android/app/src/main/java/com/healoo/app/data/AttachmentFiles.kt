package com.healoo.app.data

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.security.MessageDigest

/**
 * PdfRenderer needs a real file descriptor, so every PDF is resolved to a cached local file first.
 * Handles bundled samples (file:///android_asset/...), picked files (content://) and presigned URLs (https://).
 */
object AttachmentFiles {
    private val http by lazy { OkHttpClient() }

    suspend fun localFile(context: Context, uri: String, name: String): File = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, "attachments").apply { mkdirs() }
        // Presigned URLs change on every fetch; key the cache on the path without the query string.
        val key = sha1(uri.substringBefore('?'))
        val target = File(dir, "$key-${name.replace(Regex("[^A-Za-z0-9._-]"), "_")}")
        if (target.exists() && target.length() > 0) return@withContext target

        val tmp = File(dir, "${target.name}.part")
        when {
            uri.startsWith("file:///android_asset/") ->
                context.assets.open(uri.removePrefix("file:///android_asset/")).use { input ->
                    tmp.outputStream().use { input.copyTo(it) }
                }
            uri.startsWith("content://") || uri.startsWith("file://") ->
                context.contentResolver.openInputStream(Uri.parse(uri))!!.use { input ->
                    tmp.outputStream().use { input.copyTo(it) }
                }
            else -> http.newCall(Request.Builder().url(uri).build()).execute().use { resp ->
                check(resp.isSuccessful) { "Could not download $name (${resp.code})" }
                resp.body!!.byteStream().use { input -> tmp.outputStream().use { input.copyTo(it) } }
            }
        }
        tmp.renameTo(target)
        target
    }

    /** Name + size of a picked file, for validation against Limits before upload. */
    fun describe(context: Context, uri: Uri): Pair<String, Long> {
        var name = uri.lastPathSegment ?: "file"
        var size = 0L
        context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)
            ?.use { c ->
                if (c.moveToFirst()) {
                    c.getString(0)?.let { name = it }
                    if (!c.isNull(1)) size = c.getLong(1)
                }
            }
        return name to size
    }

    private fun sha1(s: String) = MessageDigest.getInstance("SHA-1").digest(s.toByteArray())
        .joinToString("") { "%02x".format(it) }.take(16)
}
