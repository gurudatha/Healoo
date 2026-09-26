package com.healoo.app.ui.sharing

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.LocalHospital
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

class ActiveSharingViewModel : ViewModel() {
    private val repo = ServiceLocator.repository
    var groups by mutableStateOf<List<ShareGroup>?>(null); private set
    var error by mutableStateOf<String?>(null); private set

    init { load() }

    fun load() = viewModelScope.launch {
        error = null
        runCatching { groups = repo.activeShares() }.onFailure { error = "Couldn't load what you've shared. Try again." }
    }

    fun revoke(grants: List<OwnedGrant>) = viewModelScope.launch {
        grants.forEach { g -> runCatching { repo.revokeGrant(g.itemId, g.grantId) } }
        load()
    }
}

/** Settings → Active sharing: who can see which of your items, and a one-tap way to stop it. */
@Composable
fun ActiveSharingScreen(onBack: () -> Unit, onOpenItem: (String) -> Unit, vm: ActiveSharingViewModel = viewModel()) {
    var confirm by remember { mutableStateOf<Pair<String, List<OwnedGrant>>?>(null) }

    Scaffold(
        containerColor = Sage.Background,
        topBar = {
            val total = vm.groups?.sumOf { it.grants.size }
            PinnedHeader("Active sharing", total?.let { "$it share${if (it == 1) "" else "s"} across ${vm.groups!!.size} contacts" } ?: "", onBack)
        },
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            val groups = vm.groups
            when {
                vm.error != null -> item { ErrorBox(vm.error!!, vm::load) }
                groups == null -> item { LoadingBox() }
                groups.isEmpty() -> item {
                    Text("You haven't shared anything. Open an item and choose Share with… to give a doctor or hospital access.",
                        style = HType.body, color = Sage.Muted)
                }
                else -> items(groups, key = { it.granteeId }) { g ->
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            val hospital = g.granteeType == GranteeType.HOSPITAL
                            Box(Modifier.size(32.dp).clip(RoundedCornerShape(10.dp)).background(Sage.SageTint), contentAlignment = Alignment.Center) {
                                Icon(if (hospital) Icons.Outlined.LocalHospital else Icons.Outlined.Person, null, tint = Sage.Primary, modifier = Modifier.size(17.dp))
                            }
                            Column(Modifier.weight(1f)) {
                                Text(g.granteeName, style = HType.bodyStrong, color = Sage.Ink)
                                Text(if (hospital) "All affiliated doctors · ${g.grants.size} items" else "${g.grants.size} items",
                                    style = HType.small, color = Sage.Muted)
                            }
                            TextButton({ confirm = g.granteeName to g.grants }) {
                                Text("Stop all", style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Clay)
                            }
                        }
                        GroupCard {
                            g.grants.forEachIndexed { i, grant ->
                                if (i > 0) RowDivider()
                                Row(
                                    Modifier.fillMaxWidth().clickable { onOpenItem(grant.itemId) }.heightIn(min = 56.dp).padding(start = 12.dp),
                                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    TypeTile(grant.type, 32.dp)
                                    Text(grant.itemTitle, style = HType.body, color = Sage.Ink, modifier = Modifier.weight(1f),
                                        maxLines = 1, overflow = TextOverflow.Ellipsis)
                                    TextButton({ confirm = g.granteeName to listOf(grant) }) {
                                        Text("Revoke", style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Clay)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    confirm?.let { (name, grants) ->
        val all = grants.size > 1
        AlertDialog(
            onDismissRequest = { confirm = null }, containerColor = Sage.Surface,
            title = { Text(if (all) "Stop sharing everything with $name?" else "Stop sharing “${grants.first().itemTitle}”?", style = HType.section) },
            text = { Text(if (all) "$name loses access to ${grants.size} items straight away." else "$name loses access straight away. You can share it again later.", style = HType.body) },
            confirmButton = { TextButton({ vm.revoke(grants); confirm = null }) { Text("Stop sharing", color = Sage.Clay) } },
            dismissButton = { TextButton({ confirm = null }) { Text("Keep sharing", color = Sage.Primary) } },
        )
    }
}
