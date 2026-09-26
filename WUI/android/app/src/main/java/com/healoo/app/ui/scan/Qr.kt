package com.healoo.app.ui.scan

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.unit.Dp
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import kotlinx.coroutines.tasks.await

/** QR content for a Healoo ID. Scanners also accept a bare ID such as HL-2M9P4. */
fun healooQrContent(publicId: String) = "healoo://id/$publicId"

fun parseHealooId(raw: String?): String? = raw?.uppercase()?.let { Regex("HL-[A-Z0-9]{5}").find(it)?.value }

sealed interface ScanResult {
    data class Found(val publicId: String) : ScanResult
    data object Cancelled : ScanResult
    data object NotHealoo : ScanResult
    data class Failed(val reason: String) : ScanResult
}

/**
 * Opens Google's code scanner UI (Play services). It needs no camera permission in this app
 * because the camera runs inside Play services.
 */
suspend fun scanHealooId(context: Context): ScanResult {
    val options = GmsBarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).enableAutoZoom().build()
    return try {
        val code = GmsBarcodeScanning.getClient(context, options).startScan().await()
        parseHealooId(code.rawValue)?.let { ScanResult.Found(it) } ?: ScanResult.NotHealoo
    } catch (e: kotlinx.coroutines.CancellationException) {
        ScanResult.Cancelled
    } catch (e: Exception) {
        ScanResult.Failed(e.message ?: "Scanner unavailable")
    }
}

private fun qrBitmap(content: String, sizePx: Int): Bitmap {
    val matrix = QRCodeWriter().encode(content, BarcodeFormat.QR_CODE, sizePx, sizePx, mapOf(EncodeHintType.MARGIN to 1))
    val pixels = IntArray(sizePx * sizePx) { i -> if (matrix[i % sizePx, i / sizePx]) 0xFF1F2A27.toInt() else 0xFFFFFFFF.toInt() }
    return Bitmap.createBitmap(pixels, sizePx, sizePx, Bitmap.Config.ARGB_8888)
}

@Composable
fun QrImage(content: String, size: Dp, description: String) {
    val bmp = remember(content) { qrBitmap(content, 512) }
    Image(bmp.asImageBitmap(), contentDescription = description, filterQuality = FilterQuality.None, modifier = Modifier.size(size))
}
