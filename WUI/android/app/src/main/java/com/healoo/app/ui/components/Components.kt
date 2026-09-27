package com.healoo.app.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import com.healoo.app.ui.icons.automirrored.outlined.Chat
import androidx.compose.material.icons.outlined.*
import com.healoo.app.ui.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.healoo.app.data.OpId
import com.healoo.app.data.PageKind
import com.healoo.app.data.PageOperations
import com.healoo.app.data.PrimaryKind
import com.healoo.app.data.DataItem
import com.healoo.app.data.ItemStatus
import com.healoo.app.ui.theme.*
import java.time.LocalDate
import java.time.format.DateTimeFormatter

data class TypeStyle(val tint: Color, val fg: Color, val icon: ImageVector)

fun styleFor(kind: PrimaryKind): TypeStyle = when (kind) {
    PrimaryKind.REPORT -> TypeStyle(Sage.SageTint, Sage.Accent, Icons.Outlined.Description)
    PrimaryKind.MESSAGE -> TypeStyle(Sage.SageTint, Sage.Accent, Icons.AutoMirrored.Outlined.Chat)
    PrimaryKind.APPOINTMENT -> TypeStyle(Sage.SandTint, Sage.Sand, Icons.Outlined.Event)
    PrimaryKind.ALERT -> TypeStyle(Sage.ClayTint, Sage.Clay, Icons.Outlined.NotificationsNone)
}

fun shortDate(iso: String): String = runCatching {
    val d = LocalDate.parse(iso)
    if (d == LocalDate.now()) "Today" else d.format(DateTimeFormatter.ofPattern("d MMM"))
}.getOrDefault(iso)

fun longDate(iso: String): String = runCatching {
    LocalDate.parse(iso).format(DateTimeFormatter.ofPattern("d MMM yyyy"))
}.getOrDefault(iso)

/** Screen-height fraction in dp — header sizes are defined as ratios (doc 3.2). */
@Composable
fun screenFraction(fraction: Float): Dp = (LocalConfiguration.current.screenHeightDp * fraction).dp

@Composable
fun TypeBadge(type: PrimaryKind) {
    val s = styleFor(type)
    Text(
        type.label, style = HType.tiny, color = s.fg,
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).background(s.tint).padding(horizontal = 8.dp, vertical = 2.dp),
    )
}

@Composable
fun TypeTile(type: PrimaryKind, size: Dp = 40.dp) {
    val s = styleFor(type)
    Box(
        Modifier.size(size).clip(RoundedCornerShape(Radius.tile)).background(s.tint),
        contentAlignment = Alignment.Center,
    ) { Icon(s.icon, contentDescription = null, tint = s.fg, modifier = Modifier.size(size / 2)) }
}

@Composable
fun DataItemRow(item: DataItem, onClick: () -> Unit, showDate: Boolean = true) {
    // Open items are white; closed ones grey, with muted text.
    val closed = item.status == ItemStatus.CLOSED
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(if (closed) Sage.Closed else Sage.Surface)
            .clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TypeTile(item.primaryKind)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(item.title, style = HType.bodyStrong, maxLines = 1, overflow = TextOverflow.Ellipsis, color = if (closed) Sage.Muted else Sage.Ink)
            Text(if (closed) "Closed · ${item.subtitle}" else item.subtitle, style = HType.caption, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Sage.Muted)
        }
        Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            if (showDate) Text(TimeText.activity(item.updatedAt.ifEmpty { item.createdAt }), style = HType.small, color = Sage.Muted)
            TypeBadge(item.primaryKind)
        }
    }
}

@Composable
fun Avatar(initials: String, size: Dp = 44.dp, background: Color = Sage.Avatar, border: Color? = null, photoUrl: String? = null) {
    Box(
        Modifier.size(size).clip(CircleShape).background(background)
            .then(if (border != null) Modifier.border(2.dp, border, CircleShape) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        // Initials underneath: shown while the photo loads, or when there is none.
        Text(initials, style = HType.section.copy(fontSize = (size.value * 0.34f).sp), color = Sage.Accent)
        if (!photoUrl.isNullOrBlank()) {
            coil.compose.AsyncImage(
                com.healoo.app.data.thumbnailRequest(androidx.compose.ui.platform.LocalContext.current, photoUrl),
                contentDescription = null, contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                modifier = Modifier.matchParentSize().clip(CircleShape),
            )
        }
    }
}

@Composable
fun SectionHeader(title: String, action: String? = null, onAction: () -> Unit = {}) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = HType.section, color = Sage.Ink)
        if (action != null) TextButton(onClick = onAction) {
            Text(action, style = HType.bodyStrong.copy(fontSize = 14.sp), color = Sage.Accent)
        }
    }
}

@Composable
fun FieldLabel(text: String, modifier: Modifier = Modifier) =
    Text(text, style = HType.label, modifier = modifier)

