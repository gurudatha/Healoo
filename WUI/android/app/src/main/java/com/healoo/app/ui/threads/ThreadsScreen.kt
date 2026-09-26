package com.healoo.app.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.ServiceLocator
import com.healoo.app.data.ThreadSummary
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class ThreadsViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var threads by mutableStateOf<List<ThreadSummary>?>(null); private set
    init {
        load()
        viewModelScope.launch { repo.events.collect { if (it is com.healoo.app.data.RealtimeEvent.NewMessage) load() } }
    }
    private fun load() { viewModelScope.launch { threads = runCatching { repo.threads() }.getOrDefault(threads ?: emptyList()) } }
}

@Composable
fun ThreadsScreen(onOpenThread: (userId: String) -> Unit, onTab: (Tab) -> Unit, vm: ThreadsViewModel = viewModel()) {
    Scaffold(containerColor = Sage.Background, bottomBar = { HealooBottomBar(Tab.MESSAGES, onTab) }) { padding ->
        Column(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            Box(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(bottomStart = Radius.header, bottomEnd = Radius.header))
                    .background(Sage.Primary).statusBarsPadding().padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 20.dp),
            ) { Text("Messages", style = HType.screenTitle, color = Color.White) }

            LazyColumn(contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val list = vm.threads
                when {
                    list == null -> item { LoadingBox() }
                    list.isEmpty() -> item {
                        Text("No conversations yet. Open a doctor's profile from Search to send the first message.",
                            style = HType.body, color = Sage.Muted)
                    }
                    else -> items(list, key = { it.id }) { t ->
                        Row(
                            Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface)
                                .clickable { onOpenThread(t.other.id) }.padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Avatar(t.other.initials, 44.dp)
                            Column(Modifier.weight(1f)) {
                                Text(t.other.displayName, style = HType.bodyStrong, color = Sage.Ink)
                                Text(t.lastMessage, style = HType.caption, color = Sage.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                Text(t.lastMessageAt, style = HType.small, color = Sage.Muted)
                                if (t.unread > 0) Box(Modifier.size(20.dp).clip(CircleShape).background(Sage.Primary), contentAlignment = Alignment.Center) {
                                    Text("${t.unread}", style = HType.tiny, color = Color.White)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
