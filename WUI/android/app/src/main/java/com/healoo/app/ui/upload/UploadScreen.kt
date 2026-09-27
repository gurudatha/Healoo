package com.healoo.app.ui.upload

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import com.healoo.app.ui.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role as SemRole
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import coil.compose.AsyncImage
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch
import java.io.File

/** What the New item screen creates (DataItem v2: one primary part), or adding files to an item. */
enum class NewItemMode(val label: String) { REPORT("Report"), APPOINTMENT("Appointment"), ALERT("Alert"), ADD_FILES("Files") }

/**
 * [toUserId]: the person whose page this was opened from. It always receives the item: a patient
 * becomes its owner when a doctor, assistant, lab or hospital uploads for them; anyone else is
 * shared with. [doctorId]/[hospitalId] preselect an appointment's doctor (Book on a person's page).
 */
class UploadViewModel(
    private val toUserId: String?,
    private val addToItemId: String?,
    startMode: NewItemMode?,
    private val doctorId: String?,
    private val hospitalId: String?,
) : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var owner by mutableStateOf<UserProfile?>(null)
    /** Patients a doctor/assistant/lab/hospital can create items for (connected patients). */
    var patients by mutableStateOf<List<UserProfile>>(emptyList()); private set
    var contacts by mutableStateOf<List<UserProfile>>(emptyList()); private set
    var doctors by mutableStateOf<List<UserProfile>>(emptyList()); private set
    /** The page's person when they receive the item but don't own it (always shared with). */
    var fixedRecipient by mutableStateOf<UserProfile?>(null); private set
    /** The page's person is the patient this is uploaded for, so the patient can't be changed. */
    var ownerLocked by mutableStateOf(false); private set
    val extraRecipients = mutableStateListOf<UserProfile>()
    /** The item files are added to (ADD_FILES mode from an item). */
    var target by mutableStateOf<DataItem?>(null); private set
    val uploadingForSomeoneElse get() = owner != null && owner?.id != me?.id

    var mode by mutableStateOf(startMode ?: if (addToItemId != null) NewItemMode.ADD_FILES else NewItemMode.REPORT)
    /** Report mode: false = a new item, true = add the files to an existing one. */
    var intoExisting by mutableStateOf(false)
    var existingTarget by mutableStateOf<DataItem?>(null)
    var existingCandidates by mutableStateOf<List<DataItem>>(emptyList()); private set
    val addingFiles get() = mode == NewItemMode.ADD_FILES || (mode == NewItemMode.REPORT && intoExisting)

    var title by mutableStateOf("")
    var keywords by mutableStateOf("")
    var filesAreReport by mutableStateOf(true)
    val links = mutableStateListOf<String>()
    val files = mutableStateListOf<PendingAttachment>()
    val appointment = com.healoo.app.ui.item.AppointmentFormState(null)
    val alert = com.healoo.app.ui.item.AlertFormState()

    var message by mutableStateOf<String?>(null)
    var busy by mutableStateOf(false); private set

    init {
        viewModelScope.launch {
            runCatching {
                val self = repo.me()
                me = self
                val all = repo.connections()
                contacts = all
                patients = all.filter { it.primaryRole == Role.PATIENT }
                val to = toUserId?.let { repo.user(it) }
                ownerLocked = to != null && to.primaryRole == Role.PATIENT && self.isClinical
                owner = when {
                    ownerLocked -> to
                    self.isClinical -> null            // must pick a patient first
                    else -> self
                }
                fixedRecipient = to?.takeIf { !ownerLocked }
                val preselected = doctorId?.let { id -> if (id == to?.id) to else repo.user(id) }
                doctors = (all + self + listOfNotNull(preselected)).filter { it.primaryRole == Role.DOCTOR }.distinctBy { it.id }
                appointment.doctorId = when {
                    preselected != null -> preselected.id
                    self.primaryRole == Role.DOCTOR -> self.id
                    else -> doctors.singleOrNull()?.id
                }
                addToItemId?.let { target = repo.item(it); filesAreReport = target?.primaryKind == PrimaryKind.REPORT }
                // Existing items to add to: those shared with the page's person, otherwise all open ones.
                existingCandidates = (if (to != null) repo.sharedItems(to.id) else repo.items(ItemStatus.OPEN, 100))
                    .filter { it.status == ItemStatus.OPEN }
            }
        }
    }

    private fun keywordList() = keywords.split(',').map { it.trim() }.filter { it.isNotEmpty() }
    /** The page's person plus anyone added. Creating for a patient, the uploader can add doctors only. */
    private fun shares() = (listOfNotNull(fixedRecipient) + extraRecipients).map { it.id }.distinct()

    fun submit(onDone: (DataItem) -> Unit) {
        val intoItem = if (mode == NewItemMode.ADD_FILES) addToItemId else if (addingFiles) existingTarget?.id else null
        if (addingFiles && intoItem == null) { message = "Choose the item to add the files to."; return }
        val o = owner ?: if (addingFiles) me else null
        if (o == null && mode != NewItemMode.ALERT) { message = "Choose the patient this is for."; return }
        when {
            addingFiles -> if (files.isEmpty()) { message = "Add at least one image or PDF."; return }
            mode == NewItemMode.REPORT -> if (files.isEmpty() && links.isEmpty()) { message = "Add at least one image, PDF or link."; return }
            mode == NewItemMode.APPOINTMENT -> appointment.problem()?.let { message = it; return }
            mode == NewItemMode.ALERT -> alert.problem()?.let { message = it; return }
        }
        busy = true
        viewModelScope.launch {
            runCatching {
                when {
                    addingFiles -> repo.addAttachments(intoItem!!, files.toList(), filesAreReport)
                    mode == NewItemMode.REPORT -> repo.createReport(ReportDraft(o!!.id, title.trim(), keywordList(), links.toList(), shares()), files.toList())
                    mode == NewItemMode.APPOINTMENT -> repo.createAppointment(o!!.id, appointment.build(o.id).copy(hospitalId = hospitalId), shares())
                    else -> repo.createAlert(title.trim(), alert.build(), shares())
                }
            }.onSuccess(onDone).onFailure { message = "That didn't finish: ${it.message}. Nothing was lost — try again." }
            busy = false
        }
    }

    /** Adds picked files, enforcing doc 4.5 limits; images and PDFs may be mixed freely. */
    fun addFiles(context: Context, uris: List<Uri>, forcedKind: AttachmentKind? = null) {
        val rejected = mutableListOf<String>()
        for (uri in uris) {
            if (files.size >= Limits.MAX_ATTACHMENTS) { rejected += "only ${Limits.MAX_ATTACHMENTS} files per item"; break }
            val mime = context.contentResolver.getType(uri) ?: if (forcedKind == AttachmentKind.PDF) "application/pdf" else "image/jpeg"
            val kind = forcedKind ?: when {
                mime == "application/pdf" -> AttachmentKind.PDF
                mime.startsWith("image/") -> AttachmentKind.IMAGE
                else -> { rejected += "${uri.lastPathSegment} isn't an image or PDF"; continue }
            }
            val (name, size) = AttachmentFiles.describe(context, uri)
            val max = if (kind == AttachmentKind.PDF) Limits.PDF_MAX_BYTES else Limits.IMAGE_MAX_BYTES
            if (size > max) { rejected += "$name is over ${max / (1024 * 1024)} MB"; continue }
            if (files.none { it.localUri == uri }) files += PendingAttachment(uri, kind, name, mime, size)
        }
        message = rejected.takeIf { it.isNotEmpty() }?.joinToString("; ")?.let { "Some files weren't added: $it." }
    }

    fun move(index: Int, by: Int) {
        val to = index + by
        if (to in files.indices) files.add(to, files.removeAt(index))
    }

}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun UploadScreen(
    toUserId: String?,
    addToItemId: String?,
    startMode: NewItemMode?,
    doctorId: String?,
    hospitalId: String?,
    onClose: () -> Unit,
    onUploaded: (String) -> Unit,
) {
    val vm: UploadViewModel = viewModel(key = "upload-$toUserId-$addToItemId-$startMode-$doctorId-$hospitalId") {
        UploadViewModel(toUserId, addToItemId, startMode, doctorId, hospitalId)
    }
    val context = LocalContext.current
    var cameraUri by remember { mutableStateOf<Uri?>(null) }
    var showLinkDialog by remember { mutableStateOf(false) }

    // Multiple images at once (system photo picker, no storage permission needed)
    val pickImages = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia(Limits.MAX_ATTACHMENTS)) {
        vm.addFiles(context, it)
    }
    // Multiple PDFs at once
    val pickPdfs = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) {
        vm.addFiles(context, it, AttachmentKind.PDF)
    }
    // Camera adds one more image each time
    val takePhoto = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture()) { ok ->
        if (ok) cameraUri?.let { vm.addFiles(context, listOf(it), AttachmentKind.IMAGE) }
    }

    val fromItem = vm.mode == NewItemMode.ADD_FILES
    Scaffold(
        containerColor = Sage.Background,
        topBar = {
            PinnedHeader(
                title = if (vm.addingFiles) "Add files" else "New item",
                subtitle = when {
                    fromItem -> "To ${vm.target?.title ?: "…"}"
                    vm.addingFiles -> "To ${vm.existingTarget?.title ?: "an existing item"}"
                    else -> "For ${vm.owner?.displayName ?: "…"} · ${vm.mode.label}"
                },
                onBack = onClose, backIcon = Icons.Outlined.Close, backLabel = "Cancel",
                trailing = { (vm.fixedRecipient ?: vm.owner)?.let { Avatar(it.initials, 36.dp, photoUrl = it.photoUri) } },
            )
        },
        bottomBar = {
            BottomActionBar {
                PrimaryButton(
                    when {
                        vm.busy -> "Saving…"
                        vm.addingFiles -> "Add ${vm.files.size} file${if (vm.files.size == 1) "" else "s"}"
                        vm.mode == NewItemMode.APPOINTMENT -> "Book appointment"
                        vm.mode == NewItemMode.ALERT -> "Create alert"
                        else -> "Upload report"
                    },
                    { vm.submit { onUploaded(it.id) } }, Modifier.weight(1f), enabled = !vm.busy,
                )
            }
        },
    ) { padding ->
        ScrollbarLazyColumn(
            Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            val adding = vm.addingFiles
            if (!fromItem) item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    FieldLabel("Start with")
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        listOf(NewItemMode.REPORT, NewItemMode.APPOINTMENT, NewItemMode.ALERT).forEach { m ->
                            SageChip(m.label, vm.mode == m, { vm.mode = m; vm.message = null }, Modifier.weight(1f))
                        }
                    }
                    Text("You can add messages, files, appointments and alerts to the item later. Discussions start from a person's page.",
                        style = HType.small, color = Sage.Muted)
                }
            }
            // New documents go into a new item or into one that already exists.
            if (vm.mode == NewItemMode.REPORT) item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Segmented(listOf("New item", "Existing item"), if (vm.intoExisting) 1 else 0, { vm.intoExisting = it == 1; vm.message = null },
                        Modifier.fillMaxWidth())
                    if (vm.intoExisting) {
                        val picked = vm.existingTarget
                        if (picked != null) PickedRow({ Box(Modifier.weight(1f)) { ItemResultRow(picked, selected = true) } }, onChange = { vm.existingTarget = null })
                        else FilteredSearchBar(
                            placeholder = "Search open items", candidates = vm.existingCandidates, key = { it.id },
                            matches = { it, q -> it.title.lowercase().contains(q) || it.keywords.any { k -> k.lowercase().contains(q) } },
                            onPick = { vm.existingTarget = it; vm.message = null }, showAllWhenBlank = true, maxResults = 6,
                            emptyText = if (vm.existingCandidates.isEmpty()) "No open items to add to." else "No item matches.",
                        ) { ItemResultRow(it) }
                    }
                }
            }
            if (vm.me?.isClinical == true && !adding && vm.mode != NewItemMode.ALERT)
                item { PatientPicker(vm.patients, vm.owner, locked = vm.ownerLocked) { vm.owner = it } }
            if (vm.mode == NewItemMode.APPOINTMENT) item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    com.healoo.app.ui.item.AppointmentForm(vm.appointment, vm.doctors)
                    vm.message?.let { Text(it, style = HType.small, color = Sage.Clay) }
                }
            }
            if (vm.mode == NewItemMode.ALERT) item {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    SageTextField("Title (optional)", vm.title, { vm.title = it }, placeholder = "e.g. Evening medicine")
                    com.healoo.app.ui.item.AlertForm(vm.alert)
                    vm.message?.let { Text(it, style = HType.small, color = Sage.Clay) }
                }
            }

            if (vm.mode == NewItemMode.REPORT || adding) item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    FieldLabel("Attach · ${vm.files.size} of ${Limits.MAX_ATTACHMENTS}")
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        AttachButton("Camera", Icons.Outlined.PhotoCamera, Modifier.weight(1f)) {
                            val file = File(context.cacheDir, "captures/${System.currentTimeMillis()}.jpg").apply { parentFile?.mkdirs() }
                            val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
                            cameraUri = uri; takePhoto.launch(uri)
                        }
                        AttachButton("Images", Icons.Outlined.Image, Modifier.weight(1f)) {
                            pickImages.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
                        }
                        AttachButton("PDFs", Icons.Outlined.PictureAsPdf, Modifier.weight(1f)) { pickPdfs.launch(arrayOf("application/pdf")) }
                        if (!adding) AttachButton("Link", Icons.Outlined.Link, Modifier.weight(1f)) { showLinkDialog = true }
                    }
                    vm.message?.let { Text(it, style = HType.small, color = Sage.Clay) }
                }
            }

            if (adding) item {
                Row(Modifier.fillMaxWidth().clickable(role = SemRole.Checkbox) { vm.filesAreReport = !vm.filesAreReport },
                    verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(vm.filesAreReport, null, colors = CheckboxDefaults.colors(checkedColor = Sage.Accent))
                    Column {
                        Text("These are report files", style = HType.body, color = Sage.Ink)
                        Text("Report files open for the patient who owns the item and for doctors it is shared with.", style = HType.small, color = Sage.Muted)
                    }
                }
            }

            if ((vm.mode == NewItemMode.REPORT || adding) && (vm.files.isNotEmpty() || vm.links.isNotEmpty())) item {
                GroupCard {
                    vm.files.forEachIndexed { i, f ->
                        if (i > 0) RowDivider()
                        PendingRow(f, i, vm.files.size, onUp = { vm.move(i, -1) }, onDown = { vm.move(i, 1) }, onRemove = { vm.files.removeAt(i) })
                    }
                    vm.links.forEachIndexed { i, link ->
                        if (vm.files.isNotEmpty() || i > 0) RowDivider()
                        Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Icon(Icons.Outlined.Link, null, tint = Sage.Accent, modifier = Modifier.size(18.dp))
                            Text(link, style = HType.caption, color = Sage.Ink, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            IconButton({ vm.links.removeAt(i) }) { Icon(Icons.Outlined.Close, "Remove link", tint = Sage.Clay) }
                        }
                    }
                }
            }

            if (vm.mode == NewItemMode.REPORT && !adding) {
                item { SageTextField("Title", vm.title, { vm.title = it }, placeholder = "e.g. Home BP readings, September") }
                item { SageTextField("Keywords", vm.keywords, { vm.keywords = it }, placeholder = "Separate with commas, e.g. BP, home readings") }
            }

            // Recipients: the page's person is always included; more are added with the Filtered Search Bar.
            // Files added to an existing item keep that item's sharing.
            if (!adding) {
                if (vm.uploadingForSomeoneElse) item {
                    Text("${vm.owner?.displayName} will own this item and decide who else sees it. You keep access because you created it, " +
                        "and you can pass it on to doctors below.",
                        style = HType.caption, color = Sage.SandInk,
                        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Sage.SandTint).padding(12.dp))
                }
                item {
                    RecipientField(
                        fixed = vm.fixedRecipient, extras = vm.extraRecipients,
                        onAdd = { vm.extraRecipients += it }, onRemove = { vm.extraRecipients -= it },
                        contacts = vm.contacts, doctorsOnly = vm.uploadingForSomeoneElse,
                        exclude = setOfNotNull(vm.me?.id, vm.owner?.id), label = "Share with",
                    )
                }
            }
        }
    }

    if (showLinkDialog) LinkDialog(onDismiss = { showLinkDialog = false }) { vm.links += it; showLinkDialog = false }
}

