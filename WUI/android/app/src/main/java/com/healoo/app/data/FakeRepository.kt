package com.healoo.app.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.util.UUID

/**
 * Seeded demo backend. Everything is evaluated from the signed-in user's point of view,
 * so you can sign in as the patient, the doctor or the lab and test each flow.
 * Access follows design doc 2.3: owners see their items; others see an item only when it is
 * granted to them directly or to a hospital they are affiliated with.
 */
class FakeRepository : HealooRepository {

    private fun asset(name: String) = "file:///android_asset/sample/$name"

    private val users = mutableListOf(
        UserProfile("u-lakshmi", "HL-2M9P4", "Lakshmi K.", listOf(Role.PATIENT), "Patient", location = "[City]"),
        UserProfile("u-rao", "HL-7R2C9", "Dr. Anitha Rao", listOf(Role.DOCTOR), "Doctor · General Medicine",
            hospital = "Test Hospital A", officialNumber = "[REG-NO]", location = "[City]"),
        UserProfile("h-a", "HL-1H0A1", "Test Hospital A", listOf(Role.HOSPITAL), "Hospital"),
        UserProfile("l-city", "HL-6L3D2", "City Diagnostics", listOf(Role.LAB), "Lab"),
        UserProfile("u-srao", "HL-3K8M1", "Dr. Srinivas Rao", listOf(Role.DOCTOR), "Doctor · Independent"),
        UserProfile("h-raoheart", "HL-9P4T6", "Rao Heart Clinic", listOf(Role.HOSPITAL), "Hospital"),
        UserProfile("l-rao", "HL-5W1Q3", "Rao Diagnostics", listOf(Role.LAB), "Lab"),
        UserProfile("u-priya", "HL-4K7Q2", "Priya Rao", listOf(Role.PATIENT), "User"),
    )

    /** Accounts offered on the demo sign-in screen. */
    val demoAccounts: List<UserProfile> get() = users.filter { it.id in listOf("u-lakshmi", "u-rao", "l-city") }

    private var meId = "u-lakshmi"
    fun signInAs(userId: String) { meId = userId }

    private val affiliation = mapOf("u-rao" to "h-a")                       // doctor -> current hospital
    private val connections = mutableSetOf(
        setOf("u-lakshmi", "u-rao"), setOf("u-lakshmi", "h-a"), setOf("u-lakshmi", "l-city"),
        setOf("u-rao", "h-a"), setOf("u-rao", "l-city"), setOf("u-priya", "l-city"),
    )

    private fun raw(id: String) = users.first { it.id == id }
    private fun connected(a: String, b: String) = setOf(a, b) in connections
    private fun view(u: UserProfile) = u.copy(connected = connected(meId, u.id))
    private fun grantFor(u: UserProfile) = Grant(UUID.randomUUID().toString(),
        if (u.primaryRole == Role.HOSPITAL) GranteeType.HOSPITAL else GranteeType.USER, u.id, u.displayName)

    private fun canSee(item: DataItem, userId: String) = item.ownerId == userId || item.accessList.any {
        it.granteeId == userId || (it.granteeType == GranteeType.HOSPITAL && affiliation[userId] == it.granteeId)
    }

