package com.healoo.app.ui.admin

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Info
import com.healoo.app.ui.icons.outlined.PersonAdd
import com.healoo.app.ui.icons.outlined.MedicalServices
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import com.healoo.app.ui.upload.SageTextField
import kotlinx.coroutines.launch

/** Designations offered as chips; any other can be typed. There is no department field yet. */
private val DESIGNATIONS = listOf("Cardiologist", "Urologist", "Paediatrician", "General Medicine")

class AdminViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var doctors by mutableStateOf<List<AdminAccount>?>(null); private set
    var error by mutableStateOf<String?>(null); private set
    var notice by mutableStateOf<String?>(null)
    var busy by mutableStateOf(false); private set
    var formError by mutableStateOf<String?>(null)

    init { load() }

    fun load() = viewModelScope.launch {
        error = null
        runCatching { me = repo.me(); doctors = repo.adminDoctors() }
            .onFailure { error = "Couldn't load your hospital's doctors: ${it.message}" }
    }

    fun create(doctor: Boolean, account: NewAccount, onDone: () -> Unit) {
        busy = true; formError = null
        viewModelScope.launch {
            runCatching { if (doctor) repo.adminCreateDoctor(account) else repo.adminCreateUser(account) }
                .onSuccess { a ->
                    notice = "Created ${a.displayName} · Healoo ID ${a.publicId}. They sign in with this ID or with ${a.email}."
                    if (doctor) load()
                    onDone()
                }
                .onFailure { formError = it.message ?: "That didn't work. Try again." }
            busy = false
        }
    }

    fun setActive(d: AdminAccount, active: Boolean) = viewModelScope.launch {
        runCatching { if (active) repo.adminReactivateDoctor(d.id) else repo.adminDeactivateDoctor(d.id) }
            .onSuccess { notice = if (active) "${d.displayName} is active again." else "${d.displayName} was deleted (deactivated)."; load() }
            .onFailure { notice = "That didn't work: ${it.message}" }
    }
}

/** Hospital administrators: add users and doctors, delete (deactivate) doctors. Users can't be deleted. */
@Composable
fun AdminScreen(onBack: () -> Unit, vm: AdminViewModel = viewModel()) {
    var form by remember { mutableStateOf<Boolean?>(null) }        // true = doctor, false = user
    var confirm by remember { mutableStateOf<AdminAccount?>(null) }

    Scaffold(
        containerColor = Sage.Background,
        topBar = { PinnedHeader("Administration", vm.me?.hospital ?: "Users and doctors", onBack) },
    ) { padding ->
        ScrollbarLazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            vm.notice?.let { n ->
                item {
                    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Sage.SageTint).padding(start = 12.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        Text(n, style = HType.caption, color = Sage.Accent, modifier = Modifier.weight(1f))
                        TextButton({ vm.notice = null }) { Text("OK", color = Sage.Accent) }
                    }
                }
            }
            item {
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    AddButton("Add user", Icons.Outlined.PersonAdd, Modifier.weight(1f)) { vm.formError = null; form = false }
                    AddButton("Add doctor", Icons.Outlined.MedicalServices, Modifier.weight(1f)) { vm.formError = null; form = true }
                }
            }
            item {
                Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Sage.SandTint).padding(12.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Icon(Icons.Outlined.Info, null, tint = Sage.SandInk, modifier = Modifier.size(18.dp))
                    Text("Users can't be deleted: they own their health records. Deleting a doctor deactivates the account — " +
                        "they can't sign in and leave the hospital; their past messages and appointments stay.",
                        style = HType.caption, color = Sage.SandInk)
                }
            }
            item {
                val list = vm.doctors
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    SectionHeader("Doctors" + (list?.let { " · ${it.count { d -> d.active }}" } ?: ""))
                    when {
                        vm.error != null -> ErrorBox(vm.error!!, vm::load)
                        list == null -> LoadingBox()
                        else -> FilteredSearchBar(
                            placeholder = "Search doctors by name, designation or Healoo ID",
                            candidates = list, key = { it.id },
                            matches = { d, q -> d.displayName.lowercase().contains(q) || d.headline.lowercase().contains(q) || d.publicId.lowercase().contains(q) },
                            onPick = { confirm = it },
                            filters = listOf(SearchFilter("Active") { it.active }, SearchFilter("Deleted") { !it.active }),
                            showAllWhenBlank = true, maxResults = 200,
                            emptyText = if (list.isEmpty()) "No doctors at this hospital yet. Add one above." else "No doctor matches.",
                        ) { d -> UserResultRow(d.profile, trailing = if (d.active) "Delete" else "Reactivate") }
                    }
                }
            }
        }
    }

    form?.let { doctor -> AccountSheet(doctor, vm, onDismiss = { form = null }) }
    confirm?.let { d ->
        AlertDialog(
            onDismissRequest = { confirm = null }, containerColor = Sage.Surface,
            title = { Text(if (d.active) "Delete ${d.displayName}?" else "Reactivate ${d.displayName}?", style = HType.section) },
            text = {
                Text(if (d.active) "They won't be able to sign in and will leave ${vm.me?.hospital ?: "the hospital"}. " +
                    "Their past messages, appointments and records stay. You can reactivate them later."
                else "They can sign in again and rejoin ${vm.me?.hospital ?: "the hospital"}.", style = HType.body, color = Sage.InkSoft)
            },
            confirmButton = {
                TextButton({ vm.setActive(d, !d.active); confirm = null }) {
                    Text(if (d.active) "Delete" else "Reactivate", color = if (d.active) Sage.Clay else Sage.Accent)
                }
            },
            dismissButton = { TextButton({ confirm = null }) { Text("Cancel", color = Sage.Muted) } },
        )
    }
}

