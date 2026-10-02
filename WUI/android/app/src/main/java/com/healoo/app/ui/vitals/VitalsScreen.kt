package com.healoo.app.ui.vitals

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
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
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import com.healoo.app.ui.upload.SageTextField
import kotlinx.coroutines.launch

/** [userId] null = the caller's own vitals; otherwise a patient's, seen by their doctor. */
class VitalsViewModel(private val userId: String?) : ViewModel() {
    private val repo = ServiceLocator.repository
    var patient by mutableStateOf<UserProfile?>(null); private set
    var types by mutableStateOf<List<VitalType>?>(null); private set
    var latest by mutableStateOf<Map<String, VitalReading>>(emptyMap()); private set
    var readings by mutableStateOf<List<VitalReading>>(emptyList()); private set
    var settings by mutableStateOf(VitalSettings()); private set
    /** History filter: one vital's code, or null for all (always newest entry first). */
    var filter by mutableStateOf<String?>(null); private set
    var error by mutableStateOf<String?>(null); private set
    var busy by mutableStateOf(false); private set
    var sheetError by mutableStateOf<String?>(null)
    val asDoctor get() = userId != null

    init { load() }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            if (userId != null) patient = repo.user(userId)
            settings = repo.vitalSettings()
            types = repo.vitalTypes(userId)
            latest = repo.vitalLatest(userId).associateBy { it.code }
            readings = repo.vitalReadings(userId, filter)
        }.onFailure { error = "Couldn't load vitals: ${it.message}" }
    }

    fun filterBy(code: String?) { filter = code; viewModelScope.launch { runCatching { readings = repo.vitalReadings(userId, code) } } }

    private fun act(onDone: () -> Unit, block: suspend () -> Unit) {
        busy = true; sheetError = null
        viewModelScope.launch {
            runCatching { block() }.onSuccess { onDone(); load() }.onFailure { sheetError = it.message ?: "That didn't work. Try again." }
            busy = false
        }
    }

    fun record(t: VitalType, values: Map<String, Double>, note: String?, takenAt: String?, onDone: () -> Unit) = act(onDone) { repo.recordVital(NewVitalReading(t.code, values, note, takenAt)) }
    fun setRange(t: VitalType, update: VitalRangeUpdate, onDone: () -> Unit) = act(onDone) { repo.setVitalRange(userId!!, t.code, update) }
    fun addType(type: NewVitalType, onDone: () -> Unit) = act(onDone) { repo.addVitalType(type) }
}

private fun num(v: Double?) = v?.let { if (it == Math.floor(it)) it.toLong().toString() else it.toString() }.orEmpty()

/** "90–120 / 60–80 · yellow up to 30% outside", or "No range set". */
private fun rangeText(t: VitalType): String {
    val parts = t.components.map { c ->
        when {
            c.low != null && c.high != null -> "${num(c.low)}–${num(c.high)}"
            c.low != null -> "from ${num(c.low)}"
            c.high != null -> "up to ${num(c.high)}"
            else -> null
        }
    }
    val byRange = if (parts.all { it == null }) null
        else parts.joinToString(" / ") { it ?: "any" } + " ${t.unit} · yellow up to ${t.marginPct ?: 30}% outside"
    // Urine: coloured by output as a percentage of intake (thresholds from the server's configuration).
    val byRatio = t.ratio?.let { r ->
        val name = { key: String -> t.components.firstOrNull { it.key == key }?.label?.lowercase() ?: key }
        if (r.greenMinPct == null || r.yellowMinPct == null) null
        else "${name(r.numerator)} ${num(r.greenMinPct)}% or more of ${name(r.denominator)} is green, from ${num(r.yellowMinPct)}% yellow, less red"
    }
    return listOfNotNull(byRatio?.replaceFirstChar { it.uppercase() }, byRange).joinToString(" · ").ifEmpty { "No range set: every value is green" }
}

/**
 * A person's vitals: every vital with its latest value and colour, then the history, newest entry
 * first. The person taps a vital to enter a value. Their doctor instead sees and edits the range
 * of each vital and can add a vital for this patient; ranges are never shown to the patient.
 */
