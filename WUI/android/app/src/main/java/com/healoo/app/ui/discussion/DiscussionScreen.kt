package com.healoo.app.ui.discussion

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Send
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.item.senderName
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

/** The messages of one DataItem (DataItem_Design.md D5: every message belongs to an item). */
class DiscussionViewModel(private val itemId: String) : ViewModel() {
    private val repo = ServiceLocator.repository
    var item by mutableStateOf<DataItem?>(null); private set
    var me by mutableStateOf<UserProfile?>(null); private set
    var messages by mutableStateOf<List<Message>>(emptyList()); private set
    var draft by mutableStateOf("")
    var error by mutableStateOf<String?>(null); private set

    init {
        load()
        viewModelScope.launch {
            repo.events.collect { e ->
                if (e is RealtimeEvent.NewMessage && e.message.itemId == itemId && messages.none { it.id == e.message.id }) {
                    messages = messages + e.message
                }
            }
        }
    }

    fun load() = viewModelScope.launch {
        error = null
        runCatching {
            me = repo.me()
            item = repo.item(itemId)
            messages = repo.itemMessages(itemId)          // also marks them read
        }.onFailure { error = "Couldn't open this discussion." }
    }

    fun send() {
        val body = draft.trim().ifEmpty { return }
        draft = ""
        viewModelScope.launch {
            runCatching { repo.sendItemMessage(itemId, body) }
                .onSuccess { m -> if (messages.none { it.id == m.id }) messages = messages + m }
                .onFailure { draft = body; error = it.message ?: "Couldn't send. Try again." }
        }
    }
}

@Composable
fun DiscussionScreen(itemId: String, onBack: () -> Unit, onOpenItem: (String) -> Unit) {
    val vm: DiscussionViewModel = viewModel(key = "discussion-$itemId") { DiscussionViewModel(itemId) }
    val item = vm.item
    val me = vm.me
    Scaffold(
        containerColor = Sage.Background,
        topBar = {
            PinnedHeader(
                title = item?.title ?: "Discussion",
                subtitle = item?.let { "${it.primaryKind.label} · tap for details" } ?: "",
                onBack = onBack,
                trailing = { if (item != null) TextButton({ onOpenItem(itemId) }) { Text("Details", color = Color.White) } },
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
            when {
                item == null && vm.error != null -> ErrorBox(vm.error!!, vm::load)
                item == null || me == null -> LoadingBox()
                else -> {
                    val listState = rememberLazyListState()
                    LaunchedEffect(vm.messages.size) { if (vm.messages.isNotEmpty()) listState.animateScrollToItem(vm.messages.lastIndex) }
                    LazyColumn(
                        state = listState, modifier = Modifier.weight(1f),
                        contentPadding = PaddingValues(horizontal = 20.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        if (vm.messages.isEmpty()) item {
                            Text("No messages yet. Write the first one below.", style = HType.body, color = Sage.Muted)
                        }
                        items(vm.messages, key = { it.id }) { m -> Bubble(m, mine = m.senderId == me.id, name = senderName(item, m.senderId, me.id)) }
                    }
                    if (item.can("message")) Composer(vm.draft, { vm.draft = it }, vm::send)
                    else Text(if (item.status == ItemStatus.CLOSED) "This item is closed. Reopen it to continue the discussion." else "You can read this discussion.",
                        style = HType.small, color = Sage.Muted, modifier = Modifier.fillMaxWidth().background(Sage.Surface).padding(16.dp))
                }
            }
        }
    }
}

@Composable
private fun Bubble(m: Message, mine: Boolean, name: String) {
    Box(Modifier.fillMaxWidth(), contentAlignment = if (mine) Alignment.CenterEnd else Alignment.CenterStart) {
        Column(
            Modifier.widthIn(max = 280.dp)
                .clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp, bottomStart = if (mine) 16.dp else 4.dp, bottomEnd = if (mine) 4.dp else 16.dp))
                .background(if (mine) Sage.Primary else Sage.Surface).padding(horizontal = 14.dp, vertical = 10.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(m.body, style = HType.body.copy(fontSize = 14.sp), color = if (mine) Color.White else Sage.Ink)
            Text("$name · ${m.sentAt.drop(11).take(5)}", style = HType.tiny.copy(fontWeight = FontWeight.Normal),
                color = if (mine) Sage.OnPrimaryLine else Sage.Muted)
        }
    }
}

@Composable
fun Composer(draft: String, onDraft: (String) -> Unit, onSend: () -> Unit, placeholder: String = "Write a message") {
    Row(
        Modifier.fillMaxWidth().background(Sage.Surface).navigationBarsPadding().imePadding().padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = draft, onValueChange = onDraft, modifier = Modifier.weight(1f),
            placeholder = { Text(placeholder, style = HType.body, color = Sage.Placeholder) },
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

/** One conversation row (Messages tab and the Messages section of a person's page). */
@Composable
fun ConversationRow(c: Conversation, onClick: () -> Unit, showPerson: Boolean = true) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface).clickable(onClick = onClick).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (showPerson) Avatar(c.other.initials, 44.dp) else TypeTile(c.primaryKind)
        Column(Modifier.weight(1f)) {
            Text(if (showPerson) c.other.displayName else c.itemTitle, style = HType.bodyStrong, color = Sage.Ink, maxLines = 1)
            if (showPerson) Text(c.itemTitle, style = HType.small.copy(fontWeight = FontWeight.SemiBold), color = Sage.Primary, maxLines = 1)
            Text(c.lastMessage, style = HType.caption, color = Sage.Muted, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
        }
        Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(shortDate(c.lastMessageAt.take(10)), style = HType.small, color = Sage.Muted)
            if (c.unread > 0) Box(Modifier.size(20.dp).clip(androidx.compose.foundation.shape.CircleShape).background(Sage.Primary), contentAlignment = Alignment.Center) {
                Text("${c.unread}", style = HType.tiny, color = Color.White)
            }
        }
    }
}
