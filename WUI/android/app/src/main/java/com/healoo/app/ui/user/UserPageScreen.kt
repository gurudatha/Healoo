package com.healoo.app.ui.user

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.healoo.app.data.*
import com.healoo.app.ui.components.*
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
    /** The caller's contacts, offered when adding more recipients. */
    var contacts by mutableStateOf<List<UserProfile>>(emptyList()); private set
    /** Hospital page: doctors working there. */
    var doctors by mutableStateOf<List<UserProfile>>(emptyList()); private set
    /** Items the caller may share (Share a document). */
    var shareable by mutableStateOf<List<DataItem>>(emptyList()); private set
    var tab by mutableIntStateOf(if (startOnMessages) 1 else 0)
    var error by mutableStateOf<String?>(null); private set
    var notice by mutableStateOf<String?>(null)

    val page: PageKind get() = user?.let(PageOperations::pageFor) ?: PageKind.USER
    val operations: ResolvedOperations get() = PageOperations.resolve(page, OpFact.of(me, user))

    // Message and Share sheets: the page's person is always a recipient; these are the extra ones.
    val extraRecipients = mutableStateListOf<UserProfile>()
    var draft by mutableStateOf("")
    var picked by mutableStateOf<DataItem?>(null)
    var busy by mutableStateOf(false); private set
    var sheetError by mutableStateOf<String?>(null)

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
            val u = repo.user(userId)
            user = u
            if (PageOperations.pageFor(u) == PageKind.HOSPITAL) doctors = repo.hospitalDoctors(u.id)
            if (u.connected) {
                shared = repo.sharedItems(userId)
                conversations = repo.conversations(userId)
            }
            contacts = repo.connections()
        }.onFailure { error = "Couldn't load this profile. Try again." }
    }

    private fun refreshConversations() = viewModelScope.launch {
        runCatching { repo.conversations(userId) }.onSuccess { conversations = it }
    }

    fun connect() = viewModelScope.launch { runCatching { repo.connect(userId) }.onSuccess { load() } }

    fun openSheet() {
        extraRecipients.clear(); draft = ""; picked = null; sheetError = null
    }

    fun loadShareable() = viewModelScope.launch {
        runCatching { repo.items(ItemStatus.OPEN, 100) }
            .onSuccess { list -> shareable = list.filter { it.can("share") && it.ownerId != userId } }
    }

    /** Passing on an item the caller doesn't own is allowed to doctors only (server rule). */
    val pickedIsReferral: Boolean get() = picked?.let { it.ownerId != me?.id } ?: false

    /** Starts a new MESSAGE item with this person (and anyone added); returns its id for navigation. */
    fun sendMessage(onStarted: (String) -> Unit) {
        val body = draft.trim().ifEmpty { sheetError = "Write a message first."; return }
        busy = true
        viewModelScope.launch {
            runCatching { repo.startConversation(userId, body, extraRecipients.map { it.id }) }
                .onSuccess { onStarted(it.id); refreshConversations() }
                .onFailure { sheetError = "That didn't send: ${it.message}. Try again." }
            busy = false
        }
    }

    fun share(onDone: () -> Unit) {
        val item = picked ?: run { sheetError = "Choose a document to share."; return }
        val people = listOfNotNull(user) + extraRecipients
        if (pickedIsReferral && people.any { it.primaryRole != Role.DOCTOR }) {
            sheetError = "You don't own \"${item.title}\", so you can pass it on to doctors only."; return
        }
        busy = true
        viewModelScope.launch {
            runCatching { people.forEach { repo.share(item.id, it.id) } }
                .onSuccess {
                    notice = "Shared \"${item.title}\" with ${people.joinToString { it.displayName }}."
                    runCatching { repo.sharedItems(userId) }.onSuccess { shared = it }
                    tab = 0
                    onDone()
                }
                .onFailure { sheetError = "That didn't share: ${it.message}." }
            busy = false
        }
    }
}

private enum class Sheet { MESSAGE, SHARE, BOOK_AT_HOSPITAL }