@Composable
private fun PatientPicker(patients: List<UserProfile>, selected: UserProfile?, locked: Boolean, onSelect: (UserProfile) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        FieldLabel("Upload for")
        Box {
            OutlinedButton(
                onClick = { if (!locked) open = true }, modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
                shape = RoundedCornerShape(12.dp), border = BorderStroke(1.dp, if (selected == null) Sage.Clay else Sage.Border),
                colors = ButtonDefaults.outlinedButtonColors(containerColor = Sage.Surface),
            ) {
                if (selected != null) { Avatar(selected.initials, 28.dp, photoUrl = selected.photoUri); Spacer(Modifier.width(10.dp)) }
                Text(selected?.let { "${it.displayName} · ${it.publicId}" } ?: "Choose a patient", style = HType.body,
                    color = if (selected == null) Sage.Placeholder else Sage.Ink, modifier = Modifier.weight(1f))
                if (!locked) Icon(Icons.Outlined.ArrowDropDown, null, tint = Sage.Muted)
            }
            DropdownMenu(open, { open = false }) {
                if (patients.isEmpty()) DropdownMenuItem({ Text("No connected patients yet — add them from Search") }, { open = false })
                patients.forEach { p -> DropdownMenuItem({ Text("${p.displayName} · ${p.publicId}") }, { onSelect(p); open = false }) }
            }
        }
    }
}

