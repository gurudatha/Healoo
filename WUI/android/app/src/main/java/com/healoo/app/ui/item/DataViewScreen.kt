package com.healoo.app.ui.item

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.*
import com.healoo.app.ui.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import coil.compose.AsyncImage
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class DataViewModel(private val itemId: String) : ViewModel() {
    private val repo = ServiceLocator.repository
    var item by mutableStateOf<DataItem?>(null); private set
    var me by mutableStateOf<UserProfile?>(null); private set
    var contacts by mutableStateOf<List<UserProfile>>(emptyList()); private set
    var error by mutableStateOf<String?>(null); private set
    /** One-line result of the last action (errors from the server included). */
    var notice by mutableStateOf<String?>(null)

    init {
        load()
        // Refresh when someone else changes this item (design doc 4.4, item.updated / message.new).
        viewModelScope.launch {
            repo.events.collect { e ->
                val mine = (e is RealtimeEvent.ItemChanged && e.itemId == itemId) || (e is RealtimeEvent.NewMessage && e.message.itemId == itemId)
                if (mine) runCatching { repo.item(itemId) }.onSuccess { item = it }
            }
        }
    }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            me = repo.me()
            item = repo.item(itemId)
            contacts = repo.connections()
        }.onFailure { error = "Couldn't open this item. You may no longer have access to it." }
    }

    val doctors: List<UserProfile> get() = (contacts + listOfNotNull(me)).filter { it.primaryRole == Role.DOCTOR }.distinctBy { it.id }

    private fun act(done: String? = null, block: suspend () -> DataItem) = viewModelScope.launch {
        runCatching { block() }
            .onSuccess { item = it; notice = done }
            .onFailure { notice = it.message ?: "That didn't work. Try again." }
    }

    fun share(granteeId: String) = act("Shared") { repo.share(itemId, granteeId) }
    fun revoke(grant: Grant) = act("Stopped sharing with ${grant.granteeName}") { repo.revokeGrant(itemId, grant.grantId) }
    fun close(feedback: String?, rating: Int?) = act("Item closed") { repo.closeItem(itemId, feedback, rating) }
    fun reopen() = act("Item reopened") { repo.reopenItem(itemId) }
    fun book(a: NewAppointment) = act("Appointment booked") { repo.bookAppointment(itemId, a) }
    fun visit(a: Appointment, v: Visit, action: String, newDate: String? = null, newTime: String? = null) =
        act(when (action) { "CANCELLED" -> "Visit cancelled"; "MOVED" -> "Visit moved"; "COMPLETED" -> "Marked completed"; else -> "Marked as missed" }) {
            repo.visitAction(itemId, a.id, v.originalDate, action, newDate, newTime)
        }
    fun cancelSeries(a: Appointment) = act("Appointment cancelled") { repo.cancelAppointment(itemId, a.id) }
    fun addAlert(a: NewAlert) = act("Alert added") { repo.addAlert(itemId, a) }
    fun deleteAlert(a: Alert) = act("Alert removed") { repo.deleteAlert(itemId, a.id) }
}

