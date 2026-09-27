package com.healoo.app.ui.profile

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.FileProvider
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.ProfilePhotos
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
import java.io.File

class EditProfileViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var name by mutableStateOf("")
    var location by mutableStateOf("")
    var saving by mutableStateOf(false); private set
    var message by mutableStateOf<String?>(null); private set
    var photoBusy by mutableStateOf(false); private set

    /** Resizes the picked or captured image (512 px JPEG), uploads it and makes it the profile picture. */
    fun changePhoto(context: Context, source: Uri) {
        photoBusy = true; message = null
        viewModelScope.launch {
            runCatching { repo.setProfilePhoto(ProfilePhotos.prepare(context, source)) }
                .onSuccess { me = it; ServiceLocator.auth.signedIn(it) }
                .onFailure { message = "The photo wasn't saved. Check your connection and try again." }
            photoBusy = false
        }
    }

    fun removePhoto() {
        photoBusy = true; message = null
        viewModelScope.launch {
            runCatching { repo.removeProfilePhoto() }
                .onSuccess { me = it; ServiceLocator.auth.signedIn(it) }
                .onFailure { message = "The photo wasn't removed. Try again." }
            photoBusy = false
        }
    }

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
    val context = LocalContext.current
    val pickPhoto = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri -> uri?.let { vm.changePhoto(context, it) } }
    var cameraUri by remember { mutableStateOf<Uri?>(null) }
    val takePhoto = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture()) { ok -> if (ok) cameraUri?.let { vm.changePhoto(context, it) } }
    Scaffold(
        containerColor = Sage.Background,
        topBar = { PinnedHeader("Edit profile", me?.publicId ?: "", onClose, backIcon = Icons.Outlined.Close, backLabel = "Cancel") },
        bottomBar = { BottomActionBar { PrimaryButton(if (vm.saving) "Saving…" else "Save changes", { vm.save(onClose) }, Modifier.weight(1f), enabled = !vm.saving) } },
    ) { padding ->
        if (me == null) { LoadingBox(Modifier.padding(padding)); return@Scaffold }
        ScrollbarLazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            item {
                // Profile picture: shown to everyone who can see this profile.
                Column(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface).padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Avatar(me.initials, 96.dp, photoUrl = me.photoUri)
                    if (vm.photoBusy) Text("Saving photo…", style = HType.small, color = Sage.Muted)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SecondaryButton("Choose photo", { pickPhoto.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) }, enabled = !vm.photoBusy)
                        SecondaryButton("Take photo", {
                            val file = File(context.cacheDir, "captures/profile-${System.currentTimeMillis()}.jpg").apply { parentFile?.mkdirs() }
                            val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
                            cameraUri = uri; takePhoto.launch(uri)
                        }, enabled = !vm.photoBusy)
                    }
                    if (!me.photoUri.isNullOrBlank()) Text("Remove photo", style = HType.bodyStrong, color = Sage.Clay,
                        modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable(enabled = !vm.photoBusy) { vm.removePhoto() }.padding(6.dp))
                }
            }
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
