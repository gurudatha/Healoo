package com.healoo.app.ui.conversations

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.Conversation
import com.healoo.app.data.RealtimeEvent
import com.healoo.app.data.ServiceLocator
import com.healoo.app.ui.components.*
import com.healoo.app.ui.discussion.ConversationRow
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

/** Messages tab: every item with a discussion, per person (replaces v0.1 chat threads). */
class ConversationsViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var list by mutableStateOf<List<Conversation>?>(null); private set
    init {
        load()
        viewModelScope.launch { repo.events.collect { if (it is RealtimeEvent.NewMessage) load() } }
    }
    fun load() { viewModelScope.launch { list = runCatching { repo.conversations() }.getOrDefault(list ?: emptyList()) } }
}

@Composable
fun ConversationsScreen(onOpenDiscussion: (itemId: String) -> Unit, onTab: (Tab) -> Unit, vm: ConversationsViewModel = viewModel()) {
    LaunchedEffect(Unit) { vm.load() }
    Scaffold(containerColor = Sage.Background, bottomBar = { HealooBottomBar(Tab.MESSAGES, onTab) }) { padding ->
        Column(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            Box(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(bottomStart = Radius.header, bottomEnd = Radius.header))
                    .background(Sage.Primary).statusBarsPadding().padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 20.dp),
            ) { Text("Messages", style = HType.screenTitle, color = Sage.OnPrimary) }

            ScrollbarLazyColumn(contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val l = vm.list
                when {
                    l == null -> item { LoadingBox() }
                    l.isEmpty() -> item {
                        Text("No conversations yet. Open a person's profile from Search to start one, or use Start discussion on any item.",
                            style = HType.body, color = Sage.Muted)
                    }
                    else -> items(l, key = { it.itemId + it.other.id }) { c -> ConversationRow(c, onClick = { onOpenDiscussion(c.itemId) }) }
                }
            }
        }
    }
}