@Composable
fun DataViewScreen(
    itemId: String,
    onBack: () -> Unit,
    onOpenAttachment: (itemId: String, index: Int) -> Unit,
    onOpenDiscussion: (itemId: String) -> Unit,
    onAddFiles: (itemId: String) -> Unit,
) {
    val vm: DataViewModel = viewModel(key = "item-$itemId") { DataViewModel(itemId) }
    val item = vm.item
    val me = vm.me
    var confirmRevoke by remember { mutableStateOf<Grant?>(null) }
    var showShare by remember { mutableStateOf(false) }
    var showClose by remember { mutableStateOf(false) }
    var showBook by remember { mutableStateOf(false) }
    var showAlert by remember { mutableStateOf(false) }
    var moving by remember { mutableStateOf<Pair<Appointment, Visit>?>(null) }
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(vm.notice) { vm.notice?.let { snackbar.showSnackbar(it); vm.notice = null } }

    Scaffold(
        containerColor = Sage.Background,
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            PinnedHeader(
                title = item?.title ?: "",
                subtitle = item?.let { "${it.primaryKind.label} · added by ${it.createdByName.ifEmpty { "—" }}" } ?: "",
                onBack = onBack,
                trailing = { me?.let { Avatar(it.initials, 36.dp, photoUrl = it.photoUri) } },
            )
        },
        bottomBar = {
            if (item != null) BottomActionBar {
                when {
                    item.can("message") || item.messages.isNotEmpty() ->
                        SecondaryButton(if (item.messages.isEmpty()) "Start discussion" else "Discussion", { onOpenDiscussion(item.id) }, Modifier.weight(1f))
                }
                if (item.can("share")) PrimaryButton("Share with…", { showShare = true }, Modifier.weight(1f))
                else if (item.can("reopen")) PrimaryButton("Reopen", vm::reopen, Modifier.weight(1f))
            }
        },
    ) { padding ->
        when {
            vm.error != null -> Box(Modifier.padding(padding)) { ErrorBox(vm.error!!, vm::load) }
            item == null || me == null -> LoadingBox(Modifier.padding(padding))
            else -> ScrollbarLazyColumn(
                Modifier.fillMaxSize().padding(padding),
                contentPadding = PaddingValues(20.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                item { MetaRow(item, onClose = { showClose = true }, onReopen = vm::reopen) }
                item.closure?.let { c -> item { ClosureCard(c) } }
                if (item.can("meta")) item {
                    Text("This item contains report files, which open only for doctors. You can see its appointments.",
                        style = HType.body, color = Sage.InkSoft)
                }
                if (item.attachments.isNotEmpty() || item.can("attach")) item {
                    AttachmentsSection(item.attachments, canAdd = item.can("attach"), onAdd = { onAddFiles(item.id) }) { index -> onOpenAttachment(item.id, index) }
                }
                if (item.messages.isNotEmpty()) item { DiscussionPreview(item, me.id) { onOpenDiscussion(item.id) } }
                if (item.appointments.isNotEmpty() || item.can("book")) item {
                    AppointmentsSection(item, me, canBook = item.can("book"), onBook = { showBook = true },
                        onVisit = { a, v, action -> if (action == "MOVED") moving = a to v else vm.visit(a, v, action) },
                        onCancelSeries = vm::cancelSeries)
                }
                if (item.alerts.isNotEmpty() || item.can("alert")) item {
                    AlertsSection(item.alerts, canAdd = item.can("alert"), myId = me.id, isOwner = item.ownerId == me.id,
                        onAdd = { showAlert = true }, onDelete = vm::deleteAlert)
                }
                if (item.links.isNotEmpty()) item { LinksSection(item.links) }
                if (item.keywords.isNotEmpty()) item { KeywordsSection(item.keywords) }
                if (item.ownerId == me.id) item { AccessSection(item, isOwner = true, onRevoke = { confirmRevoke = it }) }
            }
        }
    }

    if (showShare && item != null) {
        val available = vm.contacts.filter { c -> (c.primaryRole == Role.DOCTOR || c.primaryRole == Role.HOSPITAL || c.primaryRole == Role.PATIENT) &&
            item.accessList.none { it.granteeId == c.id } }
        AlertDialog(
            onDismissRequest = { showShare = false }, containerColor = Sage.Surface,
            title = { Text("Share this item", style = HType.section) },
            text = {
                if (available.isEmpty()) Text("Everyone in your contacts already has access. Add a doctor or hospital from Search first.", style = HType.body)
                else Column {
                    available.forEach { c ->
                        Row(Modifier.fillMaxWidth().clickable { vm.share(c.id); showShare = false }.heightIn(min = 48.dp),
                            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            Avatar(c.initials, 36.dp, photoUrl = c.photoUri)
                            Column {
                                Text(c.displayName, style = HType.bodyStrong, color = Sage.Ink)
                                Text(if (c.primaryRole == Role.HOSPITAL) "All its doctors" else c.headline, style = HType.small, color = Sage.Muted)
                            }
                        }
                    }
                }
            },
            confirmButton = { TextButton({ showShare = false }) { Text("Done", color = Sage.Accent) } },
        )
    }

    confirmRevoke?.let { grant ->
        AlertDialog(
            onDismissRequest = { confirmRevoke = null },
            title = { Text("Stop sharing with ${grant.granteeName}?", style = HType.section) },
            text = { Text("They will lose access to this item straight away. You can share it again later.", style = HType.body) },
            confirmButton = { TextButton({ vm.revoke(grant); confirmRevoke = null }) { Text("Stop sharing", color = Sage.Clay) } },
            dismissButton = { TextButton({ confirmRevoke = null }) { Text("Keep sharing", color = Sage.Accent) } },
            containerColor = Sage.Surface,
        )
    }

    if (showClose && item != null) CloseItemDialog(canRate = item.can("rate"), onDismiss = { showClose = false }) { f, r ->
        showClose = false; vm.close(f, r)
    }
    if (showBook && item != null) BookAppointmentDialog(item.ownerId, vm.doctors,
        defaultDoctor = me?.id?.takeIf { me.primaryRole == Role.DOCTOR }, onDismiss = { showBook = false }) { a -> showBook = false; vm.book(a) }
    if (showAlert) AddAlertDialog(onDismiss = { showAlert = false }) { a -> showAlert = false; vm.addAlert(a) }
    moving?.let { (a, v) -> MoveVisitDialog(v, onDismiss = { moving = null }) { d, t -> moving = null; vm.visit(a, v, "MOVED", d, t) } }
}