@Composable
fun VitalsScreen(userId: String?, onBack: () -> Unit) {
    val vm: VitalsViewModel = viewModel(key = "vitals-$userId") { VitalsViewModel(userId) }
    var entering by remember { mutableStateOf<VitalType?>(null) }
    var editing by remember { mutableStateOf<VitalType?>(null) }
    var adding by remember { mutableStateOf(false) }
    val types = vm.types

    Scaffold(
        containerColor = Sage.Background,
        topBar = { PinnedHeader("Vitals", if (vm.asDoctor) vm.patient?.displayName ?: "…" else "Your readings", onBack) },
    ) { padding ->
        ScrollbarLazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            when {
                vm.error != null -> item { ErrorBox(vm.error!!, vm::load) }
                types == null -> item { LoadingBox() }
                else -> {
                    if (vm.asDoctor) item {
                        Text("Ranges are set per patient and are not shown to them. A reading inside the range is green, " +
                            "outside by up to the margin yellow, and beyond that red. Changes apply to new readings.",
                            style = HType.caption, color = Sage.SandInk,
                            modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Sage.SandTint).padding(12.dp))
                    }
                    item {
                        GroupCard {
                            types.forEachIndexed { i, t ->
                                if (i > 0) RowDivider()
                                VitalRow(t, vm.latest[t.code], vm.asDoctor) { vm.sheetError = null; if (vm.asDoctor) editing = t else entering = t }
                            }
                        }
                    }
                    if (vm.asDoctor) item { SecondaryButton("Add a vital for this patient", { vm.sheetError = null; adding = true }, Modifier.fillMaxWidth()) }
                    item {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            SectionHeader("History")
                            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                SageChip("All", vm.filter == null, { vm.filterBy(null) })
                                types.forEach { t -> SageChip(t.label, vm.filter == t.code, { vm.filterBy(t.code) }) }
                            }
                        }
                    }
                    if (vm.readings.isEmpty()) item { Text("No readings yet.", style = HType.body, color = Sage.Muted) }
                    else item {
                        GroupCard {
                            vm.readings.forEachIndexed { i, r ->
                                if (i > 0) RowDivider()
                                ReadingRow(types.firstOrNull { it.code == r.code }, r, showLabel = vm.filter == null)
                            }
                        }
                    }
                }
            }
        }
    }

    entering?.let { t ->
        VitalEntrySheet(t, vm.readings.filter { it.code == t.code }, vm.busy, vm.sheetError, vm.settings.backdateHours, onDismiss = { entering = null }) { values, note, takenAt ->
            vm.record(t, values, note, takenAt) { entering = null }
        }
    }
    editing?.let { t -> RangeSheet(t, vm.settings, vm.busy, vm.sheetError, onDismiss = { editing = null }) { vm.setRange(t, it) { editing = null } } }
    if (adding) AddVitalSheet(vm.settings, vm.busy, vm.sheetError, onDismiss = { adding = false }) { label, icon, unit, low, high, margin ->
        vm.addType(NewVitalType(userId!!, label, icon, unit, low, high, margin)) { adding = false }
    }
}

