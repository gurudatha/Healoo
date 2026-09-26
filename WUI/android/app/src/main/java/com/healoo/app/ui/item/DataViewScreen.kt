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
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
    var linked by mutableStateOf<DataItem?>(null); private set
    var me by mutableStateOf<UserProfile?>(null); private set
    var contacts by mutableStateOf<List<UserProfile>>(emptyList()); private set
    var error by mutableStateOf<String?>(null); private set

    init { load() }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            me = repo.me()
            item = repo.item(itemId)
            linked = item!!.pointerItemId?.let { runCatching { repo.item(it) }.getOrNull() }
            contacts = repo.connections().filter { it.primaryRole == Role.DOCTOR || it.primaryRole == Role.HOSPITAL }
        }.onFailure { error = "Couldn't open this item. You may no longer have access to it." }
    }

    fun toggleStatus() = viewModelScope.launch {
        val current = item ?: return@launch
        val next = if (current.status == ItemStatus.OPEN) ItemStatus.CLOSED else ItemStatus.OPEN
        runCatching { repo.setStatus(itemId, next) }.onSuccess { item = it }
    }

    fun share(granteeId: String) = viewModelScope.launch {
        runCatching { repo.share(itemId, granteeId) }.onSuccess { item = it }
    }

    fun revoke(grant: Grant) = viewModelScope.launch {
        runCatching { repo.revokeGrant(itemId, grant.grantId) }.onSuccess { item = it }
    }
}

@Composable
fun DataViewScreen(
    itemId: String,
    onBack: () -> Unit,
    onOpenAttachment: (itemId: String, index: Int) -> Unit,
    onOpenItem: (String) -> Unit,
    onMessage: (doctorUserId: String?) -> Unit,
) {
    val vm: DataViewModel = viewModel(key = "item-$itemId") { DataViewModel(itemId) }
    val item = vm.item
    var confirmRevoke by remember { mutableStateOf<Grant?>(null) }
    var showShare by remember { mutableStateOf(false) }

    Scaffold(
        containerColor = Sage.Background,
        topBar = {
            PinnedHeader(
                title = item?.title ?: "",
                subtitle = item?.let { "${vm.me?.displayName ?: ""} · added by ${it.createdByName}" } ?: "",
                onBack = onBack,
                trailing = { vm.me?.let { Avatar(it.initials, 36.dp) } },
            )
        },
        bottomBar = {
            if (item != null) BottomActionBar {
                val isOwner = item.ownerId == vm.me?.id
                if (isOwner) {
                    val doctor = item.accessList.firstOrNull { it.granteeType == GranteeType.USER }
                    SecondaryButton("Message doctor", { onMessage(doctor?.granteeId) }, Modifier.weight(1f))
                } else {
                    // Doctor or lab viewing a patient's record: talk to the owner.
                    SecondaryButton("Message patient", { onMessage(item.ownerId) }, Modifier.weight(1f))
                }
                if (item.ownerId == vm.me?.id) PrimaryButton("Share with…", { showShare = true }, Modifier.weight(1f))
            }
        },
    ) { padding ->
        when {
            vm.error != null -> Box(Modifier.padding(padding)) { ErrorBox(vm.error!!, vm::load) }
            item == null -> LoadingBox(Modifier.padding(padding))
            else -> LazyColumn(
                Modifier.fillMaxSize().padding(padding),
                contentPadding = PaddingValues(20.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                item { MetaRow(item, canChange = "status" in item.allowedActions || item.ownerId == vm.me?.id, onToggle = vm::toggleStatus) }
                if (item.attachments.isNotEmpty()) item {
                    AttachmentsSection(item.attachments) { index -> onOpenAttachment(item.id, index) }
                }
                if (item.links.isNotEmpty()) item { LinksSection(item.links) }
                if (item.keywords.isNotEmpty()) item { KeywordsSection(item.keywords) }
                item {
                    AccessSection(item, isOwner = item.ownerId == vm.me?.id, onRevoke = { confirmRevoke = it })
                }
                vm.linked?.let { linked ->
                    item {
                        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            FieldLabel("Linked items")
                            DataItemRow(linked, onClick = { onOpenItem(linked.id) })
                        }
                    }
                }
            }
        }
    }

    if (showShare && item != null) {
        val available = vm.contacts.filter { c -> item.accessList.none { it.granteeId == c.id } }
        AlertDialog(
            onDismissRequest = { showShare = false }, containerColor = Sage.Surface,
            title = { Text("Share this item", style = HType.section) },
            text = {
                if (available.isEmpty()) Text("Everyone in your contacts already has access. Add a doctor or hospital from Search first.", style = HType.body)
                else Column {
                    available.forEach { c ->
                        Row(Modifier.fillMaxWidth().clickable { vm.share(c.id); showShare = false }.heightIn(min = 48.dp),
                            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            Avatar(c.initials, 36.dp)
                            Column {
                                Text(c.displayName, style = HType.bodyStrong, color = Sage.Ink)
                                Text(if (c.primaryRole == Role.HOSPITAL) "All its doctors" else c.headline, style = HType.small, color = Sage.Muted)
                            }
                        }
                    }
                }
            },
            confirmButton = { TextButton({ showShare = false }) { Text("Done", color = Sage.Primary) } },
        )
    }

    confirmRevoke?.let { grant ->
        AlertDialog(
            onDismissRequest = { confirmRevoke = null },
            title = { Text("Stop sharing with ${grant.granteeName}?", style = HType.section) },
            text = { Text("They will lose access to this item straight away. You can share it again later.", style = HType.body) },
            confirmButton = { TextButton({ vm.revoke(grant); confirmRevoke = null }) { Text("Stop sharing", color = Sage.Clay) } },
            dismissButton = { TextButton({ confirmRevoke = null }) { Text("Keep sharing", color = Sage.Primary) } },
            containerColor = Sage.Surface,
        )
    }
}

