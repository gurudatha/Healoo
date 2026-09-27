package com.healoo.app.ui.item

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Star
import com.healoo.app.ui.icons.outlined.StarOutline
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import com.healoo.app.ui.upload.SageTextField
import java.time.LocalDate
import java.time.LocalTime

// ------------------------------------------------------------------ repeat picker

/** Repeat (frequency) and period chips, with the visit count or the reason it is not allowed. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RepeatPicker(
    startDate: String,
    frequency: Frequency?, onFrequency: (Frequency?) -> Unit,
    period: Period?, onPeriod: (Period?) -> Unit,
    noun: String = "visit",
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel("Repeat")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            SageChip("No repeat", frequency == null, { onFrequency(null) })
            Frequency.entries.forEach { f -> SageChip(f.label, frequency == f, { onFrequency(f); if (f == Frequency.DAILY && period == null) onPeriod(Period.ONE_MONTH) }) }
        }
        if (frequency != null) {
            FieldLabel("For")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Period.entries.forEach { p -> SageChip(p.label, period == p, { onPeriod(p) }) }
                if (frequency != Frequency.DAILY) SageChip("Until cancelled", period == null, { onPeriod(null) })
            }
            val start = runCatching { LocalDate.parse(startDate) }.getOrNull()
            val rule = Recurrence(frequency, period)
            val problem = start?.let { RecurrenceRules.problem(it, rule) }
            val text = when {
                start == null -> "Enter the first date as YYYY-MM-DD."
                problem != null -> problem
                else -> RecurrenceRules.visitCount(start, rule)?.let { n ->
                    "$n ${noun}s, last on ${RecurrenceRules.nthDate(start, frequency, n - 1)}"
                } ?: "Repeats until cancelled."
            }
            Text(text, style = HType.small, color = if (problem != null) Sage.Clay else Sage.Muted)
        }
    }
}

// ------------------------------------------------------------------ appointment form

class AppointmentFormState(defaultDoctor: String?) {
    var doctorId by mutableStateOf(defaultDoctor)
    var date by mutableStateOf(LocalDate.now().plusDays(1).toString())
    var time by mutableStateOf("10:00")
    var frequency by mutableStateOf<Frequency?>(null)
    var period by mutableStateOf<Period?>(null)
    var notes by mutableStateOf("")

    fun recurrence() = frequency?.let { Recurrence(it, period) }

    /** Null when the form can be sent, otherwise what to fix. */
    fun problem(): String? {
        if (doctorId == null) return "Choose a doctor."
        val d = runCatching { LocalDate.parse(date.trim()) }.getOrNull() ?: return "Date must be YYYY-MM-DD."
        if (d < LocalDate.now()) return "The first visit can't be in the past."
        if (runCatching { LocalTime.parse(time.trim()) }.isFailure) return "Time must be HH:MM (24-hour)."
        return RecurrenceRules.problem(d, recurrence())
    }

    fun build(patientId: String) = NewAppointment(
        patientId = patientId, doctorId = doctorId!!, date = date.trim(), time = time.trim(),
        notes = notes.trim().ifEmpty { null }, recurrence = recurrence(),
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AppointmentForm(state: AppointmentFormState, doctors: List<UserProfile>) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        FieldLabel("Doctor")
        if (doctors.isEmpty()) Text("Add a doctor to your contacts from Search first.", style = HType.small, color = Sage.Muted)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            doctors.forEach { d -> SageChip(d.displayName, state.doctorId == d.id, { state.doctorId = d.id }) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(Modifier.weight(1f)) { SageTextField("First visit", state.date, { state.date = it }, placeholder = "YYYY-MM-DD") }
            Box(Modifier.weight(1f)) { SageTextField("Time", state.time, { state.time = it }, placeholder = "HH:MM") }
        }
        RepeatPicker(state.date, state.frequency, { state.frequency = it }, state.period, { state.period = it })
        SageTextField("Notes (optional)", state.notes, { state.notes = it })
    }
}

// ------------------------------------------------------------------ alert form

private val ALERT_TYPES = listOf("MEDICATION" to "Medication", "FOLLOW_UP" to "Follow-up", "CUSTOM" to "Other")

class AlertFormState {
    var type by mutableStateOf("MEDICATION")
    var text by mutableStateOf("")
    var date by mutableStateOf(LocalDate.now().toString())
    var time by mutableStateOf("20:00")
    var frequency by mutableStateOf<Frequency?>(Frequency.DAILY)
    var period by mutableStateOf<Period?>(Period.ONE_MONTH)

    fun recurrence() = frequency?.let { Recurrence(it, period) }

    fun problem(): String? {
        if (text.isBlank()) return "Write what the alert should say."
        val d = runCatching { LocalDate.parse(date.trim()) }.getOrNull() ?: return "Date must be YYYY-MM-DD."
        if (runCatching { LocalTime.parse(time.trim()) }.isFailure) return "Time must be HH:MM (24-hour)."
        return RecurrenceRules.problem(d, recurrence())
    }