@Composable
private fun MetaRow(item: DataItem, onClose: () -> Unit, onReopen: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        val s = styleFor(item.primaryKind)
        Pill(item.primaryKind.label, s.tint, s.fg)
        item.kinds.filter { it != item.primaryKind.name && it != PartKind.ATTACHMENT }.take(2).forEach {
            Pill("+ " + it.lowercase().replaceFirstChar { c -> c.uppercase() }, Sage.SandTint, Sage.Sand)
        }
        Spacer(Modifier.weight(1f))
        val open = item.status == ItemStatus.OPEN
        val canChange = if (open) item.can("close") else item.can("reopen")
        OutlinedButton(
            onClick = { if (open) onClose() else onReopen() }, enabled = canChange, shape = RoundedCornerShape(16.dp),
            border = BorderStroke(1.dp, if (open) Sage.Accent else Sage.Border),
            contentPadding = PaddingValues(horizontal = 12.dp), modifier = Modifier.height(36.dp)
                .semantics { contentDescription = if (open) "Status open. Tap to close" else "Status closed. Tap to reopen" },
        ) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(if (open) Sage.Accent else Sage.Muted))
            Spacer(Modifier.width(6.dp))
            Text(if (open) "Open" else "Closed", style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = if (open) Sage.Accent else Sage.Muted)
        }
    }
}

@Composable
private fun ClosureCard(c: Closure) {
    GroupCard {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("Closed ${longDate(c.closedAt.take(10))}", style = HType.bodyStrong, color = Sage.Ink)
            c.rating?.let { r ->
                Row { (1..5).forEach { n -> Icon(if (n <= r) Icons.Filled.Star else Icons.Outlined.StarOutline, null, tint = Sage.Sand, modifier = Modifier.size(18.dp)) } }
            }
            c.feedback?.let { Text(it, style = HType.body, color = Sage.InkSoft) }
        }
    }
}

@Composable
private fun DiscussionPreview(item: DataItem, myId: String, onOpen: () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel("Discussion · ${item.messages.size}" + if (item.counts.unreadMessages > 0) " · ${item.counts.unreadMessages} new" else "")
        GroupCard {
            item.messages.takeLast(3).forEachIndexed { i, m ->
                if (i > 0) RowDivider()
                Column(Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(horizontal = 14.dp, vertical = 10.dp)) {
                    Text(senderName(item, m.senderId, myId),
                        style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = Sage.Muted)
                    Text(m.body, style = HType.body, color = Sage.Ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
            }
            RowDivider()
            Row(Modifier.fillMaxWidth().clickable(onClick = onOpen).heightIn(min = 48.dp).padding(horizontal = 14.dp),
                verticalAlignment = Alignment.CenterVertically) {
                Text("Open discussion", style = HType.bodyStrong, color = Sage.Accent, modifier = Modifier.weight(1f))
                Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, null, tint = Sage.Accent)
            }
        }
    }
}

/** "You", a name from the access list, or the owner. */
fun senderName(item: DataItem, senderId: String, myId: String): String = when (senderId) {
    myId -> "You"
    item.ownerId -> "Patient"
    else -> item.accessList.firstOrNull { it.granteeId == senderId }?.granteeName
        ?: item.appointments.firstOrNull { it.doctorId == senderId }?.doctorName?.ifEmpty { null }
        ?: "Care team"
}

private fun visitLabel(v: Visit): String = "${longDate(v.date)} · ${v.time}" + when (v.status) {
    "CANCELLED" -> " · cancelled"; "COMPLETED" -> " · completed"; "NO_SHOW" -> " · missed"; else -> ""
}