@Composable
private fun MetaRow(item: DataItem, canChange: Boolean, onToggle: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        val s = styleFor(item.type)
        Pill(item.type.label, s.tint, s.fg)
        Pill(longDate(item.date), Sage.SandTint, Sage.Sand)
        Spacer(Modifier.weight(1f))
        val open = item.status == ItemStatus.OPEN
        OutlinedButton(
            onClick = onToggle, enabled = canChange, shape = RoundedCornerShape(16.dp),
            border = BorderStroke(1.dp, if (open) Sage.Primary else Sage.Border),
            contentPadding = PaddingValues(horizontal = 12.dp), modifier = Modifier.height(36.dp)
                .semantics { contentDescription = if (open) "Status open. Tap to close" else "Status closed. Tap to reopen" },
        ) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(if (open) Sage.Primary else Sage.Muted))
            Spacer(Modifier.width(6.dp))
            Text(if (open) "Open" else "Closed", style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = if (open) Sage.Primary else Sage.Muted)
        }
    }
}

@Composable
private fun Pill(text: String, bg: androidx.compose.ui.graphics.Color, fg: androidx.compose.ui.graphics.Color) =
    Text(text, style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = fg,
        modifier = Modifier.clip(RoundedCornerShape(14.dp)).background(bg).padding(horizontal = 12.dp, vertical = 6.dp))

/** Thumbnails of every image/PDF; tapping one opens the swipeable viewer at that position (doc 3.6). */
@Composable
private fun AttachmentsSection(attachments: List<Attachment>, onOpen: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel("Attachments · ${attachments.size}")
        LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            itemsIndexed(attachments, key = { _, a -> a.uri }) { index, a ->
                Column(
                    Modifier.width(132.dp).clip(RoundedCornerShape(Radius.card)).background(Sage.Surface)
                        .clickable { onOpen(index) }
                        .semantics { contentDescription = "Open ${a.name}, ${index + 1} of ${attachments.size}" },
                ) {
                    Box(Modifier.fillMaxWidth().height(110.dp).background(Sage.Preview), contentAlignment = Alignment.Center) {
                        when {
                            a.kind == AttachmentKind.IMAGE -> AsyncImage(a.thumbUri ?: a.uri, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
                            a.thumbUri != null -> AsyncImage(a.thumbUri, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
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
                    Icon(Icons.Outlined.Link, null, tint = Sage.Primary, modifier = Modifier.size(18.dp))
                    Text(link, style = HType.caption, color = Sage.Primary, maxLines = 1, overflow = TextOverflow.Ellipsis)
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
        if (item.type == CoreItemType.REPORT)
            Text("Report files open only for doctors you share with.", style = HType.small, color = Sage.Muted)
    }
}

@Composable
private fun AccessRow(icon: androidx.compose.ui.graphics.vector.ImageVector, name: String, note: String, onRevoke: (() -> Unit)?) {
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(32.dp).clip(RoundedCornerShape(10.dp)).background(Sage.SageTint), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = Sage.Primary, modifier = Modifier.size(17.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(name, style = HType.bodyStrong, color = Sage.Ink)
            Text(note, style = HType.small, color = Sage.Muted)
        }
        if (onRevoke != null) TextButton(onClick = onRevoke) { Text("Revoke", style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Clay) }
    }
}
