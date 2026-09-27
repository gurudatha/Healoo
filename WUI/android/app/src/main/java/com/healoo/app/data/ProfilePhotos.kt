package com.healoo.app.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/** Profile pictures are resized on the phone before upload: at most [MAX_PX] on the long side, JPEG. */
object ProfilePhotos {
    const val MAX_PX = 512

    /**
     * Decodes a picked or captured image (ImageDecoder also applies the camera's EXIF rotation),
     * scales it down and writes a JPEG into the cache. Returns it ready for upload.
     */
    suspend fun prepare(context: Context, source: Uri): PendingAttachment = withContext(Dispatchers.IO) {
        val bitmap = ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.contentResolver, source)) { decoder, info, _ ->
            val (w, h) = info.size.width to info.size.height
            val scale = minOf(1f, MAX_PX.toFloat() / maxOf(w, h))
            decoder.setTargetSize((w * scale).toInt().coerceAtLeast(1), (h * scale).toInt().coerceAtLeast(1))
            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE   // needed to compress it below
        }
        val file = File(context.cacheDir, "profile/photo-${System.currentTimeMillis()}.jpg").apply { parentFile?.mkdirs() }
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.JPEG, 85, it) }
        bitmap.recycle()
        // Older prepared photos are no longer needed.
        file.parentFile?.listFiles()?.filter { it != file }?.forEach { it.delete() }
        PendingAttachment(Uri.fromFile(file), AttachmentKind.IMAGE, "profile.jpg", "image/jpeg", file.length())
    }
}