@Composable
private fun AppointmentsSection(
    item: DataItem, me: UserProfile, canBook: Boolean, onBook: () -> Unit,
    onVisit: (Appointment, Visit, String) -> Unit, onCancelSeries: (Appointment) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            FieldLabel("Appointments · ${item.appointments.size}", Modifier.weight(1f))
            if (canBook) TextButton(onBook) { Text("Book", style = HType.bodyStrong, color = Sage.Accent) }
        }
        item.appointments.forEach { a ->
            val manage = item.can("book") && a.status != "CANCELLED"
            val isDoctorSide = me.id == a.doctorId || me.primaryRole == Role.ASSISTANT
            GroupCard {
                Column(Modifier.padding(horizontal = 14.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(a.doctorName.ifEmpty { "Doctor" }, style = HType.bodyStrong, color = Sage.Ink)
                    Text((a.recurrence?.label ?: "Single visit") + (a.visitCount?.let { if (a.recurrence != null) " · $it visits" else "" } ?: "") +
                        if (a.status == "CANCELLED") " · cancelled" else "", style = HType.small, color = Sage.Muted)
                    a.notes?.let { Text(it, style = HType.small, color = Sage.InkSoft) }
                }
                a.visits.take(5).forEach { v ->
                    RowDivider()
                    VisitRow(v, enabled = manage && v.status == "SCHEDULED", doctorSide = isDoctorSide) { action -> onVisit(a, v, action) }
                }
                if (a.visits.isEmpty()) { RowDivider(); Text("No upcoming visits.", style = HType.small, color = Sage.Muted, modifier = Modifier.padding(14.dp)) }
                if (manage) {
                    RowDivider()
                    TextButton({ onCancelSeries(a) }, Modifier.padding(horizontal = 4.dp)) {
                        Text(if (a.recurrence != null) "Cancel all future visits" else "Cancel appointment", style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Clay)
                    }
                }
            }
        }
    }
}

@Composable
private fun VisitRow(v: Visit, enabled: Boolean, doctorSide: Boolean, onAction: (String) -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 14.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Outlined.Event, null, tint = if (v.status == "SCHEDULED") Sage.Sand else Sage.Muted, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(10.dp))
        Text(visitLabel(v), style = HType.caption, color = if (v.status == "SCHEDULED") Sage.Ink else Sage.Muted, modifier = Modifier.weight(1f))
        if (enabled) Box {
            IconButton({ menu = true }) { Icon(Icons.Outlined.MoreVert, "Change this visit", tint = Sage.Muted) }
            DropdownMenu(menu, { menu = false }) {
                DropdownMenuItem({ Text("Move this visit") }, { menu = false; onAction("MOVED") })
                DropdownMenuItem({ Text("Cancel this visit") }, { menu = false; onAction("CANCELLED") })
                if (doctorSide) {
                    DropdownMenuItem({ Text("Mark completed") }, { menu = false; onAction("COMPLETED") })
                    DropdownMenuItem({ Text("Mark missed") }, { menu = false; onAction("NO_SHOW") })
                }
            }
        }
    }
}

@Composable
private fun AlertsSection(alerts: List<Alert>, canAdd: Boolean, myId: String, isOwner: Boolean, onAdd: () -> Unit, onDelete: (Alert) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            FieldLabel("Alerts · ${alerts.size}", Modifier.weight(1f))
            if (canAdd) TextButton(onAdd) { Text("Add", style = HType.bodyStrong, color = Sage.Accent) }
        }
        if (alerts.isNotEmpty()) GroupCard {
            alerts.forEachIndexed { i, a ->
                if (i > 0) RowDivider()
                Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(start = 14.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Outlined.NotificationsNone, null, tint = if (a.active) Sage.Clay else Sage.Muted, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(a.text, style = HType.bodyStrong, color = if (a.active) Sage.Ink else Sage.Muted)
                        Text("${a.firesAt.drop(11).take(5)} · ${a.recurrence?.label ?: "once on ${longDate(a.firesAt.take(10))}"}" + if (!a.active) " · stopped" else "",
                            style = HType.small, color = Sage.Muted)
                    }
                    if (canAdd && (isOwner || a.forUser == myId) && a.type != "APPOINTMENT_REMINDER")
                        IconButton({ onDelete(a) }) { Icon(Icons.Outlined.DeleteOutline, "Remove alert", tint = Sage.Muted) }
                }
            }
        }
    }
}

@Composable
private fun Pill(text: String, bg: androidx.compose.ui.graphics.Color, fg: androidx.compose.ui.graphics.Color) =
    Text(text, style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = fg,
        modifier = Modifier.clip(RoundedCornerShape(14.dp)).background(bg).padding(horizontal = 12.dp, vertical = 6.dp))

