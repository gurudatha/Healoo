package com.healoo.app.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity

/**
 * Header that shrinks from [expandedFraction] to [collapsedFraction] of the screen height while the
 * content scrolls, then stays pinned (design doc 3.3). Scrolling up collapses the header first;
 * scrolling down expands it only once the list has reached its top.
 *
 * [header] receives progress: 0f = fully expanded, 1f = fully collapsed.
 */
@Composable
fun CollapsingHeaderLayout(
    expandedFraction: Float,
    collapsedFraction: Float,
    header: @Composable (progress: Float) -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    BoxWithConstraints(modifier.fillMaxSize()) {
        val density = LocalDensity.current
        // Ratios apply to the screen; the status bar inset is added on top so content never sits under it.
        val inset = WindowInsets.statusBars.getTop(density).toFloat()
        val maxPx = constraints.maxHeight * expandedFraction + inset
        val minPx = constraints.maxHeight * collapsedFraction + inset
        val range = (maxPx - minPx).coerceAtLeast(1f)
        var offset by remember { mutableFloatStateOf(0f) }   // 0 .. -range

        val connection = remember(range) {
            object : NestedScrollConnection {
                override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                    if (available.y >= 0f) return Offset.Zero            // only collapse here
                    val new = (offset + available.y).coerceIn(-range, 0f)
                    val consumed = new - offset
                    offset = new
                    return Offset(0f, consumed)
                }

                override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
                    if (available.y <= 0f) return Offset.Zero            // expand with leftover downward scroll
                    val new = (offset + available.y).coerceIn(-range, 0f)
                    val used = new - offset
                    offset = new
                    return Offset(0f, used)
                }
            }
        }

        Column(Modifier.fillMaxSize().nestedScroll(connection)) {
            val heightDp = with(density) { (maxPx + offset).toDp() }
            Box(Modifier.fillMaxWidth().height(heightDp)) { header(-offset / range) }
            content()
        }
    }
}
