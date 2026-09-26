package com.healoo.app.ui.user

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Send
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.discussion.Composer
import com.healoo.app.ui.discussion.ConversationRow
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class UserPageViewModel(private val userId: String, startOnMessages: Boolean) : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var user by mutableStateOf<UserProfile?>(null); private set
    var shared by mutableStateOf<List<DataItem>>(emptyList()); private set
    /** Items with a discussion between the caller and this person (DataItem v2). */
    var conversations by mutableStateOf<List<Conversation>>(emptyList()); private set
    var tab by mutableIntStateOf(if (startOnMessages) 1 else 0)
    var draft by mutableStateOf("")
    var sending by mutableStateOf(false); private set
    var error by mutableStateOf<String?>(null); private set

    init {
        load()
        viewModelScope.launch {
            repo.events.collect { e -> if (e is RealtimeEvent.NewMessage) refreshConversations() }
        }
    }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            me = repo.me()
            user = repo.user(userId)
            if (user!!.connected) {
                shared = repo.sharedItems(userId)
                conversations = repo.conversations(userId)
            }
        }.onFailure { error = "Couldn't load this profile. Try again." }
    }

    private fun refreshConversations() = viewModelScope.launch {
        runCatching { repo.conversations(userId) }.onSuccess { conversations = it }
    }

    fun connect() = viewModelScope.launch { runCatching { repo.connect(userId) }.onSuccess { load() } }

    /** Starts a new MESSAGE item with this person; returns its id for navigation. */
    fun startConversation(onStarted: (String) -> Unit) {
        val body = draft.trim().ifEmpty { return }
        sending = true
        viewModelScope.launch {
            runCatching { repo.startConversation(userId, body) }
                .onSuccess { draft = ""; onStarted(it.id); refreshConversations() }
                .onFailure { error = null }
            sending = false
        }
    }
}

@Composable
fun UserPageScreen(
    userId: String,
    startOnMessages: Boolean,
    onBack: () -> Unit,
    onOpenItem: (String) -> Unit,
    onOpenDiscussion: (String) -> Unit,
    onUploadFor: (String) -> Unit,
    onTab: (Tab) -> Unit,
) {
    val vm: UserPageViewModel = viewModel(key = "user-$userId") { UserPageViewModel(userId, startOnMessages) }
    val user = vm.user

    Scaffold(containerColor = Sage.Background, bottomBar = { HealooBottomBar(Tab.MESSAGES, onTab) }) { padding ->
        Box(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            when {
                vm.error != null -> Column(Modifier.statusBarsPadding()) { ErrorBox(vm.error!!, vm::load) }
                user == null -> LoadingBox(Modifier.statusBarsPadding())
                else -> CollapsingHeaderLayout(
                    expandedFraction = HeaderRatio.USER_EXPANDED,
                    collapsedFraction = HeaderRatio.USER_COLLAPSED,
                    header = { progress -> ProfileHeader(user, progress, onBack) },
                ) {
                    if (!user.connected) NotConnected(user, vm::connect)
                    else {
                        // Doctors, assistants and labs can upload a record that the patient will own (doc 2.3).
                        if (vm.me?.isClinical == true && user.primaryRole == Role.PATIENT) {
                            PrimaryButton("Upload for ${user.displayName.substringBefore(' ')}", { onUploadFor(user.id) },
                                Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp))
                        }
                        Segmented(
                            listOf("Shared items · ${vm.shared.size}", "Messages · ${vm.conversations.size}"), vm.tab, { vm.tab = it },
                            Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 8.dp),
                        )
                        if (vm.tab == 0) SharedItems(vm.shared, onOpenItem)
                        else Conversations(vm.conversations, onOpenDiscussion, vm.draft, { vm.draft = it }) { vm.startConversation(onOpenDiscussion) }
                    }
                }
            }
        }
    }
}

@Composable
private fun ProfileHeader(user: UserProfile, progress: Float, onBack: () -> Unit) {
    val full = 1f - (progress * 1.6f).coerceIn(0f, 1f)          // details fade out early
    Box(
        Modifier.fillMaxSize()
            .clip(RoundedCornerShape(bottomStart = Radius.header * (1 - progress), bottomEnd = Radius.header * (1 - progress)))
            .background(Sage.Primary).statusBarsPadding()
            .padding(start = 20.dp, end = 20.dp, top = 8.dp, bottom = 12.dp),
    ) {
        // Collapsed row (always present; name slides in as details fade)
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            HeaderIconButton(Icons.AutoMirrored.Outlined.ArrowBack, "Back", onBack)
            Text(user.displayName, style = HType.headerTitle, color = Color.White, modifier = Modifier.weight(1f).alpha(1f - full),
                maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (user.connected) ConnectedPill()
        }
        if (full > 0.02f) Column(
            Modifier.align(Alignment.BottomStart).fillMaxWidth().alpha(full),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Avatar(user.initials, 64.dp, border = Sage.OnPrimaryLine)
                Column {
                    Text(user.displayName, style = HType.profileName, color = Color.White)
                    Text(user.headline, style = HType.caption, color = Sage.OnPrimarySoft)
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOfNotNull(
                    user.hospital?.let { "Hospital" to it },
                    user.officialNumber?.let { "Reg. number" to it },
                    "Healoo ID" to user.publicId,
                ).forEach { (k, v) ->
                    Column(Modifier.weight(1f)) {
                        Text(k, style = HType.tiny.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Normal), color = Sage.OnPrimaryLine)
                        Text(v, style = HType.caption.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = Color.White,
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

@Composable
private fun ConnectedPill() {
    Row(
        Modifier.clip(RoundedCornerShape(15.dp)).background(Sage.PrimaryRaised).padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(Icons.Outlined.Check, contentDescription = null, tint = Sage.OnPrimarySoft, modifier = Modifier.size(14.dp))
        Text("Connected", style = HType.small.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold), color = Sage.OnPrimarySoft)
    }
}

@Composable
private fun NotConnected(user: UserProfile, onConnect: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("You can see ${user.displayName}'s profile. Add them to your contacts to message them; " +
            "records appear only after the owner shares them.", style = HType.body, color = Sage.InkSoft)
        PrimaryButton("Add to contacts", onConnect, Modifier.fillMaxWidth())
    }
}

@Composable
private fun SharedItems(items: List<DataItem>, onOpenItem: (String) -> Unit) {
    LazyColumn(
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 8.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (items.isEmpty()) item {
            Text("Nothing shared between you yet.", style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 12.dp))
        }
        items(items, key = { it.id }) { DataItemRow(it, onClick = { onOpenItem(it.id) }) }
    }
}

/** Discussions with this person, newest first, plus a composer that starts a new one. */
@Composable
private fun ColumnScope.Conversations(
    list: List<Conversation>, onOpen: (String) -> Unit,
    draft: String, onDraft: (String) -> Unit, onStart: () -> Unit,
) {
    LazyColumn(
        modifier = Modifier.weight(1f),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (list.isEmpty()) item {
            Text("No discussions yet. Write below to start one — it becomes an item you can both add to.",
                style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 12.dp))
        }
        items(list, key = { it.itemId }) { c -> ConversationRow(c, onClick = { onOpen(c.itemId) }, showPerson = false) }
    }
    Composer(draft, onDraft, onStart, placeholder = "Start a new discussion")
}
