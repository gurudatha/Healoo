package com.healoo.app.ui.vitals

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class VitalAlertsViewModel(initial: Severity) : ViewModel() {
    private val repo = ServiceLocator.repository
    var severity by mutableStateOf(initial); private set
    var sort by mutableStateOf(AlertSort.TIME); private set
    var counts by mutableStateOf(VitalAlertCounts()); private set
    var alerts by mutableStateOf<List<VitalAlert>?>(null); private set
    var error by mutableStateOf<String?>(null); private set

    init {
        load()
        // A patient just recorded something: the list fills in by itself.
        viewModelScope.launch { repo.events.collect { if (it is RealtimeEvent.VitalAlert) load() } }
    }

    fun load() = viewModelScope.launch {
        error = null
        runCatching { repo.vitalAlerts(severity, sort) }
            .onSuccess { counts = it.counts; alerts = it.data }
            .onFailure { error = "Couldn't load alerts: ${it.message}" }
    }

    fun show(s: Severity) { severity = s; alerts = null; load() }
    fun sortBy(s: AlertSort) { sort = s; load() }

    /** Removes the alert from the active list at once; the server is told in the background. */
    fun dismiss(a: VitalAlert) {
        alerts = alerts?.filterNot { it.id == a.id }
        counts = when (a.severity) {
            Severity.RED -> counts.copy(red = (counts.red - 1).coerceAtLeast(0))
            Severity.YELLOW -> counts.copy(yellow = (counts.yellow - 1).coerceAtLeast(0))
            Severity.GREEN -> counts.copy(green = (counts.green - 1).coerceAtLeast(0))
        }
        viewModelScope.launch { runCatching { repo.readVitalAlert(a.id) }.onFailure { load() } }
    }
}

/**
 * A doctor's unread alerts of one colour. Swipe an alert left to remove it from the active list,
 * right to open the patient's profile; the tick and a tap do the same without swiping.
 */
@Composable
fun VitalAlertsScreen(initial: Severity, onBack: () -> Unit, onOpenUser: (String) -> Unit) {
    val vm: VitalAlertsViewModel = viewModel(key = "alerts-$initial") { VitalAlertsViewModel(initial) }
    val list = vm.alerts

    Scaffold(containerColor = Sage.Background, topBar = { PinnedHeader("Patient alerts", "${vm.severity.label} · unread", onBack) }) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
            Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Severity.entries.forEach { s -> SageChip("${s.label} · ${vm.counts[s]}", vm.severity == s, { vm.show(s) }) }
                }
                Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Sort", style = HType.small, color = Sage.Muted)
                    AlertSort.entries.forEach { s -> SageChip(s.label, vm.sort == s, { vm.sortBy(s) }, compact = true) }
                }
                Text("Swipe left to remove from the list · swipe right for the patient's profile", style = HType.small, color = Sage.Muted)
            }
            ScrollbarLazyColumn(contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                when {
                    vm.error != null -> item { ErrorBox(vm.error!!, vm::load) }
                    list == null -> item { LoadingBox() }
                    list.isEmpty() -> item {
                        Text("No unread ${vm.severity.label.lowercase()} alerts.", style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 16.dp))
                    }
                    else -> items(list, key = { it.id }) { a ->
                        AlertRow(a, onDismiss = { vm.dismiss(a) }, onOpenUser = { onOpenUser(a.patientId) }, modifier = Modifier.animateItem())
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AlertRow(a: VitalAlert, onDismiss: () -> Unit, onOpenUser: () -> Unit, modifier: Modifier = Modifier) {
    val state = rememberSwipeToDismissBoxState(confirmValueChange = { target ->
        when (target) {
            SwipeToDismissBoxValue.EndToStart -> { onDismiss(); true }          // swipe left: read
            SwipeToDismissBoxValue.StartToEnd -> { onOpenUser(); false }        // swipe right: profile, alert stays
            SwipeToDismissBoxValue.Settled -> false
        }
    })
    SwipeToDismissBox(
        state = state, modifier = modifier,
        backgroundContent = {
            val toProfile = state.dismissDirection == SwipeToDismissBoxValue.StartToEnd
            Box(
                Modifier.fillMaxSize().clip(RoundedCornerShape(Radius.card)).background(if (toProfile) Sage.SageTint else Sage.Sunken).padding(horizontal = 16.dp),
                contentAlignment = if (toProfile) Alignment.CenterStart else Alignment.CenterEnd,
            ) { Text(if (toProfile) "Profile" else "Remove", style = HType.bodyStrong, color = if (toProfile) Sage.Accent else Sage.Muted) }
        },
    ) {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface).clickable(onClick = onOpenUser)
                .padding(start = 12.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).background(severityTint(a.severity)), contentAlignment = Alignment.Center) {
                Icon(vitalIcon(a.icon), contentDescription = a.severity.label, tint = severityColor(a.severity), modifier = Modifier.size(20.dp))
            }
            Column(Modifier.weight(1f)) {
                Text(a.patientName, style = HType.bodyStrong, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text("${a.label} · ${formatVital(a.values, a.unit)}", style = HType.caption, color = severityColor(a.severity), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(vitalTime(a.takenAt), style = HType.small, color = Sage.Muted)
            }
            IconButton(onDismiss) { Icon(Icons.Outlined.Check, contentDescription = "Remove ${a.patientName}'s ${a.label} alert from the list", tint = Sage.Muted) }
        }
    }
}
