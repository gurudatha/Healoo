package com.healoo.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.healoo.app.R

/**
 * Sage Clinic tokens — mirrors design-tokens.json. Theme colour #AAB5AD (grey-green).
 *
 * Primary is light, so the roles are split:
 * - [Primary] and [PrimaryRaised] are surfaces (headers, buttons, selected chips, tiles), with
 *   [OnPrimary] / [OnPrimarySoft] text and icons on them (white on #AAB5AD would be ~2:1 contrast).
 * - [Accent], a darker shade of the same colour, is for links, icons and text on light backgrounds.
 */
object Sage {
    val Primary = Color(0xFFAAB5AD)
    val PrimaryRaised = Color(0xFFC3CBC5)
    val PrimaryPressed = Color(0xFF97A39A)
    val OnPrimary = Color(0xFF1F2A27)
    val OnPrimarySoft = Color(0xFF34403A)
    val OnPrimaryLine = Color(0xFF7F8C84)
    val Accent = Color(0xFF4B5A51)
    /** Background of closed items (open ones are white). */
    val Closed = Color(0xFFE3E4E2)

    val Background = Color(0xFFF5F3EE)
    val Surface = Color(0xFFFFFFFF)
    val Sunken = Color(0xFFE9E5DC)
    val Preview = Color(0xFFEDEAE2)

    val Ink = Color(0xFF1F2A27)
    val InkSoft = Color(0xFF3E4A46)
    val Muted = Color(0xFF56625E)
    val Placeholder = Color(0xFF66706C)

    val Border = Color(0xFFD9D5CB)
    val Divider = Color(0xFFE4E0D6)
    val RowDivider = Color(0xFFEEEBE4)

    val SageTint = Color(0xFFE4E9E5)
    val Avatar = Color(0xFFE6EBE7)
    val Clay = Color(0xFF9A4A26)
    val ClayTint = Color(0xFFF6E6DC)
    val ClayBorder = Color(0xFFE8CFC0)
    val Sand = Color(0xFF5B5340)
    val SandTint = Color(0xFFEFEADF)
    val SandInk = Color(0xFF4A4436)
    val SwitchOff = Color(0xFFCFCAC0)
    val Dashed = Color(0xFFB8C1BA)
    val ViewerBackground = Color(0xFF141A18)
}

object Radius {
    val header = 28.dp
    val card = 16.dp
    val tile = 12.dp
    val field = 14.dp
    val chip = 20.dp
    val pill = 25.dp
}

/** Header heights as a fraction of screen height (design doc 3.2 / 3.3). */
object HeaderRatio {
    const val LANDING = 0.20f
    const val USER_EXPANDED = 0.25f
    const val USER_COLLAPSED = 0.10f
    const val PINNED = 0.10f
}

val Fraunces = FontFamily(Font(R.font.fraunces_semibold, FontWeight.SemiBold))
val Figtree = FontFamily(
    Font(R.font.figtree_regular, FontWeight.Normal),
    Font(R.font.figtree_medium, FontWeight.Medium),
    Font(R.font.figtree_semibold, FontWeight.SemiBold),
)

object HType {
    val screenTitle = TextStyle(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, fontSize = 26.sp, lineHeight = 31.sp)
    val greeting = TextStyle(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, fontSize = 24.sp, lineHeight = 29.sp)
    val profileName = TextStyle(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, fontSize = 23.sp, lineHeight = 28.sp)
    val headerTitle = TextStyle(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, fontSize = 18.sp, lineHeight = 22.sp)
    val section = TextStyle(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, fontSize = 19.sp, lineHeight = 24.sp)
    val counter = TextStyle(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, fontSize = 26.sp, lineHeight = 26.sp)

    val body = TextStyle(fontFamily = Figtree, fontWeight = FontWeight.Normal, fontSize = 15.sp, lineHeight = 21.sp)
    val bodyStrong = body.copy(fontWeight = FontWeight.SemiBold)
    val caption = TextStyle(fontFamily = Figtree, fontWeight = FontWeight.Normal, fontSize = 13.sp, lineHeight = 18.sp)
    val label = caption.copy(fontWeight = FontWeight.SemiBold, color = Sage.InkSoft)
    val small = TextStyle(fontFamily = Figtree, fontWeight = FontWeight.Normal, fontSize = 12.sp, lineHeight = 16.sp)
    val tiny = TextStyle(fontFamily = Figtree, fontWeight = FontWeight.SemiBold, fontSize = 11.sp, lineHeight = 14.sp)
}

private val colors = lightColorScheme(
    // Material's "primary" colours text buttons, focused fields and switches on light backgrounds.
    primary = Sage.Accent,
    onPrimary = Color.White,
    primaryContainer = Sage.SageTint,
    onPrimaryContainer = Sage.Accent,
    secondaryContainer = Sage.SageTint,
    onSecondaryContainer = Sage.Accent,
    background = Sage.Background,
    onBackground = Sage.Ink,
    surface = Sage.Surface,
    onSurface = Sage.Ink,
    surfaceVariant = Sage.Sunken,
    onSurfaceVariant = Sage.Muted,
    outline = Sage.Border,
    outlineVariant = Sage.Divider,
    error = Sage.Clay,
)

private val typography = Typography(
    headlineMedium = HType.screenTitle,
    titleLarge = HType.section,
    titleMedium = HType.bodyStrong,
    bodyLarge = HType.body,
    bodyMedium = HType.caption,
    bodySmall = HType.small,
    labelLarge = HType.bodyStrong.copy(fontSize = 14.sp),
    labelMedium = HType.small.copy(fontWeight = FontWeight.SemiBold),
    labelSmall = HType.tiny,
)

@Composable
fun HealooTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = colors, typography = typography, content = content)
}