@Composable
fun UserPageScreen(
    userId: String,
    startOnMessages: Boolean,
    onBack: () -> Unit,
    onOpenItem: (String) -> Unit,
    onOpenDiscussion: (String) -> Unit,
    onOpenUser: (String) -> Unit,
    onUploadFor: (userId: String) -> Unit,
    onBook: (doctorId: String, hospitalId: String?) -> Unit,
) {
    val vm: UserPageViewModel = viewModel(key = "user-$userId") { UserPageViewModel(userId, startOnMessages) }
    val user = vm.user
    var sheet by remember { mutableStateOf<Sheet?>(null) }
    val doctorSearch = remember { FocusRequester() }

    fun perform(op: String) {
        val u = vm.user ?: return
        when (op) {
            OpId.CONNECT -> vm.connect()
            OpId.MESSAGE -> { vm.openSheet(); sheet = Sheet.MESSAGE }
            OpId.HISTORY -> vm.tab = 0
            OpId.SHARE_DOCUMENT -> { vm.openSheet(); vm.loadShareable(); sheet = Sheet.SHARE }
            OpId.UPLOAD_FOR -> onUploadFor(u.id)
            OpId.BOOK_APPOINTMENT -> if (vm.page == PageKind.HOSPITAL) sheet = Sheet.BOOK_AT_HOSPITAL else onBook(u.id, null)
            OpId.SEARCH_DOCTORS -> runCatching { doctorSearch.requestFocus() }
        }
    }

    Scaffold(
        containerColor = Sage.Background,
        bottomBar = { if (user != null) OperationBar(vm.operations, ::perform) },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            when {
                vm.error != null -> Column(Modifier.statusBarsPadding()) { ErrorBox(vm.error!!, vm::load) }
                user == null -> LoadingBox(Modifier.statusBarsPadding())
                else -> CollapsingHeaderLayout(
                    expandedFraction = HeaderRatio.USER_EXPANDED,
                    collapsedFraction = HeaderRatio.USER_COLLAPSED,
                    header = { progress -> ProfileHeader(user, progress, onBack) },
                ) {
                    vm.notice?.let { Notice(it) { vm.notice = null } }
                    when {
                        vm.page == PageKind.HOSPITAL -> HospitalDoctors(vm.doctors, doctorSearch, onOpenUser)
                        !user.connected -> NotConnected(user)
                        else -> {
                            // History with this person: what is shared between you, and your discussions.
                            Segmented(
                                listOf("Shared items · ${vm.shared.size}", "Messages · ${vm.conversations.size}"), vm.tab, { vm.tab = it },
                                Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 8.dp),
                            )
                            if (vm.tab == 0) SharedItems(vm.shared, onOpenItem)
                            else Conversations(vm.conversations, onOpenDiscussion)
                        }
                    }
                }
            }
        }
    }

    if (user != null) when (sheet) {
        Sheet.MESSAGE -> MessageSheet(vm, user, onDismiss = { sheet = null }) { id -> sheet = null; onOpenDiscussion(id) }
        Sheet.SHARE -> ShareSheet(vm, user, onDismiss = { sheet = null })
        Sheet.BOOK_AT_HOSPITAL -> BookAtHospitalSheet(user, vm.doctors, onDismiss = { sheet = null }) { d -> sheet = null; onBook(d.id, user.id) }
        null -> {}
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
private fun Notice(text: String, onDismiss: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 12.dp).clip(RoundedCornerShape(12.dp)).background(Sage.SageTint)
            .padding(start = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = HType.caption, color = Sage.Primary, modifier = Modifier.weight(1f))
        TextButton(onDismiss) { Text("OK", color = Sage.Primary) }
    }
}

@Composable
private fun NotConnected(user: UserProfile) {
    Text("You can see ${user.displayName}'s profile. Add them to your contacts (below) to message them; " +
        "records appear only after the owner shares them.", style = HType.body, color = Sage.InkSoft, modifier = Modifier.padding(20.dp))
}

@Composable
private fun SharedItems(items: List<DataItem>, onOpenItem: (String) -> Unit) {
    ScrollbarLazyColumn(
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 8.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (items.isEmpty()) item {
            Text("Nothing shared between you yet.", style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 12.dp))
        }
        items(items, key = { it.id }) { DataItemRow(it, onClick = { onOpenItem(it.id) }) }
    }
}

/** Discussions with this person, newest first. New ones start from the Message operation. */
@Composable
private fun Conversations(list: List<Conversation>, onOpen: (String) -> Unit) {
    ScrollbarLazyColumn(
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 8.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (list.isEmpty()) item {
            Text("No discussions yet. Tap Message below to start one — it becomes an item you can both add to.",
                style = HType.body, color = Sage.Muted, modifier = Modifier.padding(vertical = 12.dp))
        }
        items(list, key = { it.itemId }) { c -> ConversationRow(c, onClick = { onOpen(c.itemId) }, showPerson = false) }
    }
}