    fun build(forUser: String? = null) = NewAlert(type, text.trim(), forUser, date.trim(), time.trim(), recurrence = recurrence())
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AlertForm(state: AlertFormState) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        FieldLabel("Type")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ALERT_TYPES.forEach { (k, label) -> SageChip(label, state.type == k, { state.type = k }) }
        }
        SageTextField("Alert text", state.text, { state.text = it }, placeholder = "e.g. Iron tablet after dinner")
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(Modifier.weight(1f)) { SageTextField("Starts", state.date, { state.date = it }, placeholder = "YYYY-MM-DD") }
            Box(Modifier.weight(1f)) { SageTextField("Time", state.time, { state.time = it }, placeholder = "HH:MM") }
        }
        RepeatPicker(state.date, state.frequency, { state.frequency = it }, state.period, { state.period = it }, noun = "alert")
    }
}

// ------------------------------------------------------------------ dialogs

@Composable
private fun FormDialog(title: String, confirm: String, problem: String?, onDismiss: () -> Unit, onConfirm: () -> Unit, content: @Composable () -> Unit) {
    var tried by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDismiss, containerColor = Sage.Surface,
        title = { Text(title, style = HType.section) },
        text = {
            Column(Modifier.verticalScrollWithBar(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                content()
                if (tried && problem != null) Text(problem, style = HType.small, color = Sage.Clay)
            }
        },
        confirmButton = { TextButton({ tried = true; if (problem == null) onConfirm() }) { Text(confirm, color = Sage.Primary) } },
        dismissButton = { TextButton(onDismiss) { Text("Cancel", color = Sage.Muted) } },
    )
}

@Composable
fun BookAppointmentDialog(patientId: String, doctors: List<UserProfile>, defaultDoctor: String?, onDismiss: () -> Unit, onBook: (NewAppointment) -> Unit) {
    val state = remember { AppointmentFormState(defaultDoctor ?: doctors.singleOrNull()?.id) }
    FormDialog("Book appointment", "Book", state.problem(), onDismiss, { onBook(state.build(patientId)) }) {
        AppointmentForm(state, doctors)
    }
}

@Composable
fun AddAlertDialog(onDismiss: () -> Unit, onAdd: (NewAlert) -> Unit) {
    val state = remember { AlertFormState() }
    FormDialog("Add alert", "Add", state.problem(), onDismiss, { onAdd(state.build()) }) { AlertForm(state) }
}

@Composable
fun MoveVisitDialog(visit: Visit, onDismiss: () -> Unit, onMove: (date: String, time: String) -> Unit) {
    var date by remember { mutableStateOf(visit.date) }
    var time by remember { mutableStateOf(visit.time) }
    val problem = when {
        runCatching { LocalDate.parse(date.trim()) }.isFailure -> "Date must be YYYY-MM-DD."
        runCatching { LocalTime.parse(time.trim()) }.isFailure -> "Time must be HH:MM."
        else -> null
    }
    FormDialog("Move this visit", "Move", problem, onDismiss, { onMove(date.trim(), time.trim()) }) {
        Text("Only this visit changes; the rest of the series stays the same.", style = HType.small, color = Sage.Muted)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(Modifier.weight(1f)) { SageTextField("New date", date, { date = it }, placeholder = "YYYY-MM-DD") }
            Box(Modifier.weight(1f)) { SageTextField("New time", time, { time = it }, placeholder = "HH:MM") }
        }
    }
}

/** Closing: feedback from anyone who closes; the star rating only when the patient closes (3.2). */
@Composable
fun CloseItemDialog(canRate: Boolean, onDismiss: () -> Unit, onClose: (feedback: String?, rating: Int?) -> Unit) {
    var feedback by remember { mutableStateOf("") }
    var rating by remember { mutableIntStateOf(0) }
    FormDialog("Close this item", "Close item", null, onDismiss, { onClose(feedback.trim().ifEmpty { null }, rating.takeIf { canRate && it > 0 }) }) {
        Text("Future visits are cancelled and alerts stop. You can reopen it later.", style = HType.small, color = Sage.Muted)
        if (canRate) {
            FieldLabel("How was your care? (optional)")
            Row {
                (1..5).forEach { n ->
                    Icon(
                        if (n <= rating) Icons.Filled.Star else Icons.Outlined.StarOutline, null, tint = Sage.Sand,
                        modifier = Modifier.size(40.dp).clickable { rating = if (rating == n) 0 else n }.padding(6.dp)
                            .semantics { contentDescription = "$n star${if (n > 1) "s" else ""}" },
                    )
                }
            }
        }
        SageTextField("Feedback (optional)", feedback, { feedback = it })
    }
}
