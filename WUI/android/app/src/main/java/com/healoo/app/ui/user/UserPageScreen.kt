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
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class UserPageViewModel(private val userId: String, startOnMessages: Boolean) : ViewModel() {
    private val repo = ServiceLocator.repository
    var me by mutableStateOf<UserProfile?>(null); private set
    var user by mutableStateOf<UserProfile?>(null); private set
    var shared by mutableStateOf<List<DataItem>>(emptyList()); private set
    var messages by mutableStateOf<List<Message>>(emptyList()); private set
    var tab by mutableIntStateOf(if (startOnMessages) 1 else 0)
    var draft by mutableStateOf("")
    var error by mutableStateOf<String?>(null); private set

    init {
        load()
        // Live messages from the WebSocket (design doc 4.4).
        viewModelScope.launch {
            repo.events.collect { e ->
                if (e is RealtimeEvent.NewMessage && e.message.senderId == userId && messages.none { it.id == e.message.id }) {
                    messages = messages + e.message
                }
            }
        }
    }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            me = repo.me()
            user = repo.user(userId)
            if (user!!.connected) {
                shared = repo.sharedItems(userId)
                messages = repo.messages(userId)
            }
        }.onFailure { error = "Couldn't load this profile. Try again." }
    }

    fun connect() = viewModelScope.launch { runCatching { repo.connect(userId) }.onSuccess { load() } }

    fun send() {
        val body = draft.trim().ifEmpty { return }
        draft = ""
        viewModelScope.launch {
            runCatching { repo.sendMessage(userId, body) }
                .onSuccess { m -> if (messages.none { it.id == m.id }) messages = messages + m }
                .onFailure { draft = body; error = null }
        }
    }
}

@Composable
fun UserPageScreen(
    userId: String,
    startOnMessages: Boolean,
    onBack: () -> Unit,
    onOpenItem: (String) -> Unit,
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
                            listOf("Shared items · ${vm.shared.size}", "Messages"), vm.tab, { vm.tab = it },
                            Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 8.dp),
                        )
                        if (vm.tab == 0) SharedItems(vm.shared, onOpenItem)
                        else Conversation(vm.messages, vm.me?.id, vm.draft, { vm.draft = it }, vm::send)
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

@Composable
private fun ColumnScope.Conversation(messages: List<Message>, myId: String?, draft: String, onDraft: (String) -> Unit, onSend: () -> Unit) {
    val listState = rememberLazyListState()
    LaunchedEffect(messages.size) { if (messages.isNotEmpty()) listState.animateScrollToItem(messages.lastIndex) }
    LazyColumn(
        state = listState, modifier = Modifier.weight(1f),
        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        items(messages, key = { it.id }) { m ->
            val mine = m.senderId == myId
            Box(Modifier.fillMaxWidth(), contentAlignment = if (mine) Alignment.CenterEnd else Alignment.CenterStart) {
                Column(
                    Modifier.widthIn(max = 280.dp)
                        .clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp, bottomStart = if (mine) 16.dp else 4.dp, bottomEnd = if (mine) 4.dp else 16.dp))
                        .background(if (mine) Sage.Primary else Sage.Surface).padding(horizontal = 14.dp, vertical = 10.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Text(m.body, style = HType.body.copy(fontSize = 14.sp),
                        color = if (mine) Color.White else Sage.Ink)
                    Text(if (mine) "You · ${m.sentAt}" else m.sentAt, style = HType.tiny.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Normal),
                        color = if (mine) Sage.OnPrimaryLine else Sage.Muted)
                }
            }
        }
    }
    Row(
        Modifier.fillMaxWidth().background(Sage.Surface).padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = draft, onValueChange = onDraft, modifier = Modifier.weight(1f),
            placeholder = { Text("Write a message", style = HType.body, color = Sage.Placeholder) },
            textStyle = HType.body, shape = RoundedCornerShape(22.dp), maxLines = 4,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send), keyboardActions = KeyboardActions(onSend = { onSend() }),
            colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = Sage.Primary, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Primary),
        )
        FilledIconButton(onClick = onSend, enabled = draft.isNotBlank(), modifier = Modifier.size(48.dp),
            colors = IconButtonDefaults.filledIconButtonColors(containerColor = Sage.Primary)) {
            Icon(Icons.AutoMirrored.Outlined.Send, contentDescription = "Send message", tint = Color.White)
        }
    }
}
