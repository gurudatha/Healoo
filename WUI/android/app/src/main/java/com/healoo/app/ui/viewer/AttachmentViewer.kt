package com.healoo.app.ui.viewer

import com.healoo.app.ui.components.ScrollbarLazyColumn
import android.graphics.Bitmap
import android.graphics.pdf.PdfRenderer
import android.os.ParcelFileDescriptor
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import com.healoo.app.ui.icons.outlined.PictureAsPdf
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import coil.compose.AsyncImage
import com.healoo.app.data.*
import com.healoo.app.ui.components.LoadingBox
import com.healoo.app.ui.theme.HType
import com.healoo.app.ui.theme.Sage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File

class ViewerViewModel(private val itemId: String) : ViewModel() {
    var attachments by mutableStateOf<List<Attachment>?>(null); private set
    var title by mutableStateOf(""); private set

    init {
        viewModelScope.launch {
            runCatching { ServiceLocator.repository.item(itemId) }.onSuccess {
                title = it.title
                attachments = it.attachments.sortedBy { a -> a.position }
            }.onFailure { attachments = emptyList() }
        }
    }
}

/**
 * Design doc 3.6 — opens on [startIndex]; swiping left/right moves through every image and PDF of the
 * item without closing. PDF pages scroll vertically inside their page of the pager.
 */
@Composable
fun AttachmentViewer(itemId: String, startIndex: Int, onClose: () -> Unit) {
    val vm: ViewerViewModel = viewModel(key = "viewer-$itemId") { ViewerViewModel(itemId) }
    val files = vm.attachments

    Box(Modifier.fillMaxSize().background(Sage.ViewerBackground)) {
        if (files == null) { LoadingBox(Modifier.align(Alignment.Center)); return@Box }
        if (files.isEmpty()) { Text("This item has no attachments.", color = Color.White, modifier = Modifier.align(Alignment.Center)); return@Box }

        val pager = rememberPagerState(initialPage = startIndex.coerceIn(0, files.lastIndex)) { files.size }
        var zoomed by remember { mutableStateOf(false) }
        val pdfPage = remember { mutableStateMapOf<Int, Pair<Int, Int>>() }     // attachment index -> (page, total)
        val scope = rememberCoroutineScope()

        Column(Modifier.fillMaxSize()) {
            // Top bar: close · file name · "2 / 5"
            Row(
                Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onClose) { Icon(Icons.Outlined.Close, "Close viewer", tint = Color.White) }
                Column(Modifier.weight(1f)) {
                    val current = files[pager.currentPage]
                    Text(current.name, style = HType.bodyStrong, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    val sub = pdfPage[pager.currentPage]?.let { (p, t) -> "Page $p of $t" }
                        ?: if (current.kind == AttachmentKind.IMAGE) "Image" else "PDF"
                    Text(sub, style = HType.small, color = Sage.OnPrimaryLine)
                }
                Text("${pager.currentPage + 1} / ${files.size}", style = HType.bodyStrong, color = Color.White,
                    modifier = Modifier.padding(end = 12.dp).semantics { contentDescription = "Attachment ${pager.currentPage + 1} of ${files.size}" })
            }

            HorizontalPager(
                state = pager,
                userScrollEnabled = !zoomed,          // while an image is zoomed, drags pan the image instead
                beyondViewportPageCount = 1,          // preload one attachment on each side
                modifier = Modifier.weight(1f).fillMaxWidth(),
                key = { files[it].uri },
            ) { index ->
                val a = files[index]
                when (a.kind) {
                    AttachmentKind.IMAGE -> ZoomableImage(a, onZoomChanged = { if (index == pager.currentPage) zoomed = it })
                    AttachmentKind.PDF -> PdfAttachment(a) { page, total -> pdfPage[index] = page to total }
                }
            }

            ThumbnailStrip(files, pager.currentPage) { scope.launch { zoomed = false; pager.animateScrollToPage(it) } }
        }

        LaunchedEffect(pager.currentPage) { zoomed = false }
    }
}