    private val items = mutableListOf(
        DataItem(
            id = "i-cbc", date = "2026-09-20", type = CoreItemType.REPORT,
            title = "CBC – Complete blood count", subtitle = "City Diagnostics · Lab report",
            keywords = listOf("CBC", "Haemoglobin", "Routine check"),
            attachments = listOf(
                Attachment(AttachmentKind.IMAGE, asset("cbc_scan_1.jpg"), "image/jpeg", 91_000, 0, "cbc_scan_1.jpg"),
                Attachment(AttachmentKind.IMAGE, asset("cbc_scan_2.jpg"), "image/jpeg", 92_000, 1, "cbc_scan_2.jpg"),
                Attachment(AttachmentKind.PDF, asset("cbc_report.pdf"), "application/pdf", 3_400, 2, "CBC_20Sep2026.pdf", pageCount = 3),
                Attachment(AttachmentKind.PDF, asset("reference_ranges.pdf"), "application/pdf", 2_500, 3, "Reference_ranges.pdf", pageCount = 2),
            ),
            ownerId = "u-lakshmi", createdByName = "City Diagnostics",
            accessList = listOf(Grant("g1", GranteeType.HOSPITAL, "h-a", "Test Hospital A"),
                Grant("g2", GranteeType.USER, "u-rao", "Dr. Anitha Rao"), Grant("g3", GranteeType.USER, "l-city", "City Diagnostics")),
            pointerItemId = "i-followup", allowedActions = listOf("read", "share", "revoke", "status"),
        ),
        DataItem("i-msg", "2026-09-22", CoreItemType.MESSAGE, "Dr. Anitha Rao", "Please share your latest BP readings",
            ownerId = "u-lakshmi", createdByName = "Dr. Anitha Rao", accessList = listOf(Grant("g4", GranteeType.USER, "u-rao", "Dr. Anitha Rao"))),
        DataItem("i-followup", "2026-09-24", CoreItemType.BOOKING, "Follow-up consultation", "Test Hospital A · 24 Sep, 11:30",
            ownerId = "u-lakshmi", createdByName = "Dr. Anitha Rao",
            accessList = listOf(Grant("g5", GranteeType.HOSPITAL, "h-a", "Test Hospital A")), pointerItemId = "i-cbc"),
        DataItem("i-med", "2026-09-22", CoreItemType.ALERT, "Medication reminder", "Evening dose · 8:00 PM",
            ownerId = "u-lakshmi", createdByName = "Dr. Anitha Rao", accessList = listOf(Grant("g6", GranteeType.USER, "u-rao", "Dr. Anitha Rao"))),
        DataItem("i-fee", "2026-09-18", CoreItemType.PAYMENT, "Consultation fee", "₹600 · Payment pending",
            ownerId = "u-lakshmi", createdByName = "Test Hospital A", accessList = listOf(Grant("g7", GranteeType.HOSPITAL, "h-a", "Test Hospital A"))),
        DataItem("i-feedback", "2026-09-10", CoreItemType.FEEDBACK, "Visit feedback", "Rate your 10 Sep consultation",
            ownerId = "u-lakshmi", createdByName = "Dr. Anitha Rao", accessList = listOf(Grant("g8", GranteeType.USER, "u-rao", "Dr. Anitha Rao"))),
        DataItem("i-priya-lipid", "2026-09-19", CoreItemType.REPORT, "Lipid profile", "City Diagnostics · Lab report",
            ownerId = "u-priya", createdByName = "City Diagnostics", accessList = listOf(Grant("g9", GranteeType.USER, "l-city", "City Diagnostics"))),
    )

    private fun key(a: String, b: String) = listOf(a, b).sorted().joinToString("|")
    private val chats = mutableMapOf(
        key("u-lakshmi", "u-rao") to mutableListOf(
            Message("m1", "t-u-lakshmi|u-rao", "u-rao", "Please share your latest BP readings before Thursday.", "10:42"),
            Message("m2", "t-u-lakshmi|u-rao", "u-lakshmi", "Sure, I will upload them tonight.", "10:50"),
        ),
    )

    private val prefs = mutableMapOf<String, NotificationPrefs>()

    // ---- realtime simulation ----
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val _events = MutableSharedFlow<RealtimeEvent>(extraBufferCapacity = 32)
    override val events: SharedFlow<RealtimeEvent> = _events
    override fun startRealtime() { _events.tryEmit(RealtimeEvent.ConnectionChanged(true)) }
    override fun stopRealtime() {}

    // ---- reads ----
    override suspend fun me() = view(raw(meId))

    override suspend fun dashboard(): Dashboard = latency {
        val mine = items.filter { canSee(it, meId) && it.status == ItemStatus.OPEN }
        Dashboard(mine.count { it.type == CoreItemType.REPORT }, if (meId == "u-lakshmi") 2 else 1, mine.count { it.type == CoreItemType.BOOKING })
    }

    override suspend fun openItems(limit: Int) = latency {
        items.filter { canSee(it, meId) && it.status == ItemStatus.OPEN }.sortedByDescending { it.date }.take(limit)
    }

    override suspend fun item(id: String) = latency {
        items.first { it.id == id }.also { check(canSee(it, meId)) { "No access" } }
            .let { if (it.ownerId == meId) it else it.copy(allowedActions = listOf("read")) }
    }

    override suspend fun user(id: String) = latency { view(raw(id)) }

    override suspend fun sharedItems(userId: String): List<DataItem> = latency {
        if (!connected(meId, userId)) emptyList()
        else items.filter { it.status == ItemStatus.OPEN &&
            ((it.ownerId == meId && canSee(it, userId)) || (it.ownerId == userId && canSee(it, meId))) }
    }

    override suspend fun messages(userId: String) = latency { chats[key(meId, userId)].orEmpty().toList() }

    override suspend fun sendMessage(userId: String, body: String): Message = latency {
        val k = key(meId, userId)
        val m = Message(UUID.randomUUID().toString(), "t-$k", meId, body, now())
        chats.getOrPut(k) { mutableListOf() } += m
        // Simulate the other person answering over the WebSocket, so live delivery can be tested.
        val other = raw(userId)
        if (other.primaryRole != Role.HOSPITAL) scope.launch {
            delay(1500)
            val reply = Message(UUID.randomUUID().toString(), "t-$k", userId,
                if (other.primaryRole == Role.PATIENT) "Thank you, doctor." else "Noted — I'll review it today.", now())
            chats.getValue(k) += reply
            _events.emit(RealtimeEvent.NewMessage(reply))
        }
        m
    }

