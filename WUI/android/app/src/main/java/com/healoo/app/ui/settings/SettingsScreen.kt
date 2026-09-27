package com.healoo.app.ui.settings

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import com.healoo.app.ui.icons.automirrored.outlined.Logout
import com.healoo.app.ui.icons.outlined.AdminPanelSettings
import com.healoo.app.ui.icons.outlined.Language
import com.healoo.app.ui.icons.outlined.People
import com.healoo.app.ui.icons.outlined.Shield
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.NotificationPrefs
import com.healoo.app.data.ServiceLocator
import com.healoo.app.data.UserProfile
import com.healoo.app.data.isAdministrator
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class SettingsViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var prefs by mutableStateOf<NotificationPrefs?>(null); private set
    var saveError by mutableStateOf<String?>(null); private set

    init { refresh() }

    fun refresh() = viewModelScope.launch {
        runCatching { me = repo.me() }
        runCatching { prefs = repo.notificationPrefs() }
    }

    /** Optimistic save to PUT /v1/me/notification-prefs; reverts if the server refuses. */
    fun update(change: (NotificationPrefs) -> NotificationPrefs) {
        val before = prefs ?: return
        val after = change(before)
        prefs = after; saveError = null
        viewModelScope.launch {
            runCatching { repo.saveNotificationPrefs(after) }
                .onSuccess { prefs = it }
                .onFailure { prefs = before; saveError = "Couldn't save that setting. Try again." }
        }
    }
}

@Composable
fun SettingsScreen(
    onTab: (Tab) -> Unit,
    onEditProfile: () -> Unit,
    onActiveSharing: () -> Unit,
    onContacts: () -> Unit,
    onLogout: () -> Unit,
    onAdministration: () -> Unit = {},
    vm: SettingsViewModel = viewModel(),
) {
    Scaffold(containerColor = Sage.Background, bottomBar = { HealooBottomBar(Tab.SETTINGS, onTab) }) { padding ->
        Column(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            Column(
                Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(bottomStart = Radius.header, bottomEnd = Radius.header))
                    .background(Sage.Primary).statusBarsPadding()
                    .padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Text("Settings", style = HType.screenTitle, color = Color.White)
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(Sage.PrimaryRaised).padding(12.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Avatar(vm.me?.initials ?: "", 52.dp)
                    Column(Modifier.weight(1f)) {
                        Text(vm.me?.displayName ?: "", style = HType.bodyStrong.copy(fontSize = 17.sp), color = Color.White)
                        Text("${vm.me?.headline ?: ""} · ${vm.me?.publicId ?: ""}", style = HType.caption, color = Sage.OnPrimarySoft)
                    }
                    Surface(onClick = onEditProfile, shape = RoundedCornerShape(20.dp), color = Color.White) {
                        Text("Edit", style = HType.caption.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = Sage.Primary,
                            modifier = Modifier.padding(horizontal = 14.dp, vertical = 11.dp))
                    }
                }
            }

            ScrollbarLazyColumn(contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        FieldLabel("Notifications")
                        val p = vm.prefs
                        if (p == null) LoadingBox() else GroupCard {
                            SwitchRow("Messages & alerts", "Push when the app is closed", p.pushMessages) { v -> vm.update { it.copy(pushMessages = v) } }
                            RowDivider()
                            SwitchRow("New reports", "When a lab uploads for you", p.pushReports) { v -> vm.update { it.copy(pushReports = v) } }
                            RowDivider()
                            SwitchRow("Quiet hours", "${p.quietStart} – ${p.quietEnd}, no sound", p.quietHours) { v -> vm.update { it.copy(quietHours = v) } }
                        }
                        vm.saveError?.let { Text(it, style = HType.small, color = Sage.Clay) }
                    }
                }
                if (vm.me?.isAdministrator == true) item {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        FieldLabel("Administration")
                        GroupCard { LinkRow(Icons.Outlined.AdminPanelSettings, "Users and doctors", vm.me?.hospital ?: "", onAdministration) }
                    }
                }
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        FieldLabel("Privacy & sharing")
                        GroupCard {
                            LinkRow(Icons.Outlined.Shield, "Active sharing", "", onActiveSharing)
                            RowDivider()
                            LinkRow(Icons.Outlined.People, "Contacts", "", onContacts)
                            RowDivider()
                            LinkRow(Icons.Outlined.Language, "Language", "English") {}
                        }
                    }
                }
                item {
                    OutlinedButton(
                        onClick = onLogout, modifier = Modifier.fillMaxWidth().height(50.dp), shape = RoundedCornerShape(Radius.card),
                        border = BorderStroke(1.dp, Sage.ClayBorder), colors = ButtonDefaults.outlinedButtonColors(containerColor = Sage.Surface, contentColor = Sage.Clay),
                    ) {
                        Icon(Icons.AutoMirrored.Outlined.Logout, null, modifier = Modifier.size(18.dp)); Spacer(Modifier.width(8.dp))
                        Text("Log out", style = HType.bodyStrong)
                    }
                }
            }
        }
    }
}

@Composable
private fun SwitchRow(label: String, note: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(role = Role.Switch) { onChange(!checked) }.padding(start = 14.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = HType.body.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Medium), color = Sage.Ink)
            Text(note, style = HType.small, color = Sage.Muted)
        }
        Switch(
            checked = checked, onCheckedChange = null,
            colors = SwitchDefaults.colors(checkedTrackColor = Sage.Primary, uncheckedTrackColor = Sage.SwitchOff,
                uncheckedThumbColor = Color.White, uncheckedBorderColor = Sage.SwitchOff),
        )
    }
}

@Composable
private fun LinkRow(icon: ImageVector, label: String, value: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 52.dp).clickable(onClick = onClick).padding(start = 14.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(32.dp).clip(RoundedCornerShape(10.dp)).background(Sage.SageTint), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = Sage.Primary, modifier = Modifier.size(17.dp))
        }
        Text(label, style = HType.body.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Medium), color = Sage.Ink, modifier = Modifier.weight(1f))
        if (value.isNotEmpty()) Text(value, style = HType.caption, color = Sage.Muted)
        Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, null, tint = Sage.Muted)
    }
}
