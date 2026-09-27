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
 * Full images and PDFs shown in the viewer, as local files (PdfRenderer needs a real file descriptor).
 * Handles bundled samples (file:///android_asset/...), picked files (content://) and presigned URLs (https://).
 *
 * The cache keeps only the [MAX_FILES] most recently used files and at most [MAX_BYTES] together:
 * each download evicts the least recently used ones. Thumbnails are cached separately by Coil
 * (see HealooApplication), so the two stay within about 100 MB.
 */
object AttachmentFiles {
    const val MAX_FILES = 15
    const val MAX_BYTES = 90L * 1024 * 1024
    /** The newest files are never evicted, even over the byte limit (they may be on screen). */
    private const val KEEP_NEWEST = 3

    private val http by lazy { OkHttpClient() }

    suspend fun localFile(context: Context, uri: String, name: String): File = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, "attachments").apply { mkdirs() }
        // Presigned URLs change on every fetch; key the cache on the path without the query string.
        val key = sha1(uri.substringBefore('?'))
        val target = File(dir, "$key-${name.replace(Regex("[^A-Za-z0-9._-]"), "_")}")
        if (target.exists() && target.length() > 0) {
            target.setLastModified(System.currentTimeMillis())   // recently used: evicted last
            return@withContext target
        }

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
        target.setLastModified(System.currentTimeMillis())
        trim(dir)
        target
    }

    /** Deletes the least recently used files beyond [MAX_FILES] or [MAX_BYTES]. */
    @Synchronized
    fun trim(dir: File) {
        val files = dir.listFiles { f: File -> f.isFile && !f.name.endsWith(".part") }.orEmpty()
            .sortedByDescending { it.lastModified() }
        var bytes = 0L
        files.forEachIndexed { i, f ->
            bytes += f.length()
            if (i >= KEEP_NEWEST && (i >= MAX_FILES || bytes > MAX_BYTES)) f.delete()
        }
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

/**
 * Coil request for a thumbnail (or a card's image). Presigned URLs change on every fetch, so the
 * cache key is the URL without its query string; otherwise every visit would download it again.
 */
fun thumbnailRequest(context: Context, uri: String): coil.request.ImageRequest {
    val key = uri.substringBefore('?')
    return coil.request.ImageRequest.Builder(context).data(uri).diskCacheKey(key).memoryCacheKey(key).crossfade(true).build()
}