@Composable
/** [compact]: half height (20 dp) with smaller text, for the landing page's role filter. */
fun SageChip(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, onHeader: Boolean = false, compact: Boolean = false) {
    val (bg, fg, border) = when {
        onHeader && selected -> Triple(Color.White, Sage.Accent, Color.White)
        onHeader -> Triple(Sage.PrimaryRaised, Sage.OnPrimary, Sage.PrimaryRaised)
        selected -> Triple(Sage.Primary, Sage.OnPrimary, Sage.Primary)
        else -> Triple(Sage.Surface, Sage.Ink, Sage.Border)
    }
    Surface(
        onClick = onClick, shape = RoundedCornerShape(Radius.chip), color = bg, border = BorderStroke(1.dp, border),
        modifier = modifier.heightIn(min = if (compact) 20.dp else 40.dp).semantics { this.selected = selected },
    ) {
        Box(Modifier.padding(horizontal = if (compact) 10.dp else 14.dp), contentAlignment = Alignment.Center) {
            Text(label, style = (if (compact) HType.small else HType.caption).copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = fg)
        }
    }
}

/** Two-option segmented control used for tabs and Open/Closed. */
@Composable
fun Segmented(options: List<String>, selected: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier.clip(RoundedCornerShape(Radius.field)).background(Sage.Sunken).padding(3.dp),
    ) {
        options.forEachIndexed { i, label ->
            val on = i == selected
            Box(
                Modifier.weight(1f).heightIn(min = 40.dp).clip(RoundedCornerShape(11.dp))
                    .background(if (on) Sage.Surface else Color.Transparent)
                    .clickable(role = Role.Tab) { onSelect(i) }
                    .semantics { this.selected = on },
                contentAlignment = Alignment.Center,
            ) {
                Text(label, style = if (on) HType.bodyStrong.copy(fontSize = 14.sp) else HType.body.copy(fontSize = 14.sp),
                    color = if (on) Sage.Ink else Sage.Muted)
            }
        }
    }
}

/** 10% pinned header used by Data view and Upload. */
@Composable
fun PinnedHeader(
    title: String,
    subtitle: String,
    onBack: () -> Unit,
    backIcon: ImageVector = Icons.AutoMirrored.Outlined.ArrowBack,
    backLabel: String = "Back",
    trailing: @Composable () -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().background(Sage.Primary).statusBarsPadding()
            .height(screenFraction(HeaderRatio.PINNED)).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        HeaderIconButton(backIcon, backLabel, onBack)
        Column(Modifier.weight(1f)) {
            Text(title, style = HType.headerTitle, color = Sage.OnPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(subtitle, style = HType.small, color = Sage.OnPrimarySoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        trailing()
    }
}

@Composable
fun HeaderIconButton(icon: ImageVector, label: String, onClick: () -> Unit) {
    Box(
        Modifier.size(44.dp).clip(CircleShape).background(Sage.PrimaryRaised).clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) { Icon(icon, contentDescription = null, tint = Sage.OnPrimary, modifier = Modifier.size(20.dp)) }
}

/** The global screens; each is the operation of the same id in page-operations.json. */
enum class Tab(val opId: String) {
    HOME(OpId.HOME),
    SEARCH(OpId.SEARCH),
    UPLOAD(OpId.UPLOAD),
    MESSAGES(OpId.MESSAGES),
    SETTINGS(OpId.SETTINGS),
}

/** Bottom bar of the global (non-person) screens, from the "global" page in page-operations.json. */
@Composable
fun HealooBottomBar(current: Tab?, onSelect: (Tab) -> Unit) {
    val ops = remember { PageOperations.resolve(PageKind.GLOBAL, emptySet()) }
    OperationBar(ops, onOperation = { id -> Tab.entries.firstOrNull { it.opId == id }?.let(onSelect) }, selected = current?.opId)
}

@Composable
fun PrimaryButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    Button(
        onClick = onClick, enabled = enabled, modifier = modifier.height(50.dp),
        shape = RoundedCornerShape(Radius.pill),
        colors = ButtonDefaults.buttonColors(containerColor = Sage.Primary, contentColor = Sage.OnPrimary),
    ) { Text(text, style = HType.bodyStrong) }
}

@Composable
fun SecondaryButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    OutlinedButton(
        onClick = onClick, modifier = modifier.height(50.dp), enabled = enabled, shape = RoundedCornerShape(Radius.pill),
        border = BorderStroke(1.dp, Sage.Accent),
        colors = ButtonDefaults.outlinedButtonColors(contentColor = Sage.Accent),
    ) { Text(text, style = HType.bodyStrong) }
}

@Composable
fun BottomActionBar(content: @Composable RowScope.() -> Unit) {
    Column(Modifier.background(Sage.Surface).navigationBarsPadding()) {
        HorizontalDivider(color = Sage.Divider)
        Row(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp), content = content)
    }
}

@Composable
fun LoadingBox(modifier: Modifier = Modifier) =
    Box(modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = Sage.Accent) }

@Composable
fun ErrorBox(message: String, onRetry: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(message, style = HType.body, color = Sage.Ink)
        SecondaryButton("Try again", onRetry)
    }
}

/** A white card that groups rows with inset dividers (settings, access list). */
@Composable
fun GroupCard(content: @Composable ColumnScope.() -> Unit) =
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface), content = content)

@Composable
fun RowDivider() = HorizontalDivider(color = Sage.RowDivider, modifier = Modifier.padding(start = 14.dp))
