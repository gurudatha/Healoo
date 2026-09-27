package com.healoo.app.ui.search

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Check
import com.healoo.app.ui.icons.outlined.QrCodeScanner
import androidx.compose.material.icons.outlined.Search
import com.healoo.app.ui.icons.outlined.Shield
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.Role
import com.healoo.app.data.ServiceLocator
import com.healoo.app.data.UserProfile
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class SearchViewModel(initialQuery: String, initialRole: Role?) : ViewModel() {
    private val repo = ServiceLocator.repository
    var query by mutableStateOf(initialQuery); private set
    var role by mutableStateOf(initialRole); private set
    var results by mutableStateOf<List<UserProfile>?>(null); private set
    var adding by mutableStateOf<Set<String>>(emptySet()); private set
    var error by mutableStateOf<String?>(null); private set
    private var job: Job? = null

    init { run(debounce = false) }

    fun onQuery(q: String) { query = q; run() }
    fun onRole(r: Role?) { role = r; run(debounce = false) }

    fun run(debounce: Boolean = true) {
        job?.cancel()
        job = viewModelScope.launch {
            if (debounce) delay(300)
            error = null
            runCatching { results = repo.search(query, role) }
                .onFailure { error = "Search isn't available right now. Try again in a moment." }
        }
    }

    fun connect(user: UserProfile) = viewModelScope.launch {
        adding = adding + user.id
        runCatching { repo.connect(user.id) }.onSuccess { updated ->
            results = results?.map { if (it.id == updated.id) updated else it }
        }
        adding = adding - user.id
    }
}

private val roleFilters = listOf(null to "All", Role.DOCTOR to "Doctors", Role.PATIENT to "Users", Role.HOSPITAL to "Hospitals", Role.LAB to "Labs")

@Composable
fun RoleFilterRow(selected: Role?, onSelect: (Role?) -> Unit, includeAll: Boolean = true, onHeader: Boolean = false, compact: Boolean = false) {
    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(if (compact) 6.dp else 8.dp)) {
        roleFilters.filter { includeAll || it.first != null }.forEach { (r, label) ->
            SageChip(label, selected == r, onClick = { onSelect(r) }, onHeader = onHeader, compact = compact)
        }
    }
}

/**
 * [compact]: 40 dp high instead of Material's 56 dp (30% smaller), for the landing page. Material's
 * text field can't go below 56 dp, so the compact one is drawn here with the same look.
 */