@Composable
private fun AddButton(label: String, icon: androidx.compose.ui.graphics.vector.ImageVector, modifier: Modifier, onClick: () -> Unit) {
    OutlinedCard(onClick = onClick, modifier = modifier.height(64.dp), shape = RoundedCornerShape(14.dp),
        colors = CardDefaults.outlinedCardColors(containerColor = Sage.Surface)) {
        Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null, tint = Sage.Accent, modifier = Modifier.size(20.dp)); Spacer(Modifier.width(8.dp))
            Text(label, style = HType.bodyStrong, color = Sage.Accent)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun AccountSheet(doctor: Boolean, vm: AdminViewModel, onDismiss: () -> Unit) {
    var name by remember { mutableStateOf(if (doctor) "Dr. " else "") }
    var email by remember { mutableStateOf("") }
    var location by remember { mutableStateOf("") }
    var designation by remember { mutableStateOf("") }
    var reg by remember { mutableStateOf("") }
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Sage.Background, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(
            Modifier.fillMaxWidth().verticalScrollWithBar(rememberScrollState()).imePadding().padding(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(if (doctor) "Add a doctor" else "Add a user", style = HType.section, color = Sage.Ink)
            Text(if (doctor) "The doctor joins ${vm.me?.hospital ?: "your hospital"}." else "A user (patient) owns their records; they can't be deleted later.",
                style = HType.caption, color = Sage.Muted)
            SageTextField("Full name", name, { name = it }, placeholder = if (doctor) "e.g. Dr. Asha Verma" else "e.g. Asha Verma")
            SageTextField("Email", email, { email = it.trim() }, placeholder = "Used to link their sign-in")
            if (doctor) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    FieldLabel("Designation")
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        DESIGNATIONS.forEach { d -> SageChip(d, designation == d, { designation = if (designation == d) "" else d }) }
                    }
                    SageTextField("Or type one", designation, { designation = it }, placeholder = "e.g. Dermatologist")
                }
                SageTextField("Registration number (optional)", reg, { reg = it })
            }
            SageTextField("City (optional)", location, { location = it })
            vm.formError?.let { Text(it, style = HType.small, color = Sage.Clay) }
            PrimaryButton(
                if (vm.busy) "Saving…" else if (doctor) "Add doctor" else "Add user",
                {
                    vm.create(doctor, NewAccount(name.trim(), email, location.trim().ifEmpty { null },
                        designation.trim().ifEmpty { null }.takeIf { doctor }, reg.trim().ifEmpty { null }.takeIf { doctor }), onDismiss)
                },
                Modifier.fillMaxWidth(), enabled = !vm.busy && name.trim().length > (if (doctor) 4 else 1) && email.contains('@'),
            )
        }
    }
}

