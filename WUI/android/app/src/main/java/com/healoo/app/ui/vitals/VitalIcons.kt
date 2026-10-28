package com.healoo.app.ui.vitals

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp
import com.healoo.app.data.Severity
import com.healoo.app.ui.theme.Sage

// The predefined icon set for vitals. Keys match "icons" in Server/config/vitals.json; a doctor
// picks one when adding a vital. Drawn here as simple line icons (24 x 24, SVG path data), so the
// app doesn't need the full Material icon library.

private val paths = mapOf(
    "water" to "M7 4h10l-1.5 16h-7z M7.6 10h8.8",                                                   // glass
    "bp" to "M12 20C5 14.5 3 11.5 3 8.5a4.5 4.5 0 0 1 9-1.2a4.5 4.5 0 0 1 9 1.2c0 3-2 6-9 11.5z",   // heart
    "sugar" to "M12 3l7.5 4.5v9L12 21l-7.5-4.5v-9z M12 9v6 M9 12h6",                                // hexagon +
    "heart" to "M3 12h4l2-5l4 10l2-5h6",                                                            // pulse
    "urine" to "M12 3C12 3 6 10 6 14.5a6 6 0 0 0 12 0C18 10 12 3 12 3z",                            // drop
    "weight" to "M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z M8.5 10a4 4 0 0 1 7 0 M12 10l1.5-2",
    "temperature" to "M10 14V5a2 2 0 0 1 4 0v9a4 4 0 1 1-4 0z M12 9v8",
    "oxygen" to "M4 12a5 5 0 1 0 10 0a5 5 0 1 0-10 0 M16.5 15a1.6 1.6 0 0 1 3.2 0c0 1.6-3.2 2.4-3.2 4h3.2",
    "sleep" to "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z",                                 // moon
    "steps" to "M5.5 9a2.5 4 0 1 0 5 0a2.5 4 0 1 0-5 0 M13.5 15a2.5 4 0 1 0 5 0a2.5 4 0 1 0-5 0",
    "pain" to "M13 3L5 13h6l-1 8l8-10h-6z",                                                         // bolt
    "breath" to "M3 9h10a2.5 2.5 0 1 0-2.5-2.5 M3 13h15a2.5 2.5 0 1 1-2.5 2.5 M3 17h7",              // wind
    "pill" to "M4.5 12a4 4 0 0 1 4-4h7a4 4 0 0 1 0 8h-7a4 4 0 0 1-4-4z M12 8v8",
    "lab" to "M9 3h6 M10 3v6l-5 9.5a1.5 1.5 0 0 0 1.3 2.5h11.4a1.5 1.5 0 0 0 1.3-2.5L14 9V3 M7.5 15h9",
    "mood" to "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0 M8 14.5a4.5 4.5 0 0 0 8 0 M9 9.5v.5 M15 9.5v.5",
    "other" to "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0 M12 8v8 M8 12h8",                               // circled +
    "more" to "M5 12h.01 M12 12h.01 M19 12h.01",
    // Doctor dashboard: one shape per colour, so the three aren't told apart by colour alone.
    "alert_red" to "M12 3L2.5 20h19z M12 10v4.5 M12 17.3v.2",                                       // triangle !
    "alert_yellow" to "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0 M12 7.5v5 M12 16v.2",                    // circle !
    "alert_green" to "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0 M8 12.5l2.8 2.8L16 9.5",                  // circle ✓
)

private val cache = HashMap<String, ImageVector>()

/** The icon for a key from the predefined set; unknown keys (from a newer server) get "other". */
fun vitalIcon(key: String): ImageVector {
    val k = if (key in paths) key else "other"
    return cache.getOrPut(k) {
        ImageVector.Builder(name = "Vital.$k", defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f)
            .addPath(
                pathData = addPathNodes(paths.getValue(k)), fill = null, stroke = SolidColor(Color.Black),
                strokeLineWidth = if (k == "more") 2.6f else 1.7f, strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round,
            ).build()
    }
}

fun alertIcon(s: Severity): ImageVector = vitalIcon("alert_" + s.name.lowercase())

/** Red, yellow and green as used for readings and the doctor dashboard (readable on white). */
fun severityColor(s: Severity?): Color = when (s) {
    Severity.RED -> Color(0xFFC62828)
    Severity.YELLOW -> Color(0xFFB26A00)
    Severity.GREEN -> Color(0xFF2E7D32)
    null -> Sage.Muted                         // nothing recorded yet
}

fun severityTint(s: Severity?): Color = when (s) {
    Severity.RED -> Color(0xFFFCE8E6)
    Severity.YELLOW -> Color(0xFFFFF4D6)
    Severity.GREEN -> Color(0xFFE4F3E5)
    null -> Sage.Sunken
}
