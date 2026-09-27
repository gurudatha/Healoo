package com.healoo.app.ui.landing

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.search.RoleFilterRow
import com.healoo.app.ui.search.SearchField
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.LocalTime
import java.time.format.DateTimeFormatter

class LandingViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var dashboard by mutableStateOf<Dashboard?>(null); private set
    var items by mutableStateOf<List<DataItem>?>(null); private set
    var error by mutableStateOf<String?>(null); private set
    /** Dashboard filter: a PartKind (REPORT, MESSAGE or APPOINTMENT), or null for all open items. */
    var filter by mutableStateOf<String?>(null); private set
    /** "See all": every open item instead of the latest [PREVIEW]. */
    var showAll by mutableStateOf(false); private set

    init {
        load()
        // Keep counters and the list current when new items or messages arrive live.
        viewModelScope.launch { repo.events.collect { if (it !is RealtimeEvent.ConnectionChanged) load() } }
    }

    /** Tapping the selected counter again clears the filter. */
    fun toggleFilter(kind: String) { filter = if (filter == kind) null else kind; items = null; load() }
    fun toggleShowAll() { showAll = !showAll; load() }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            me = repo.me()
            dashboard = repo.dashboard()
            items = repo.openItems(limit = if (showAll) ALL else PREVIEW, kind = filter)
        }.onFailure { error = "Couldn't load your items. Check your connection and try again." }
    }

    companion object { const val PREVIEW = 20; const val ALL = 100 }
}

@Composable
fun LandingScreen(
    onOpenItem: (String) -> Unit,
    onSearch: (query: String, role: Role?) -> Unit,
    onOpenProfile: () -> Unit,
    onTab: (Tab) -> Unit,
    vm: LandingViewModel = viewModel(),
) {
    var query by remember { mutableStateOf("") }
    var role by remember { mutableStateOf<Role?>(Role.DOCTOR) }

    Scaffold(containerColor = Sage.Background, bottomBar = { HealooBottomBar(Tab.HOME, onTab) }) { padding ->
        Column(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            DashboardHeader(vm.me, vm.dashboard, vm.filter, vm::toggleFilter, onOpenProfile)
            ScrollbarLazyColumn(
                contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        FieldLabel("Find doctors, patients, hospitals or labs")
                        SearchField(query, { query = it }, placeholder = "Search by name or Healoo ID", onSubmit = { onSearch(query, role) }, compact = true)
                        RoleFilterRow(selected = role, includeAll = true, compact = true, onSelect = { role = it; onSearch(query, it) })
                    }
                }
                item {
                    // A fine line between Find and Open items, drawn inside the gap that was already there.
                    HorizontalDivider(Modifier.padding(vertical = 2.5.dp), thickness = 1.dp, color = Sage.Border)
                    // "See all" expands the list in place (it used to open Search, which lists people, not items).
                    val more = vm.showAll || (vm.items?.size ?: 0) >= LandingViewModel.PREVIEW
                    SectionHeader(filterTitle(vm.filter), if (!more) null else if (vm.showAll) "Show fewer" else "See all", vm::toggleShowAll)
                    vm.filter?.let { f ->
                        Text("Clear filter", style = HType.small, color = Sage.Accent,
                            modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable { vm.toggleFilter(f) }.padding(vertical = 4.dp))
                    }
                }
                when {
                    vm.error != null -> item { ErrorBox(vm.error!!, vm::load) }
                    vm.items == null -> item { LoadingBox() }
                    vm.items!!.isEmpty() -> item {
                        Text(if (vm.filter == null) "Nothing open right now. New reports, messages and bookings will appear here."
                            else "No open items with ${filterNoun(vm.filter)}.",
                            style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 16.dp))
                    }
                    else -> items(vm.items!!, key = { it.id }) { DataItemRow(it, onClick = { onOpenItem(it.id) }) }
                }
            }
        }
    }
}

@Composable
private fun DashboardHeader(me: UserProfile?, d: Dashboard?, filter: String?, onFilter: (String) -> Unit, onOpenProfile: () -> Unit) {
    val greeting = when (LocalTime.now().hour) { in 0..11 -> "Good morning"; in 12..16 -> "Good afternoon"; else -> "Good evening" }
    Column(
        Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(bottomStart = Radius.header, bottomEnd = Radius.header))
            .background(Sage.Primary).statusBarsPadding().height(screenFraction(HeaderRatio.LANDING))
            .padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 16.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(LocalDate.now().format(DateTimeFormatter.ofPattern("EEEE, d MMM")), style = HType.caption, color = Sage.OnPrimarySoft)
                Text("$greeting, ${me?.displayName?.substringBefore(' ') ?: ""}", style = HType.greeting, color = Sage.OnPrimary,
                    maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            androidx.compose.foundation.layout.Box(Modifier.clip(RoundedCornerShape(22.dp))) {
                androidx.compose.material3.Surface(onClick = onOpenProfile, color = Color.Transparent) {
                    Avatar(me?.initials ?: "", size = 44.dp, border = Sage.OnPrimaryLine, photoUrl = me?.photoUri)
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Counter(d?.openReports, "Open reports", filter == PartKind.REPORT, Modifier.weight(1f)) { onFilter(PartKind.REPORT) }
            Counter(d?.unreadMessages, "Unread messages", filter == PartKind.MESSAGE, Modifier.weight(1f)) { onFilter(PartKind.MESSAGE) }
            Counter(d?.upcomingAppointments, "Appointments", filter == PartKind.APPOINTMENT, Modifier.weight(1f)) { onFilter(PartKind.APPOINTMENT) }
        }
    }
}

@Composable
private fun Counter(value: Int?, label: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    // The selected tile turns light, so it's clear which filter the list below uses.
    Column(
        modifier.clip(RoundedCornerShape(14.dp))
            .background(if (selected) Color.White else Sage.PrimaryRaised)
            .selectable(selected = selected, role = androidx.compose.ui.semantics.Role.Tab, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(value?.toString() ?: "–", style = HType.counter, color = if (selected) Sage.Accent else Sage.OnPrimary)
        Text(label, style = HType.small, color = if (selected) Sage.Ink else Sage.OnPrimarySoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

private fun filterTitle(kind: String?) = when (kind) {
    PartKind.REPORT -> "Open items · Reports"
    PartKind.MESSAGE -> "Open items · Messages"
    PartKind.APPOINTMENT -> "Open items · Appointments"
    else -> "Open items"
}

private fun filterNoun(kind: String?) = when (kind) {
    PartKind.REPORT -> "reports"
    PartKind.MESSAGE -> "messages"
    else -> "appointments"
}
