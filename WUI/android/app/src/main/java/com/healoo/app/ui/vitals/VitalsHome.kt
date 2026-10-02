package com.healoo.app.ui.vitals

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter




/** "2 Oct 08:15" in the phone's time zone. */
fun vitalTime(iso: String): String = runCatching {
    OffsetDateTime.parse(iso).atZoneSameInstant(ZoneId.systemDefault()).format(DateTimeFormatter.ofPattern("d MMM HH:mm"))
}.getOrDefault(iso.take(16).replace('T', ' '))


/**
 * Home, for a user: a block of vitals (two rows of five by default). Each tile shows the icon in the colour of the latest
 * reading (grey when nothing is recorded) and that value; tapping it opens the entry sheet.
 * The tenth tile opens the full list, where vitals a doctor added also appear.
 */
@Composable
fun VitalTileGrid(types: List<VitalType>, latest: Map<String, VitalReading>, settings: VitalSettings, onTile: (VitalType) -> Unit, onMore: () -> Unit) {
    // Columns and rows come from the server's vitals configuration (home.columns / home.rows).
    val columns = settings.homeColumns.coerceIn(2, 6)
    val shown = types.take(columns * settings.homeRows.coerceIn(1, 4) - 1)
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface).padding(vertical = 4.dp)) {
        (shown.map { it as VitalType? } + null).chunked(columns).forEach { row ->
            Row(Modifier.fillMaxWidth()) {
                row.forEach { t ->
                    if (t == null) Tile("More", vitalIcon("more"), null, if (types.size > shown.size) "+${types.size - shown.size}" else "All", Modifier.weight(1f), onMore)
                    else latest[t.code].let { r -> Tile(t.label, vitalIcon(t.icon), r?.severity, r?.let { formatVital(it.values, "", t.components.map { c -> c.key }) } ?: "–", Modifier.weight(1f)) { onTile(t) } }
                }
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
private fun Tile(label: String, icon: androidx.compose.ui.graphics.vector.ImageVector, severity: Severity?, value: String, modifier: Modifier, onClick: () -> Unit) {
    val state = when (severity) { null -> ""; else -> ", ${severity.label.lowercase()}" }
    Column(
        modifier.clip(RoundedCornerShape(10.dp)).clickable(role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = "$label $value$state" }.padding(vertical = 5.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, contentDescription = null, tint = if (label == "More") Sage.Accent else severityColor(severity), modifier = Modifier.size(22.dp))
        Text(label, style = HType.tiny, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(value, style = HType.tiny.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Normal), color = severityColor(severity), maxLines = 1)
    }
}

/**
 * Home, for a doctor: unread alerts from patients as red, yellow and green — one row, the same
 * size as the bottom bar's icons. Tapping a colour lists the patients behind it.
 */
@Composable
fun AlertDashboardRow(counts: VitalAlertCounts?, onOpen: (Severity) -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface).height(46.dp),
        horizontalArrangement = Arrangement.SpaceAround, verticalAlignment = Alignment.CenterVertically,
    ) {
        Severity.entries.forEach { s ->
            val n = counts?.get(s)
            Column(
                Modifier.width(84.dp).height(44.dp).clip(RoundedCornerShape(10.dp)).clickable(role = Role.Button) { onOpen(s) }
                    .semantics(mergeDescendants = true) { contentDescription = "${n ?: 0} ${s.label.lowercase()} alerts" },
                horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
            ) {
                // Grey when there is nothing to look at, so colour means "needs attention".
                val color = if ((n ?: 0) > 0) severityColor(s) else Sage.Muted
                Icon(alertIcon(s), contentDescription = null, tint = color, modifier = Modifier.size(22.dp))
                Spacer(Modifier.height(1.dp))
                Text("${s.label} · ${n ?: "–"}", style = HType.tiny, color = color, maxLines = 1)
            }
        }
    }
}

/** A coloured dot, value and time: one reading in a list. */
@Composable
fun ReadingRow(type: VitalType?, r: VitalReading, showLabel: Boolean = false) {
    Row(Modifier.fillMaxWidth().heightIn(min = 40.dp).padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.size(10.dp).clip(CircleShape).background(severityColor(r.severity)).semantics { contentDescription = r.severity.label })
        if (showLabel) Icon(vitalIcon(type?.icon ?: "other"), null, tint = Sage.Muted, modifier = Modifier.size(18.dp))
        Column(Modifier.weight(1f)) {
            Text((if (showLabel) "${type?.label ?: r.code} · " else "") + formatVital(r.values, type?.unit.orEmpty(), type?.components?.map { it.key }.orEmpty())
                + (type?.ratio?.percent(r.values)?.let { " · ${"%.0f".format(it)}%" } ?: ""),
                style = HType.bodyStrong, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            r.note?.let { Text(it, style = HType.small, color = Sage.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
        Text(vitalTime(r.takenAt), style = HType.small, color = Sage.Muted)
    }
}

/**
 * The most recent moment with this clock time: today, or yesterday if that time hasn't come yet
 * today. Null when it is more than [backdateHours] ago, which can't be entered.
 */
fun earlierTime(hour: Int, minute: Int, backdateHours: Int, now: ZonedDateTime = ZonedDateTime.now()): ZonedDateTime? {
    val today = now.withHour(hour).withMinute(minute).withSecond(0).withNano(0)
    val at = if (today.isAfter(now)) today.minusDays(1) else today
    return at.takeIf { !it.isBefore(now.minusHours(backdateHours.toLong())) }
}

private fun dayAndTime(at: ZonedDateTime, now: ZonedDateTime = ZonedDateTime.now()) =
    (if (at.toLocalDate() == now.toLocalDate()) "Today " else "Yesterday ") + at.format(DateTimeFormatter.ofPattern("HH:mm"))

/**
 * Enter a value of one vital, for now or for a time up to [backdateHours] ago (in case the
 * person couldn't enter it straight away). Once saved, a reading and its time can't be changed.
 * Shows the person's last few entries, newest first.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VitalEntrySheet(
    type: VitalType, recent: List<VitalReading>?, busy: Boolean, error: String?, backdateHours: Int,
    onDismiss: () -> Unit, onSave: (values: Map<String, Double>, note: String?, takenAt: String?) -> Unit,
) {
    val inputs = remember(type.code) { mutableStateMapOf<String, String>() }
    var note by remember(type.code) { mutableStateOf("") }
    val values = type.components.associate { it.key to inputs[it.key]?.trim()?.replace(',', '.')?.toDoubleOrNull() }
    // Urine: the percentage can't be worked out without some water intake.
    val ratioMissing = type.ratio != null && values.values.all { it != null } && type.ratio.percent(values) == null
    val complete = values.values.all { it != null && it >= 0 } && !ratioMissing
    var at by remember(type.code) { mutableStateOf<ZonedDateTime?>(null) }      // null = now
    var picking by remember { mutableStateOf(false) }
    var timeError by remember(type.code) { mutableStateOf<String?>(null) }

    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Sage.Background, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).imePadding().padding(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Icon(vitalIcon(type.icon), null, tint = Sage.Accent, modifier = Modifier.size(24.dp))
                Text(type.label, style = HType.section, color = Sage.Ink)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                type.components.forEach { c ->
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        FieldLabel(if (type.components.size == 1) "Value" + (if (type.unit.isBlank()) "" else " (${type.unit})") else c.label)
                        OutlinedTextField(
                            value = inputs[c.key].orEmpty(), onValueChange = { v -> inputs[c.key] = v.filter { it.isDigit() || it == '.' || it == ',' }.take(8) },
                            singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                            textStyle = HType.body.copy(color = Sage.Ink), shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth(),
                            suffix = if (type.components.size > 1 && c == type.components.last() && type.unit.isNotBlank()) ({ Text(type.unit, style = HType.small, color = Sage.Muted) }) else null,
                            colors = OutlinedTextFieldDefaults.colors(focusedContainerColor = Sage.Surface, unfocusedContainerColor = Sage.Surface,
                                focusedBorderColor = Sage.Accent, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Accent),
                        )
                    }
                }
            }
            type.ratio?.let { r ->
                val name = { key: String -> type.components.firstOrNull { it.key == key }?.label ?: key }
                val pct = r.percent(values)
                Text(
                    when {
                        pct != null -> "${name(r.numerator)} is ${"%.0f".format(pct)}% of ${name(r.denominator).lowercase()}."
                        ratioMissing -> "${name(r.denominator)} must be more than 0."
                        else -> "Enter both to compare ${name(r.numerator).lowercase()} with ${name(r.denominator).lowercase()}."
                    },
                    style = HType.caption, color = if (ratioMissing) Sage.Clay else Sage.Muted,
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                FieldLabel("Time")
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    SageChip("Now", at == null, { at = null; timeError = null })
                    SageChip(at?.let { dayAndTime(it) } ?: "Earlier…", at != null, { picking = true })
                }
                Text(timeError ?: "Up to $backdateHours hours back. The time can't be changed after saving.",
                    style = HType.small, color = if (timeError != null) Sage.Clay else Sage.Muted)
            }
            com.healoo.app.ui.upload.SageTextField("Note (optional)", note, { note = it.take(200) })
            error?.let { Text(it, style = HType.small, color = Sage.Clay) }
            PrimaryButton(if (busy) "Saving…" else "Save",
                { onSave(values.mapValues { it.value!! }, note.trim().ifEmpty { null }, at?.toOffsetDateTime()?.toString()) },
                Modifier.fillMaxWidth(), enabled = complete && !busy)

            if (!recent.isNullOrEmpty()) {
                FieldLabel("Recent entries")
                GroupCard { recent.take(5).forEachIndexed { i, r -> if (i > 0) RowDivider(); ReadingRow(type, r) } }
            }
        }
    }

    if (picking) {
        val now = ZonedDateTime.now()
        val state = rememberTimePickerState(initialHour = (at ?: now).hour, initialMinute = (at ?: now).minute, is24Hour = true)
        AlertDialog(
            onDismissRequest = { picking = false }, containerColor = Sage.Surface,
            title = { Text("When was it measured?", style = HType.section) },
            text = { Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { TimeInput(state) } },
            confirmButton = {
                TextButton({
                    val chosen = earlierTime(state.hour, state.minute, backdateHours)
                    if (chosen == null) { at = null; timeError = "That is more than $backdateHours hours ago, so it can't be entered. Using now." }
                    else { at = chosen; timeError = null }
                    picking = false
                }) { Text("Set time", color = Sage.Accent) }
            },
            dismissButton = { TextButton({ picking = false }) { Text("Cancel", color = Sage.Muted) } },
        )
    }
}