@Composable
private fun ZoomableImage(a: Attachment, onZoomChanged: (Boolean) -> Unit) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }

    fun set(newScale: Float, newOffset: Offset) {
        scale = newScale.coerceIn(1f, 5f)
        offset = if (scale > 1f) newOffset else Offset.Zero
        onZoomChanged(scale > 1f)
    }

    // The full image comes from the size-limited attachment cache (AttachmentFiles), not Coil's disk cache.
    val context = LocalContext.current
    val file by produceState<File?>(null, a.uri) { value = runCatching { AttachmentFiles.localFile(context, a.uri, a.name) }.getOrNull() }
    AsyncImage(
        model = file ?: a.thumbUri?.let { thumbnailRequest(context, it) }, contentDescription = a.name, contentScale = ContentScale.Fit,
        modifier = Modifier.fillMaxSize()
            .pointerInput(a.uri) {
                detectTapGestures(onDoubleTap = { if (scale > 1f) set(1f, Offset.Zero) else set(2.5f, Offset.Zero) })
            }
            .pointerInput(a.uri) {
                // Pinch always zooms; one-finger drags are only consumed while zoomed, so at 1× they reach the pager.
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    do {
                        val event = awaitPointerEvent()
                        if (event.changes.size > 1 || scale > 1f) {
                            set(scale * event.calculateZoom(), offset + event.calculatePan())
                            event.changes.forEach { if (it.positionChanged()) it.consume() }
                        }
                    } while (event.changes.any { it.pressed })
                }
            }
            .graphicsLayer { scaleX = scale; scaleY = scale; translationX = offset.x; translationY = offset.y },
    )
}

@Composable
private fun PdfAttachment(a: Attachment, onPage: (page: Int, total: Int) -> Unit) {
    val context = LocalContext.current
    val file by produceState<Result<File>?>(null, a.uri) {
        value = runCatching { AttachmentFiles.localFile(context, a.uri, a.name) }
    }
    when (val f = file) {
        null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = Color.White) }
        else -> f.fold(
            onSuccess = { PdfPages(it, onPage) },
            onFailure = {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("Couldn't open ${a.name}. Swipe to the next file or try again later.", color = Color.White,
                        style = HType.body, modifier = Modifier.padding(24.dp))
                }
            },
        )
    }
}

@Composable
private fun PdfPages(file: File, onPage: (Int, Int) -> Unit) {
    val renderer = remember(file) { PdfRenderer(ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)) }
    val lock = remember(file) { Mutex() }          // PdfRenderer allows one open page at a time
    DisposableEffect(renderer) { onDispose { renderer.close() } }

    val listState: LazyListState = rememberLazyListState()
    val current by remember { derivedStateOf { listState.firstVisibleItemIndex + 1 } }
    LaunchedEffect(current) { onPage(current, renderer.pageCount) }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val widthPx = constraints.maxWidth - with(androidx.compose.ui.platform.LocalDensity.current) { 32.dp.roundToPx() }
        ScrollbarLazyColumn(
            state = listState, contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.fillMaxSize(),
        ) {
            items(renderer.pageCount) { index -> PdfPage(renderer, lock, index, widthPx) }
        }
    }
}

@Composable
private fun PdfPage(renderer: PdfRenderer, lock: Mutex, index: Int, widthPx: Int) {
    val bitmap by produceState<Bitmap?>(null, index, widthPx) {
        value = withContext(Dispatchers.IO) {
            lock.withLock {
                renderer.openPage(index).use { page ->
                    val height = (widthPx * page.height / page.width.toFloat()).toInt()
                    Bitmap.createBitmap(widthPx, height, Bitmap.Config.ARGB_8888).also {
                        it.eraseColor(android.graphics.Color.WHITE)
                        page.render(it, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
                    }
                }
            }
        }
    }
    val bmp = bitmap
    if (bmp == null) Box(Modifier.fillMaxWidth().aspectRatio(0.707f).background(Color.White.copy(alpha = 0.08f)))
    else Image(bmp.asImageBitmap(), contentDescription = "Page ${index + 1}", modifier = Modifier.fillMaxWidth().aspectRatio(bmp.width / bmp.height.toFloat()))
}

@Composable
private fun ThumbnailStrip(files: List<Attachment>, current: Int, onPick: (Int) -> Unit) {
    val state = rememberLazyListState()
    LaunchedEffect(current) { state.animateScrollToItem((current - 2).coerceAtLeast(0)) }
    LazyRow(
        state = state, modifier = Modifier.fillMaxWidth().navigationBarsPadding().padding(vertical = 10.dp),
        contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        itemsIndexed(files, key = { _, a -> a.uri }) { i, a ->
            val on = i == current
            Box(
                Modifier.size(56.dp).clip(RoundedCornerShape(10.dp)).background(Color.White.copy(alpha = 0.1f))
                    .border(if (on) 2.dp else 0.dp, if (on) Color.White else Color.Transparent, RoundedCornerShape(10.dp))
                    .clickable { onPick(i) }.semantics { contentDescription = "Go to ${a.name}" },
                contentAlignment = Alignment.Center,
            ) {
                if (a.kind == AttachmentKind.IMAGE || a.thumbUri != null)
                    AsyncImage(thumbnailRequest(LocalContext.current, a.thumbUri ?: a.uri), null, contentScale = ContentScale.Crop, alignment = if (a.kind == AttachmentKind.PDF) Alignment.TopCenter else Alignment.Center, modifier = Modifier.fillMaxSize())
                else Icon(Icons.Outlined.PictureAsPdf, null, tint = Color.White)
            }
        }
    }
}