    override suspend fun threads() = latency {
        chats.filterKeys { meId in it.split("|") }.mapNotNull { (k, list) ->
            val otherId = k.split("|").first { it != meId }
            list.lastOrNull()?.let { ThreadSummary("t-$k", view(raw(otherId)), it.body, it.sentAt, if (it.senderId != meId) 1 else 0) }
        }
    }

    override suspend fun search(query: String, role: Role?) = latency {
        val q = query.trim().lowercase()
        users.filter { it.id != meId }
            .filter { role == null || it.primaryRole == role }
            .filter { q.isEmpty() || it.displayName.lowercase().contains(q) || it.publicId.lowercase().contains(q) }
            .map(::view)
    }

    override suspend fun connections() = latency { users.filter { it.id != meId && connected(meId, it.id) }.map(::view) }

    // ---- writes ----
    override suspend fun connect(userId: String): UserProfile = latency { connections += setOf(meId, userId); view(raw(userId)) }

    override suspend fun setStatus(itemId: String, status: ItemStatus) = update(itemId) { it.copy(status = status) }

    override suspend fun revokeGrant(itemId: String, grantId: String) =
        update(itemId) { item -> item.copy(accessList = item.accessList.filterNot { it.grantId == grantId }) }

    override suspend fun share(itemId: String, granteeId: String) = update(itemId) { item ->
        if (item.accessList.any { it.granteeId == granteeId }) item else item.copy(accessList = item.accessList + grantFor(raw(granteeId)))
    }

    override suspend fun upload(draft: UploadDraft, files: List<PendingAttachment>): DataItem = latency(600) {
        val me = raw(meId)
        val grants = draft.shareWith.map { grantFor(raw(it)) }.toMutableList()
        // Uploading for a patient: the patient owns the item; the uploader keeps access (doc 2.3).
        if (draft.ownerId != meId && grants.none { it.granteeId == meId }) grants += grantFor(me)
        DataItem(
            id = UUID.randomUUID().toString(),
            date = draft.date.ifBlank { LocalDate.now().toString() },
            type = draft.type,
            title = draft.title.ifBlank { draft.type.label },
            subtitle = if (draft.ownerId != meId) "${me.displayName} · ${files.size} file${if (files.size == 1) "" else "s"}"
                       else "${files.size} attachment${if (files.size == 1) "" else "s"}",
            keywords = draft.keywords,
            attachments = files.mapIndexed { i, f -> Attachment(f.kind, f.localUri.toString(), f.mime, f.size, i, f.name) },
            links = draft.links, ownerId = draft.ownerId, createdByName = me.displayName,
            accessList = grants, pointerToMessage = draft.pointerToMessage, status = draft.status,
            allowedActions = listOf("read", "share", "revoke", "status"),
        ).also { items.add(0, it); _events.tryEmit(RealtimeEvent.ItemChanged(it.id)) }
    }

    override suspend fun updateProfile(update: ProfileUpdate): UserProfile = latency {
        val i = users.indexOfFirst { it.id == meId }
        users[i] = users[i].copy(displayName = update.displayName.trim(), location = update.location?.trim()?.ifEmpty { null })
        view(users[i])
    }

    override suspend fun activeShares(): List<ShareGroup> = latency {
        items.filter { it.ownerId == meId }.flatMap { item ->
            item.accessList.map { g -> OwnedGrant(g.grantId, g.granteeType, g.granteeId, g.granteeName, item.id, item.title, item.type) }
        }.groupBy { it.granteeId }.map { (id, list) -> ShareGroup(id, list.first().granteeName, list.first().granteeType, list) }
            .sortedBy { it.granteeName }
    }

    override suspend fun notificationPrefs() = latency { prefs[meId] ?: NotificationPrefs() }
    override suspend fun saveNotificationPrefs(prefs: NotificationPrefs) = latency { this.prefs[meId] = prefs; prefs }
    override suspend fun registerDevice(token: String) {}
    override suspend fun unregisterDevice(token: String) {}

    private suspend fun update(id: String, f: (DataItem) -> DataItem): DataItem = latency {
        val i = items.indexOfFirst { it.id == id }
        items[i] = f(items[i]); items[i]
    }

    private fun now() = LocalTime.now().format(DateTimeFormatter.ofPattern("HH:mm"))
    private suspend fun <T> latency(ms: Long = 150, block: () -> T): T { delay(ms); return block() }
}