@Composable
private fun AttachButton(label: String, icon: ImageVector, modifier: Modifier, onClick: () -> Unit) {
    OutlinedCard(
        onClick = onClick, modifier = modifier.height(60.dp), shape = RoundedCornerShape(14.dp),
        border = BorderStroke(1.dp, Sage.Dashed), colors = CardDefaults.outlinedCardColors(containerColor = Sage.Surface),
    ) {
        Column(Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            Icon(icon, null, tint = Sage.Accent, modifier = Modifier.size(20.dp))
            Text(label, style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = Sage.Accent)
        }
    }
}

@Composable
private fun PendingRow(f: PendingAttachment, index: Int, count: Int, onUp: () -> Unit, onDown: () -> Unit, onRemove: () -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 60.dp).padding(start = 10.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(10.dp)).background(Sage.SageTint), contentAlignment = Alignment.Center) {
            if (f.kind == AttachmentKind.IMAGE) AsyncImage(f.localUri, null, contentScale = androidx.compose.ui.layout.ContentScale.Crop, modifier = Modifier.fillMaxSize())
            else Icon(Icons.Outlined.PictureAsPdf, null, tint = Sage.Accent, modifier = Modifier.size(20.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(f.name, style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${index + 1}. ${if (f.kind == AttachmentKind.PDF) "PDF" else "Image"} · ${f.size / 1024} KB", style = HType.small, color = Sage.Muted)
        }
        IconButton(onUp, enabled = index > 0, modifier = Modifier.size(40.dp)) { Icon(Icons.Outlined.KeyboardArrowUp, "Move ${f.name} up", tint = Sage.Muted) }
        IconButton(onDown, enabled = index < count - 1, modifier = Modifier.size(40.dp)) { Icon(Icons.Outlined.KeyboardArrowDown, "Move ${f.name} down", tint = Sage.Muted) }
        IconButton(onRemove, modifier = Modifier.size(40.dp)) { Icon(Icons.Outlined.Close, "Remove ${f.name}", tint = Sage.Clay) }
    }
}

@Composable
fun SageTextField(label: String, value: String, onChange: (String) -> Unit, placeholder: String = "") {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        FieldLabel(label)
        OutlinedTextField(
            value = value, onValueChange = onChange, singleLine = true, modifier = Modifier.fillMaxWidth(),
            placeholder = { Text(placeholder, style = HType.body, color = Sage.Placeholder) },
            textStyle = HType.body.copy(color = Sage.Ink), shape = RoundedCornerShape(12.dp),
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = Sage.Surface, unfocusedContainerColor = Sage.Surface,
                focusedBorderColor = Sage.Accent, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Accent,
            ),
        )
    }
}

@Composable
private fun LinkDialog(onDismiss: () -> Unit, onAdd: (String) -> Unit) {
    var url by remember { mutableStateOf("https://") }
    val valid = url.startsWith("https://") && url.length > 10
    AlertDialog(
        onDismissRequest = onDismiss, containerColor = Sage.Surface,
        title = { Text("Add a link", style = HType.section) },
        text = { SageTextField("Web address", url, { url = it.trim() }) },
        confirmButton = { TextButton({ onAdd(url) }, enabled = valid) { Text("Add link", color = Sage.Accent) } },
        dismissButton = { TextButton(onDismiss) { Text("Cancel", color = Sage.Muted) } },
    )
}
