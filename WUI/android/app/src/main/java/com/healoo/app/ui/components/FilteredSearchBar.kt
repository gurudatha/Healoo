package com.healoo.app.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.healoo.app.data.Role
import com.healoo.app.data.ServiceLocator
import com.healoo.app.data.UserProfile
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.delay

/** A chip above the Filtered Search Bar that narrows the candidates. */
data class SearchFilter<T>(val label: String, val test: (T) -> Boolean)

/**
 * Filtered Search Bar: a text field with optional filter chips and a result list underneath.
 * Candidates are filtered on the device; [remote] (when given) adds matches from the server once
 * two or more characters are typed. Picking a result calls [onPick] and clears the text.
 * Results are a plain Column so the bar can sit inside a LazyColumn or a sheet.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun <T> FilteredSearchBar(
    placeholder: String,
    candidates: List<T>,
    key: (T) -> String,
    matches: (T, String) -> Boolean,
    onPick: (T) -> Unit,
    modifier: Modifier = Modifier,
    filters: List<SearchFilter<T>> = emptyList(),
    exclude: Set<String> = emptySet(),
    remote: (suspend (String) -> List<T>)? = null,
    showAllWhenBlank: Boolean = false,
    maxResults: Int = 8,
    focusRequester: FocusRequester = remember { FocusRequester() },
    emptyText: String = "No matches.",
    row: @Composable (T) -> Unit,
) {
    var query by remember { mutableStateOf("") }
    var filter by remember { mutableStateOf<SearchFilter<T>?>(null) }
    var fetched by remember { mutableStateOf<List<T>>(emptyList()) }

    LaunchedEffect(query, remote) {
        fetched = emptyList()
        if (remote != null && query.trim().length >= 2) {
            delay(300)
            fetched = runCatching { remote(query.trim()) }.getOrDefault(emptyList())
        }
    }

    val q = query.trim()
    val results = remember(q, filter, candidates, fetched, exclude) {
        (candidates + fetched).distinctBy(key)
            .filter { key(it) !in exclude }
            .filter { filter?.test?.invoke(it) ?: true }
            .filter { q.isEmpty() || matches(it, q.lowercase()) }
            .take(maxResults)
    }

    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OutlinedTextField(
            value = query, onValueChange = { query = it }, singleLine = true,
            placeholder = { Text(placeholder, style = HType.body, color = Sage.Placeholder) },
            leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null, tint = Sage.Muted) },
            trailingIcon = if (query.isNotEmpty()) ({
                IconButton({ query = "" }) { Icon(Icons.Outlined.Close, contentDescription = "Clear search", tint = Sage.Muted) }
            }) else null,
            textStyle = HType.body.copy(color = Sage.Ink), shape = RoundedCornerShape(Radius.field),
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = Sage.Surface, unfocusedContainerColor = Sage.Surface,
                focusedBorderColor = Sage.Primary, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Primary,
            ),
            modifier = Modifier.fillMaxWidth().focusRequester(focusRequester),
        )
        if (filters.isNotEmpty()) Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SageChip("All", filter == null, { filter = null })
            filters.forEach { f -> SageChip(f.label, filter == f, { filter = f }) }
        }
        if (q.isNotEmpty() || showAllWhenBlank) {
            if (results.isEmpty()) Text(emptyText, style = HType.caption, color = Sage.Muted, modifier = Modifier.padding(vertical = 4.dp))
            else GroupCard {
                results.forEachIndexed { i, r ->
                    if (i > 0) RowDivider()
                    Box(Modifier.fillMaxWidth().clickable { onPick(r); query = "" }) { row(r) }
                }
            }
        }
    }
}

/** A person as a Filtered Search Bar result. */
@Composable
fun UserResultRow(user: UserProfile, trailing: String? = null) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Avatar(user.initials, 36.dp)
        Column(Modifier.weight(1f)) {
            Text(user.displayName, style = HType.bodyStrong, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(listOf(user.headline, user.publicId).filter { it.isNotBlank() }.joinToString(" · "),
                style = HType.small, color = Sage.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        trailing?.let { Text(it, style = HType.small, color = Sage.Primary) }
    }
}

fun UserProfile.matchesQuery(q: String) = displayName.lowercase().contains(q) || publicId.lowercase().contains(q)

private val recipientFilters = listOf(
    SearchFilter<UserProfile>("Doctors") { it.primaryRole == Role.DOCTOR },
    SearchFilter("Hospitals") { it.primaryRole == Role.HOSPITAL },
    SearchFilter("Labs") { it.primaryRole == Role.LAB },
    SearchFilter("Users") { it.primaryRole == Role.PATIENT },
)

/**
 * Who something goes to: [fixed] is the person whose page this started from (always included,
 * can't be removed); more people are added with the Filtered Search Bar. [doctorsOnly] limits the
 * extra people to doctors (when passing on something the viewer doesn't own).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RecipientField(
    fixed: UserProfile?,
    extras: List<UserProfile>,
    onAdd: (UserProfile) -> Unit,
    onRemove: (UserProfile) -> Unit,
    contacts: List<UserProfile>,
    doctorsOnly: Boolean = false,
    exclude: Set<String> = emptySet(),
    label: String = "To",
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldLabel(label)
        if (fixed != null || extras.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            fixed?.let { RecipientChip(it, onRemove = null) }
            extras.forEach { u -> RecipientChip(u, onRemove = { onRemove(u) }) }
        }
        val allowed: (UserProfile) -> Boolean = { !doctorsOnly || it.primaryRole == Role.DOCTOR }
        FilteredSearchBar(
            placeholder = if (doctorsOnly) "Add a doctor by name or Healoo ID" else "Add someone by name or Healoo ID",
            candidates = contacts.filter(allowed),
            key = { it.id },
            matches = { u, q -> u.matchesQuery(q) },
            onPick = onAdd,
            filters = if (doctorsOnly) emptyList() else recipientFilters,
            exclude = exclude + extras.map { it.id } + listOfNotNull(fixed?.id),
            remote = { q -> ServiceLocator.repository.search(q, if (doctorsOnly) Role.DOCTOR else null).filter(allowed) },
            emptyText = if (doctorsOnly) "No doctor matches." else "No one matches.",
        ) { UserResultRow(it, trailing = "Add") }
    }
}

@Composable
private fun RecipientChip(user: UserProfile, onRemove: (() -> Unit)?) {
    Surface(
        shape = RoundedCornerShape(Radius.chip), color = if (onRemove == null) Sage.SageTint else Sage.Surface,
        border = BorderStroke(1.dp, if (onRemove == null) Sage.SageTint else Sage.Border),
    ) {
        Row(Modifier.heightIn(min = 36.dp).padding(start = 6.dp, end = if (onRemove == null) 12.dp else 2.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Avatar(user.initials, 26.dp)
            Text(user.displayName, style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Ink, maxLines = 1)
            if (onRemove == null) Icon(Icons.Outlined.Lock, contentDescription = "Always included", tint = Sage.Primary, modifier = Modifier.size(14.dp))
            else IconButton(onRemove, Modifier.size(32.dp)) {
                Icon(Icons.Outlined.Close, contentDescription = "Remove ${user.displayName}", tint = Sage.Muted, modifier = Modifier.size(16.dp))
            }
        }
    }
}

/** An item as a Filtered Search Bar result (Share a document, Add to an existing item). */
@Composable
fun ItemResultRow(item: com.healoo.app.data.DataItem, selected: Boolean = false) {
    Row(
        Modifier.fillMaxWidth().background(if (selected) Sage.SageTint else Sage.Surface).padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TypeTile(item.primaryKind, 36.dp)
        Column(Modifier.weight(1f)) {
            Text(item.title, style = HType.bodyStrong, color = Sage.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(item.primaryKind.label + (item.date.takeIf { it.isNotEmpty() }?.let { " · ${shortDate(it)}" } ?: ""),
                style = HType.small, color = Sage.Muted, maxLines = 1)
        }
    }
}

/** A selected value shown in place of a Filtered Search Bar, with a way to change it. */
@Composable
fun PickedRow(content: @Composable RowScope.() -> Unit, onChange: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.SageTint).padding(start = 4.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, content = content)
        TextButton(onChange) { Text("Change", style = HType.caption.copy(fontWeight = FontWeight.SemiBold), color = Sage.Primary) }
    }
}
