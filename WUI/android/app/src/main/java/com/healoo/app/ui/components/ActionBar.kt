package com.healoo.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import com.healoo.app.ui.icons.automirrored.outlined.Chat
import androidx.compose.material.icons.outlined.*
import com.healoo.app.ui.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.healoo.app.data.PageOperation
import com.healoo.app.data.ResolvedOperations
import com.healoo.app.ui.theme.*

/** Icon keys used in page-operations.json. */
fun operationIcon(key: String): ImageVector = when (key) {
    "home" -> Icons.Outlined.Home
    "search" -> Icons.Outlined.Search
    "upload" -> Icons.Outlined.FileUpload
    "chat" -> Icons.AutoMirrored.Outlined.Chat
    "settings" -> Icons.Outlined.Settings
    "person_add" -> Icons.Outlined.PersonAddAlt
    "history" -> Icons.Outlined.History
    "share" -> Icons.Outlined.Share
    "event" -> Icons.Outlined.Event
    "doctor" -> Icons.Outlined.MedicalServices
    else -> Icons.Outlined.Circle
}

/**
 * The bottom bar for a page: at most maxVisible operations, then a three-dot button that lists
 * the rest. [selected] highlights the current operation (the global tabs).
 */
@Composable
fun OperationBar(ops: ResolvedOperations, onOperation: (String) -> Unit, selected: String? = null) {
    var more by remember { mutableStateOf(false) }
    Column(Modifier.background(Sage.Surface).navigationBarsPadding()) {
        HorizontalDivider(color = Sage.Divider)
        Row(Modifier.fillMaxWidth().height(72.dp), horizontalArrangement = Arrangement.SpaceAround, verticalAlignment = Alignment.CenterVertically) {
            ops.visible.forEach { op -> BarButton(op.label, operationIcon(op.icon), op.id == selected) { onOperation(op.id) } }
            if (ops.overflow.isNotEmpty()) Box {
                BarButton("More", Icons.Outlined.MoreHoriz, ops.overflow.any { it.id == selected }) { more = true }
                DropdownMenu(more, { more = false }, containerColor = Sage.Surface) {
                    ops.overflow.forEach { op -> OverflowItem(op) { more = false; onOperation(op.id) } }
                }
            }
        }
    }
}

@Composable
private fun BarButton(label: String, icon: ImageVector, on: Boolean, onClick: () -> Unit) {
    val color = if (on) Sage.Primary else Sage.Muted
    Column(
        Modifier.width(68.dp).height(56.dp).clip(RoundedCornerShape(12.dp))
            .clickable(role = Role.Tab, onClick = onClick).semantics { selected = on },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(22.dp))
        Spacer(Modifier.height(4.dp))
        Text(label, style = HType.tiny, color = color, maxLines = 1)
    }
}

@Composable
private fun OverflowItem(op: PageOperation, onClick: () -> Unit) = DropdownMenuItem(
    text = { Text(op.label, style = HType.body, color = Sage.Ink) },
    leadingIcon = { Icon(operationIcon(op.icon), contentDescription = null, tint = Sage.Primary) },
    onClick = onClick,
)