@Composable
fun SearchField(
    value: String,
    onChange: (String) -> Unit,
    placeholder: String,
    onSubmit: () -> Unit = {},
    compact: Boolean = false,
    trailing: @Composable (() -> Unit)? = null,
) {
    if (compact) {
        var focused by remember { mutableStateOf(false) }
        Row(
            Modifier.fillMaxWidth().height(40.dp).clip(RoundedCornerShape(Radius.field)).background(Sage.Surface)
                .border(1.dp, if (focused) Sage.Accent else Sage.Border, RoundedCornerShape(Radius.field))
                .padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(Icons.Outlined.Search, contentDescription = null, tint = Sage.Muted, modifier = Modifier.size(18.dp))
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (value.isEmpty()) Text(placeholder, style = HType.caption, color = Sage.Placeholder, maxLines = 1)
                androidx.compose.foundation.text.BasicTextField(
                    value = value, onValueChange = onChange, singleLine = true,
                    textStyle = HType.caption.copy(color = Sage.Ink),
                    cursorBrush = androidx.compose.ui.graphics.SolidColor(Sage.Accent),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
                    modifier = Modifier.fillMaxWidth().onFocusChanged { focused = it.isFocused }
                        .semantics { contentDescription = placeholder },
                )
            }
            trailing?.invoke()
        }
        return
    }
    OutlinedTextField(
        value = value, onValueChange = onChange, singleLine = true,
        placeholder = { Text(placeholder, style = HType.body, color = Sage.Placeholder) },
        leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null, tint = Sage.Muted) },
        trailingIcon = trailing,
        textStyle = HType.body.copy(color = Sage.Ink),
        shape = RoundedCornerShape(Radius.field),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
        colors = OutlinedTextFieldDefaults.colors(
            focusedContainerColor = Sage.Surface, unfocusedContainerColor = Sage.Surface,
            focusedBorderColor = Sage.Accent, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Accent,
        ),
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
fun SearchScreen(
    initialQuery: String,
    initialRole: Role?,
    onOpenUser: (String) -> Unit,
    onTab: (Tab) -> Unit,
) {
    val vm: SearchViewModel = viewModel { SearchViewModel(initialQuery, initialRole) }
    val context = androidx.compose.ui.platform.LocalContext.current
    val scope = rememberCoroutineScope()
    var scanMessage by remember { mutableStateOf<String?>(null) }
    val onScanId: () -> Unit = {
        scope.launch {
            scanMessage = null
            when (val r = com.healoo.app.ui.scan.scanHealooId(context)) {
                is com.healoo.app.ui.scan.ScanResult.Found -> {
                    val match = runCatching { ServiceLocator.repository.search(r.publicId, null) }.getOrNull()
                        ?.firstOrNull { it.publicId.equals(r.publicId, ignoreCase = true) }
                    if (match != null) onOpenUser(match.id) else { vm.onQuery(r.publicId); scanMessage = "No one has the Healoo ID ${r.publicId}." }
                }
                com.healoo.app.ui.scan.ScanResult.NotHealoo -> scanMessage = "That QR code isn't a Healoo ID."
                is com.healoo.app.ui.scan.ScanResult.Failed -> scanMessage = "The scanner couldn't start (${r.reason}). Type the ID instead."
                com.healoo.app.ui.scan.ScanResult.Cancelled -> {}
            }
        }
    }

    Scaffold(containerColor = Sage.Background, bottomBar = { HealooBottomBar(Tab.SEARCH, onTab) }) { padding ->
        Column(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            Column(
                Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(bottomStart = Radius.header, bottomEnd = Radius.header))
                    .background(Sage.Primary).statusBarsPadding()
                    .padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text("Search", style = HType.screenTitle, color = Sage.OnPrimary)
                SearchField(vm.query, vm::onQuery, placeholder = "Healoo ID or name", onSubmit = { vm.run(false) }) {
                    IconButton(onClick = onScanId) {
                        Icon(Icons.Outlined.QrCodeScanner, contentDescription = "Scan Healoo ID QR code", tint = Sage.Accent)
                    }
                }
                RoleFilterRow(vm.role, vm::onRole, onHeader = true)
                scanMessage?.let { Text(it, style = HType.small, color = Sage.OnPrimarySoft) }
            }

            ScrollbarLazyColumn(
                contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                item {
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Sage.SandTint).padding(12.dp),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Icon(Icons.Outlined.Shield, contentDescription = null, tint = Sage.SandInk, modifier = Modifier.size(18.dp))
                        Text("You see profiles only. Health data opens after you connect and the owner shares it.",
                            style = HType.caption, color = Sage.SandInk)
                    }
                }
                val list = vm.results
                when {
                    vm.error != null -> item { ErrorBox(vm.error!!) { vm.run(false) } }
                    list == null -> item { LoadingBox() }
                    list.isEmpty() -> item {
                        Text("No one matches \"${vm.query}\". Check the Healoo ID, or try part of the name.",
                            style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 12.dp))
                    }
                    else -> {
                        item { SectionHeader("Results") }
                        items(list, key = { it.id }) { user ->
                            ResultRow(user, adding = user.id in vm.adding, onOpen = { onOpenUser(user.id) }, onAdd = { vm.connect(user) })
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ResultRow(user: UserProfile, adding: Boolean, onOpen: () -> Unit, onAdd: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface)
            .clickable(onClick = onOpen).padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        val tint = when (user.primaryRole) { Role.HOSPITAL, Role.PATIENT -> Sage.SandTint; Role.LAB -> Sage.ClayTint; else -> Sage.SageTint }
        Avatar(user.initials, 44.dp, background = tint, photoUrl = user.photoUri)
        Column(Modifier.weight(1f)) {
            Text(user.displayName, style = HType.bodyStrong, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(listOfNotNull(user.headline, user.hospital?.takeIf { user.primaryRole == Role.DOCTOR }, user.publicId).joinToString(" · "),
                style = HType.small, color = Sage.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (user.connected) {
            Row(
                Modifier.clip(RoundedCornerShape(16.dp)).background(Sage.SageTint).padding(horizontal = 10.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Icon(Icons.Outlined.Check, contentDescription = null, tint = Sage.Accent, modifier = Modifier.size(14.dp))
                Text("Connected", style = HType.small.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = Sage.Accent)
            }
        } else {
            OutlinedButton(
                onClick = onAdd, enabled = !adding, shape = RoundedCornerShape(20.dp),
                border = BorderStroke(1.dp, Sage.Accent), contentPadding = PaddingValues(horizontal = 14.dp),
                modifier = Modifier.height(40.dp),
            ) {
                Icon(Icons.Outlined.Add, contentDescription = null, tint = Sage.Accent, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(4.dp))
                Text(if (adding) "Adding…" else "Add", style = HType.caption, color = Sage.Accent)
            }
        }
    }
}