@Composable
private fun VitalRow(t: VitalType, latest: VitalReading?, asDoctor: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).background(severityTint(latest?.severity)), contentAlignment = Alignment.Center) {
            Icon(vitalIcon(t.icon), null, tint = severityColor(latest?.severity), modifier = Modifier.size(20.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(t.label + if (t.custom) " · added by doctor" else "", style = HType.bodyStrong, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(if (asDoctor) rangeText(t) else latest?.let { "Last entered ${vitalTime(it.takenAt)}" } ?: "Tap to enter a value",
                style = HType.small, color = Sage.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Column(horizontalAlignment = Alignment.End) {
            Text(latest?.let { formatVital(it.values, t.unit, t.components.map { c -> c.key }) } ?: "–", style = HType.bodyStrong, color = severityColor(latest?.severity))
            Text(if (asDoctor) "Set range" else latest?.severity?.label ?: "", style = HType.small, color = if (asDoctor) Sage.Accent else severityColor(latest?.severity))
        }
    }
}

@Composable
private fun NumberField(label: String, value: String, onChange: (String) -> Unit, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        FieldLabel(label)
        OutlinedTextField(
            value = value, onValueChange = { v -> onChange(v.filter { it.isDigit() || it == '.' }.take(8)) }, singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal), textStyle = HType.body.copy(color = Sage.Ink),
            shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth(),
            colors = OutlinedTextFieldDefaults.colors(focusedContainerColor = Sage.Surface, unfocusedContainerColor = Sage.Surface,
                focusedBorderColor = Sage.Accent, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Accent),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Sheet(title: String, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Sage.Background, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).imePadding().padding(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text(title, style = HType.section, color = Sage.Ink)
            content()
        }
    }
}

/** Doctor: this patient's range for one vital, and how far outside still counts as yellow. */
@Composable
private fun RangeSheet(t: VitalType, settings: VitalSettings, busy: Boolean, error: String?, onDismiss: () -> Unit, onSave: (VitalRangeUpdate) -> Unit) {
    val low = remember(t.code) { mutableStateMapOf<String, String>().apply { t.components.forEach { put(it.key, num(it.low)) } } }
    val high = remember(t.code) { mutableStateMapOf<String, String>().apply { t.components.forEach { put(it.key, num(it.high)) } } }
    var margin by remember(t.code) { mutableStateOf((t.marginPct ?: settings.yellowMarginPct).toString()) }
    val bounds = t.components.associate { it.key to VitalBounds(low[it.key]?.toDoubleOrNull(), high[it.key]?.toDoubleOrNull()) }
    val m = margin.toIntOrNull()
    val problem = when {
        bounds.values.any { it.low != null && it.high != null && it.low > it.high } -> "The low end can't be above the high end."
        m == null || m !in settings.marginPctMin..settings.marginPctMax -> "The margin must be ${settings.marginPctMin}–${settings.marginPctMax}%."
        else -> null
    }
    Sheet("${t.label} range", onDismiss) {
        Text("Leave an end empty for no limit on that side.", style = HType.caption, color = Sage.Muted)
        t.components.forEach { c ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                val name = if (t.components.size > 1) "${c.label} " else ""
                NumberField("${name}from" + (if (t.unit.isBlank()) "" else " (${t.unit})"), low[c.key].orEmpty(), { low[c.key] = it }, Modifier.weight(1f))
                NumberField("${name}to", high[c.key].orEmpty(), { high[c.key] = it }, Modifier.weight(1f))
            }
        }
        NumberField("Yellow up to this % outside the range (red beyond)", margin, { margin = it.filter(Char::isDigit).take(3) })
        (problem ?: error)?.let { Text(it, style = HType.small, color = Sage.Clay) }
        PrimaryButton(if (busy) "Saving…" else "Save range", { onSave(VitalRangeUpdate(bounds, m)) }, Modifier.fillMaxWidth(), enabled = problem == null && !busy)
    }
}

/** Doctor: a new vital for this patient, with an icon from the predefined set and its range. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AddVitalSheet(
    settings: VitalSettings, busy: Boolean, error: String?, onDismiss: () -> Unit,
    onAdd: (label: String, icon: String, unit: String, low: Double?, high: Double?, margin: Int?) -> Unit,
) {
    var label by remember { mutableStateOf("") }
    var icon by remember { mutableStateOf(settings.icons.lastOrNull() ?: "other") }
    var unit by remember { mutableStateOf("") }
    var low by remember { mutableStateOf("") }
    var high by remember { mutableStateOf("") }
    var margin by remember { mutableStateOf(settings.yellowMarginPct.toString()) }
    val (lo, hi, m) = Triple(low.toDoubleOrNull(), high.toDoubleOrNull(), margin.toIntOrNull())
    val problem = when {
        lo != null && hi != null && lo > hi -> "The low end can't be above the high end."
        m == null || m !in settings.marginPctMin..settings.marginPctMax -> "The margin must be ${settings.marginPctMin}–${settings.marginPctMax}%."
        else -> null
    }
    Sheet("Add a vital", onDismiss) {
        SageTextField("Name", label, { label = it.take(30) }, placeholder = "e.g. Peak flow")
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            FieldLabel("Icon")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                settings.icons.forEach { k ->
                    val on = k == icon
                    Box(
                        Modifier.size(44.dp).clip(RoundedCornerShape(12.dp)).background(if (on) Sage.SageTint else Sage.Surface)
                            .border(1.dp, if (on) Sage.Accent else Sage.Border, RoundedCornerShape(12.dp))
                            .clickable(role = Role.RadioButton) { icon = k }.semantics { selected = on; contentDescription = k },
                        contentAlignment = Alignment.Center,
                    ) { Icon(vitalIcon(k), null, tint = if (on) Sage.Accent else Sage.Muted, modifier = Modifier.size(22.dp)) }
                }
            }
        }
        SageTextField("Unit (optional)", unit, { unit = it.take(12) }, placeholder = "e.g. L/min")
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            NumberField("Range from", low, { low = it }, Modifier.weight(1f))
            NumberField("to", high, { high = it }, Modifier.weight(1f))
        }
        NumberField("Yellow up to this % outside the range (red beyond)", margin, { margin = it.filter(Char::isDigit).take(3) })
        (problem ?: error)?.let { Text(it, style = HType.small, color = Sage.Clay) }
        PrimaryButton(if (busy) "Adding…" else "Add vital", { onAdd(label.trim(), icon, unit.trim(), lo, hi, m) }, Modifier.fillMaxWidth(),
            enabled = problem == null && !busy && label.any { it.isLetterOrDigit() })
    }
}
