package com.healoo.app.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.dp
import com.healoo.app.ui.theme.Sage

/*
 * Scroll bars. Compose draws none by default, so a long list gave no hint that it scrolls.
 * The thumb shows whenever the content is taller than the screen: faint at rest, stronger while
 * scrolling. Use [ScrollbarLazyColumn] instead of LazyColumn, and [verticalScrollWithBar] instead
 * of verticalScroll.
 */

private val ThumbWidth = 4.dp
private val ThumbMinHeight = 32.dp
private val ThumbInset = 3.dp

/** A LazyColumn with a scroll bar. Same parameters as LazyColumn's commonly used ones. */
@Composable
fun ScrollbarLazyColumn(
    modifier: Modifier = Modifier,
    state: LazyListState = rememberLazyListState(),
    contentPadding: PaddingValues = PaddingValues(0.dp),
    verticalArrangement: Arrangement.Vertical = Arrangement.Top,
    content: LazyListScope.() -> Unit,
) = LazyColumn(
    modifier = modifier.verticalScrollbar(state),
    state = state,
    contentPadding = contentPadding,
    verticalArrangement = verticalArrangement,
    content = content,
)

/** verticalScroll plus a scroll bar drawn in the visible area. */
fun Modifier.verticalScrollWithBar(state: ScrollState): Modifier = verticalScrollbar(state).verticalScroll(state)

fun Modifier.verticalScrollbar(state: ScrollState): Modifier = composed {
    val alpha by animateFloatAsState(if (state.isScrollInProgress) 0.75f else 0.35f, label = "scrollbar")
    drawWithContent {
        drawContent()
        if (state.maxValue <= 0 || state.maxValue == Int.MAX_VALUE) return@drawWithContent
        val viewport = size.height
        val total = viewport + state.maxValue
        drawThumb(viewport * viewport / total, state.value.toFloat() / state.maxValue, alpha)
    }
}

fun Modifier.verticalScrollbar(state: LazyListState): Modifier = composed {
    val alpha by animateFloatAsState(if (state.isScrollInProgress) 0.75f else 0.35f, label = "scrollbar")
    drawWithContent {
        drawContent()
        val info = state.layoutInfo
        val visible = info.visibleItemsInfo
        if (visible.isEmpty() || !(state.canScrollForward || state.canScrollBackward)) return@drawWithContent
        // Item heights vary, so estimate the full length from the average visible item.
        val viewport = (info.viewportEndOffset - info.viewportStartOffset).toFloat()
        val avg = visible.sumOf { it.size + info.mainAxisItemSpacing }.toFloat() / visible.size
        val total = maxOf(avg * info.totalItemsCount + info.beforeContentPadding + info.afterContentPadding, viewport + 1f)
        val scrolled = state.firstVisibleItemIndex * avg + state.firstVisibleItemScrollOffset
        val fraction = when {
            !state.canScrollForward -> 1f
            !state.canScrollBackward -> 0f
            else -> (scrolled / (total - viewport)).coerceIn(0f, 0.98f)
        }
        drawThumb(viewport * viewport / total, fraction, alpha)
    }
}

private fun DrawScope.drawThumb(rawHeight: Float, fraction: Float, alpha: Float) {
    val width = ThumbWidth.toPx()
    val height = rawHeight.coerceIn(ThumbMinHeight.toPx(), size.height)
    val top = (size.height - height) * fraction.coerceIn(0f, 1f)
    drawRoundRect(
        color = Sage.Muted.copy(alpha = alpha),
        topLeft = Offset(size.width - width - ThumbInset.toPx(), top),
        size = Size(width, height),
        cornerRadius = CornerRadius(width / 2, width / 2),
    )
}