/** Hospital page: its doctors, narrowed with the Filtered Search Bar. */
@Composable
private fun HospitalDoctors(doctors: List<UserProfile>, focus: FocusRequester, onOpenUser: (String) -> Unit) {
    ScrollbarLazyColumn(contentPadding = PaddingValues(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item { SectionHeader("Doctors · ${doctors.size}") }
        item {
            FilteredSearchBar(
                placeholder = "Search doctors by name or Healoo ID",
                candidates = doctors, key = { it.id }, matches = { u, q -> u.matchesQuery(q) || u.headline.lowercase().contains(q) },
                onPick = { onOpenUser(it.id) }, showAllWhenBlank = true, maxResults = 100, focusRequester = focus,
                emptyText = if (doctors.isEmpty()) "No doctors are listed for this hospital yet." else "No doctor matches.",
            ) { UserResultRow(it) }
        }
    }
}

// ------------------------------------------------------------------ sheets

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun OperationSheet(title: String, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Sage.Background, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(
            Modifier.fillMaxWidth().verticalScrollWithBar(rememberScrollState()).imePadding().padding(start = 20.dp, end = 20.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(title, style = HType.section, color = Sage.Ink)
            content()
        }
    }
}

@Composable
private fun MessageSheet(vm: UserPageViewModel, user: UserProfile, onDismiss: () -> Unit, onStarted: (String) -> Unit) {
    // A clinician writing to a patient starts the discussion on the patient's behalf; only doctors can be added.
    val doctorsOnly = vm.me?.isClinical == true && user.primaryRole == Role.PATIENT
    OperationSheet("New message", onDismiss) {
        RecipientField(user, vm.extraRecipients, { vm.extraRecipients += it }, { vm.extraRecipients -= it }, vm.contacts,
            doctorsOnly = doctorsOnly, exclude = setOfNotNull(vm.me?.id))
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            FieldLabel("Message")
            OutlinedTextField(
                value = vm.draft, onValueChange = { vm.draft = it; vm.sheetError = null }, minLines = 3, maxLines = 8,
                placeholder = { Text("Write to ${user.displayName.substringBefore(' ')}", style = HType.body, color = Sage.Placeholder) },
                textStyle = HType.body.copy(color = Sage.Ink), shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth(),
                colors = OutlinedTextFieldDefaults.colors(focusedContainerColor = Sage.Surface, unfocusedContainerColor = Sage.Surface,
                    focusedBorderColor = Sage.Primary, unfocusedBorderColor = Sage.Border, cursorColor = Sage.Primary),
            )
            Text("This starts a new discussion item that everyone above can read and add to.", style = HType.small, color = Sage.Muted)
        }
        vm.sheetError?.let { Text(it, style = HType.small, color = Sage.Clay) }
        PrimaryButton(if (vm.busy) "Sending…" else "Send", { vm.sendMessage(onStarted) }, Modifier.fillMaxWidth(), enabled = !vm.busy)
    }
}

@Composable
private fun ShareSheet(vm: UserPageViewModel, user: UserProfile, onDismiss: () -> Unit) {
    val kinds = remember { PrimaryKind.entries.map { k -> SearchFilter<DataItem>(k.label + "s") { it.primaryKind == k } } }
    OperationSheet("Share a document", onDismiss) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            FieldLabel("Document")
            val item = vm.picked
            if (item != null) PickedRow({ Box(Modifier.weight(1f)) { ItemResultRow(item, selected = true) } }, onChange = { vm.picked = null })
            else FilteredSearchBar(
                placeholder = "Search your items", candidates = vm.shareable, key = { it.id },
                matches = { it, q -> it.title.lowercase().contains(q) || it.keywords.any { k -> k.lowercase().contains(q) } },
                onPick = { vm.picked = it; vm.sheetError = null }, filters = kinds, showAllWhenBlank = true, maxResults = 6,
                emptyText = if (vm.shareable.isEmpty()) "You have no open items to share yet. New documents come in through Upload." else "No item matches.",
            ) { ItemResultRow(it) }
            if (vm.pickedIsReferral) Text("You don't own this item, so it can be passed on to doctors only. The owner sees who has it.",
                style = HType.small, color = Sage.SandInk)
        }
        RecipientField(user, vm.extraRecipients, { vm.extraRecipients += it }, { vm.extraRecipients -= it }, vm.contacts,
            doctorsOnly = vm.pickedIsReferral, exclude = setOfNotNull(vm.me?.id), label = "Share with")
        vm.sheetError?.let { Text(it, style = HType.small, color = Sage.Clay) }
        PrimaryButton(if (vm.busy) "Sharing…" else "Share", { vm.share(onDismiss) }, Modifier.fillMaxWidth(), enabled = !vm.busy && vm.picked != null)
    }
}

@Composable
private fun BookAtHospitalSheet(hospital: UserProfile, doctors: List<UserProfile>, onDismiss: () -> Unit, onPick: (UserProfile) -> Unit) {
    OperationSheet("Book at ${hospital.displayName}", onDismiss) {
        Text("Choose a doctor; you pick the date and time next.", style = HType.caption, color = Sage.Muted)
        FilteredSearchBar(
            placeholder = "Search doctors by name or Healoo ID",
            candidates = doctors, key = { it.id }, matches = { u, q -> u.matchesQuery(q) || u.headline.lowercase().contains(q) },
            onPick = onPick, showAllWhenBlank = true, maxResults = 100,
            emptyText = if (doctors.isEmpty()) "No doctors are listed for this hospital yet." else "No doctor matches.",
        ) { UserResultRow(it, trailing = "Book") }
    }
}
