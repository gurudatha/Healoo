package com.healoo.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.ProfileUpdate
import com.healoo.app.data.Role
import com.healoo.app.data.ServiceLocator
import com.healoo.app.data.UserProfile
import com.healoo.app.ui.components.*
import com.healoo.app.ui.scan.QrImage
import com.healoo.app.ui.scan.healooQrContent
import com.healoo.app.ui.theme.*
import com.healoo.app.ui.upload.SageTextField
import kotlinx.coroutines.launch

class EditProfileViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var name by mutableStateOf("")
    var location by mutableStateOf("")
    var saving by mutableStateOf(false); private set
    var message by mutableStateOf<String?>(null); private set

    init {
        viewModelScope.launch {
            runCatching { repo.me() }.onSuccess { me = it; name = it.displayName; location = it.location.orEmpty() }
        }
    }

    fun save(onDone: () -> Unit) {
        if (name.isBlank()) { message = "Enter the name people should see."; return }
        saving = true
        viewModelScope.launch {
            runCatching { repo.updateProfile(ProfileUpdate(name.trim(), location.trim().ifEmpty { null })) }
                .onSuccess { ServiceLocator.auth.signedIn(it); onDone() }
                .onFailure { message = "Your changes weren't saved. Check your connection and try again." }
            saving = false
        }
    }
}

@Composable
fun EditProfileScreen(onClose: () -> Unit, vm: EditProfileViewModel = viewModel()) {
    val me = vm.me
    Scaffold(
        containerColor = Sage.Background,
        topBar = { PinnedHeader("Edit profile", me?.publicId ?: "", onClose, backIcon = Icons.Outlined.Close, backLabel = "Cancel") },
        bottomBar = { BottomActionBar { PrimaryButton(if (vm.saving) "Saving…" else "Save changes", { vm.save(onClose) }, Modifier.weight(1f), enabled = !vm.saving) } },
    ) { padding ->
        if (me == null) { LoadingBox(Modifier.padding(padding)); return@Scaffold }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            item {
                Column(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface).padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    QrImage(healooQrContent(me.publicId), 180.dp, "QR code for your Healoo ID ${me.publicId}")
                    Text(me.publicId, style = HType.section, color = Sage.Ink)
                    Text("Others can scan this from Search to find you and add you to their contacts. It shows your profile only, never your records.",
                        style = HType.caption, color = Sage.Muted)
                }
            }
            item { SageTextField("Name shown to others", vm.name, { vm.name = it }) }
            item { SageTextField("Location", vm.location, { vm.location = it }, placeholder = "City or area") }
            if (me.primaryRole == Role.DOCTOR) item {
                GroupCard {
                    ReadOnlyRow("Registration number", me.officialNumber ?: "Not set")
                    RowDivider()
                    ReadOnlyRow("Current hospital", me.hospital ?: "Independent")
                }
                Text("Your hospital administrator updates these.", style = HType.small, color = Sage.Muted, modifier = Modifier.padding(top = 6.dp))
            }
            vm.message?.let { item { Text(it, style = HType.small, color = Sage.Clay) } }
        }
    }
}

@Composable
private fun ReadOnlyRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = HType.body, color = Sage.InkSoft, modifier = Modifier.weight(1f))
        Text(value, style = HType.bodyStrong, color = Sage.Ink)
    }
}