/** Thumbnails of every image/PDF; tapping one opens the swipeable viewer at that position (doc 3.6). */
@Composable
private fun AttachmentsSection(attachments: List<Attachment>, canAdd: Boolean, onAdd: () -> Unit, onOpen: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            FieldLabel("Attachments · ${attachments.size}", Modifier.weight(1f))
            if (canAdd) TextButton(onAdd) { Text("Add files", style = HType.bodyStrong, color = Sage.Accent) }
        }
        if (attachments.isNotEmpty()) LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            itemsIndexed(attachments, key = { _, a -> a.id.ifEmpty { a.uri } }) { index, a ->
                Column(
                    Modifier.width(132.dp).clip(RoundedCornerShape(Radius.card)).background(Sage.Surface)
                        .clickable { onOpen(index) }
                        .semantics { contentDescription = "Open ${a.name}, ${index + 1} of ${attachments.size}" },
                ) {
                    Box(Modifier.fillMaxWidth().height(110.dp).background(Sage.Preview), contentAlignment = Alignment.Center) {
                        when {
                            a.kind == AttachmentKind.IMAGE -> AsyncImage(thumbnailRequest(LocalContext.current, a.thumbUri ?: a.uri), null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
                            a.thumbUri != null -> AsyncImage(thumbnailRequest(LocalContext.current, a.thumbUri), null, contentScale = ContentScale.Crop, alignment = Alignment.TopCenter, modifier = Modifier.fillMaxSize())
                            else -> Icon(Icons.Outlined.PictureAsPdf, null, tint = Sage.Muted, modifier = Modifier.size(34.dp))
                        }
                    }
                    Column(Modifier.padding(horizontal = 10.dp, vertical = 8.dp)) {
                        Text(a.name, style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(if (a.kind == AttachmentKind.PDF) "PDF · ${a.pageCount ?: "?"} pages" else "Image · ${a.size / 1024} KB",
                            style = HType.tiny.copy(fontWeight = FontWeight.Normal), color = Sage.Muted)
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun KeywordsSection(keywords: List<String>) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel("Keywords")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            keywords.forEach {
                Text(it, style = HType.caption, color = Sage.Ink, modifier = Modifier.clip(RoundedCornerShape(16.dp))
                    .background(Sage.Surface).border(1.dp, Sage.Border, RoundedCornerShape(16.dp)).padding(horizontal = 12.dp, vertical = 7.dp))
            }
        }
    }
}

@Composable
private fun LinksSection(links: List<String>) {
    val uri = LocalUriHandler.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel("Links")
        GroupCard {
            links.forEachIndexed { i, link ->
                Row(Modifier.fillMaxWidth().clickable { uri.openUri(link) }.heightIn(min = 48.dp).padding(horizontal = 14.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Icon(Icons.Outlined.Link, null, tint = Sage.Accent, modifier = Modifier.size(18.dp))
                    Text(link, style = HType.caption, color = Sage.Accent, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                if (i < links.lastIndex) RowDivider()
            }
        }
    }
}

@Composable
private fun AccessSection(item: DataItem, isOwner: Boolean, onRevoke: (Grant) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel("Who can see this")
        GroupCard {
            AccessRow(Icons.Outlined.Person, if (isOwner) "You (owner)" else "Owner", "Full control", null)
            item.accessList.forEach { g ->
                RowDivider()
                val hospital = g.granteeType == GranteeType.HOSPITAL
                AccessRow(
                    if (hospital) Icons.Outlined.LocalHospital else Icons.Outlined.Person, g.granteeName,
                    if (hospital) "All affiliated doctors" else if (g.viaHospitalId != null) "Via hospital" else "Shared directly",
                    if (isOwner) ({ onRevoke(g) }) else null,
                )
            }
        }
        if (item.attachments.any { it.isReport })
            Text("Report files open for the patient who owns this item and for doctors it is shared with.", style = HType.small, color = Sage.Muted)
    }
}

@Composable
private fun AccessRow(icon: androidx.compose.ui.graphics.vector.ImageVector, name: String, note: String, onRevoke: (() -> Unit)?) {
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(32.dp).clip(RoundedCornerShape(10.dp)).background(Sage.SageTint), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = Sage.Accent, modifier = Modifier.size(17.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(name, style = HType.bodyStrong, color = Sage.Ink)
            Text(note, style = HType.small, color = Sage.Muted)
        }
        if (onRevoke != null) TextButton(onClick = onRevoke) { Text("Revoke", style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Clay) }
    }
}
