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
import java.time.OffsetDateTime
import java.time.ZoneId
import java.util.UUID

/**
 * Seeded demo backend for DataItem v2. Everything is evaluated from the signed-in user's point of
 * view, so you can sign in as the patient, the doctor or the lab and test each flow. Access
 * follows design doc 2.3 applied to the whole item (DataItem_Design.md 3.8).
 */
class FakeRepository : HealooRepository {

    private fun asset(name: String) = "file:///android_asset/sample/$name"
    private val zone = ZoneId.of("Asia/Kolkata")
    private fun today() = LocalDate.now(zone)
    private fun nowIso() = OffsetDateTime.now(zone).toString()
    private fun id() = UUID.randomUUID().toString()

    private val users = mutableListOf(
        UserProfile("u-lakshmi", "HL-2M9P4", "Lakshmi K.", listOf(Role.PATIENT), "Patient", location = "[City]"),
        UserProfile("u-rao", "HL-7R2C9", "Dr. Anitha Rao", listOf(Role.DOCTOR), "Doctor · General Medicine",
            hospital = "Test Hospital A", officialNumber = "[REG-NO]", location = "[City]"),
        UserProfile("h-a", "HL-1H0A1", "Test Hospital A", listOf(Role.HOSPITAL), "Hospital"),
        UserProfile("l-city", "HL-6L3D2", "City Diagnostics", listOf(Role.LAB), "Lab"),
        UserProfile("u-srao", "HL-3K8M1", "Dr. Srinivas Rao", listOf(Role.DOCTOR), "Doctor · Independent"),
        UserProfile("h-raoheart", "HL-9P4T6", "Rao Heart Clinic", listOf(Role.HOSPITAL), "Hospital"),
        UserProfile("l-rao", "HL-5W1Q3", "Rao Diagnostics", listOf(Role.LAB), "Lab"),
        UserProfile("u-priya", "HL-4K7Q2", "Priya Rao", listOf(Role.PATIENT), "Patient"),
        UserProfile("u-menon", "HL-2D5M8", "Dr. Kavya Menon", listOf(Role.DOCTOR), "Doctor · Cardiology", hospital = "Test Hospital A"),
        UserProfile("a-admin", "HL-8A2D4", "Hospital A Admin", listOf(Role.ADMINISTRATOR), "Hospital administrator", hospital = "Test Hospital A"),
    )

    /** Accounts offered on the demo sign-in screen. */
    val demoAccounts: List<UserProfile> get() = users.filter { it.id in listOf("u-lakshmi", "u-rao", "l-city", "h-a", "a-admin") }

    private var meId = "u-lakshmi"
    fun signInAs(userId: String) { meId = userId }

    private val affiliation = mutableMapOf("u-rao" to "h-a", "u-menon" to "h-a")   // doctor -> current hospital
    private val adminHospital = mapOf("a-admin" to "h-a")                              // administrator -> hospital
    private val deactivated = mutableSetOf<String>()
    private val emails = mutableMapOf<String, String>()                                // user -> email (admin-created)
    private val homeHospital = mutableMapOf("u-rao" to "h-a", "u-menon" to "h-a")
    private val connections = mutableSetOf(
        setOf("u-lakshmi", "u-rao"), setOf("u-lakshmi", "h-a"), setOf("u-lakshmi", "l-city"), setOf("u-lakshmi", "u-srao"),
        setOf("u-rao", "h-a"), setOf("u-rao", "l-city"), setOf("u-priya", "l-city"),
    )

    private fun raw(id: String) = users.first { it.id == id }
    private fun connected(a: String, b: String) = setOf(a, b) in connections
    private fun view(u: UserProfile) = u.copy(connected = connected(meId, u.id))
    private fun grantFor(u: UserProfile) = Grant(id(), if (u.primaryRole == Role.HOSPITAL) GranteeType.HOSPITAL else GranteeType.USER, u.id, u.displayName)

    // ---- stored items (children kept inside; derived fields computed per viewer) ----
    private data class Stored(
        var item: DataItem,
        val createdBy: String,
        val marks: MutableMap<String, MutableMap<String, String>> = mutableMapOf(),   // appointment -> date -> COMPLETED/NO_SHOW
    )
    private val store = mutableListOf<Stored>()
    private val unread = mutableMapOf<String, Int>()                                  // "user|item" -> count

    private fun msg(itemId: String, from: String, body: String, minutesAgo: Long) =
        Message(id(), itemId, from, body, OffsetDateTime.now(zone).minusMinutes(minutesAgo).toString())

    private fun appt(itemId: String, doctor: String, date: LocalDate, time: String, rec: Recurrence?, notes: String) =
        Appointment(id(), itemId, "u-lakshmi", doctor, raw(doctor).displayName, "h-a", date.toString(), time, notes = notes, recurrence = rec)

    private fun alert(itemId: String, forUser: String, type: String, text: String, date: LocalDate, time: String, rec: Recurrence?) =
        Alert(id(), itemId, type, text, forUser, "${date}T$time:00+05:30", recurrence = rec)

    init {
        val rao = Grant("g-rao", GranteeType.USER, "u-rao", "Dr. Anitha Rao")
        val hospA = Grant("g-ha", GranteeType.HOSPITAL, "h-a", "Test Hospital A")
        val lab = Grant("g-lab", GranteeType.USER, "l-city", "City Diagnostics")

        // 1. Lab report that grew into a full case.
        store += Stored(DataItem(
            id = "i-cbc", ownerId = "u-lakshmi", primaryKind = PrimaryKind.REPORT, title = "CBC – Complete blood count",
            keywords = listOf("CBC", "Haemoglobin", "Routine check"), accessList = listOf(hospA, rao, lab),
            attachments = listOf(
                Attachment("a1", AttachmentKind.IMAGE, asset("cbc_scan_1.jpg"), "image/jpeg", 91_000, 0, "cbc_scan_1.jpg", isReport = true),
                Attachment("a2", AttachmentKind.IMAGE, asset("cbc_scan_2.jpg"), "image/jpeg", 92_000, 1, "cbc_scan_2.jpg", isReport = true),
                Attachment("a3", AttachmentKind.PDF, asset("cbc_report.pdf"), "application/pdf", 3_400, 2, "CBC_report.pdf", pageCount = 3, isReport = true),
                Attachment("a4", AttachmentKind.PDF, asset("reference_ranges.pdf"), "application/pdf", 2_500, 3, "Reference_ranges.pdf", pageCount = 2, isReport = true),
            ),
            messages = listOf(
                msg("i-cbc", "u-rao", "Haemoglobin is slightly low. Let's review every two weeks for a while.", 180),
                msg("i-cbc", "u-lakshmi", "Thank you, doctor. I'll book the visits.", 170),
            ),
            appointments = listOf(appt("i-cbc", "u-rao", today().plusDays(7), "11:30", Recurrence(Frequency.BIWEEKLY, Period.THREE_MONTHS), "Haemoglobin review")),
            alerts = listOf(alert("i-cbc", "u-lakshmi", "MEDICATION", "Iron tablet after dinner", today(), "20:30", Recurrence(Frequency.DAILY, Period.ONE_MONTH))),
            createdByName = "City Diagnostics", createdAt = nowIso(), updatedAt = nowIso(),
        ), createdBy = "l-city")

        // 2. Standalone discussion.
        store += Stored(DataItem(
            id = "i-bp", ownerId = "u-lakshmi", primaryKind = PrimaryKind.MESSAGE, title = "Latest BP readings", keywords = listOf("BP"),
            accessList = listOf(rao), createdByName = "Dr. Anitha Rao", createdAt = nowIso(), updatedAt = nowIso(),
            messages = listOf(
                msg("i-bp", "u-rao", "Please share your latest BP readings before Thursday.", 60),
                msg("i-bp", "u-lakshmi", "Sure, I will upload them tonight.", 50),
            ),
        ), createdBy = "u-rao")
        unread["u-lakshmi|i-cbc"] = 1

        // 3. Standalone appointment.
        store += Stored(DataItem(
            id = "i-followup", ownerId = "u-lakshmi", primaryKind = PrimaryKind.APPOINTMENT, title = "Appointment with Dr. Anitha Rao",
            keywords = listOf("Follow-up"), accessList = listOf(rao, hospA), createdByName = "Lakshmi K.", createdAt = nowIso(), updatedAt = nowIso(),
            appointments = listOf(appt("i-followup", "u-rao", today().plusDays(3), "10:00", null, "General follow-up")),
        ), createdBy = "u-lakshmi")

        // 4. Standalone alert.
        store += Stored(DataItem(
            id = "i-bpcheck", ownerId = "u-lakshmi", primaryKind = PrimaryKind.ALERT, title = "Check blood pressure", keywords = listOf("BP"),
            accessList = listOf(rao), createdByName = "Lakshmi K.", createdAt = nowIso(), updatedAt = nowIso(),
            alerts = listOf(alert("i-bpcheck", "u-lakshmi", "CUSTOM", "Check blood pressure", today(), "08:00", Recurrence(Frequency.WEEKLY, Period.THREE_MONTHS))),
        ), createdBy = "u-lakshmi")

        // 5. Closed item with feedback and rating.
        store += Stored(DataItem(
            id = "i-closed", ownerId = "u-lakshmi", primaryKind = PrimaryKind.MESSAGE, title = "Consultation on 10 Sep",
            accessList = listOf(rao), status = ItemStatus.CLOSED, createdByName = "Dr. Anitha Rao", createdAt = nowIso(), updatedAt = nowIso(),
            closure = Closure(nowIso(), "u-lakshmi", "Explained everything clearly.", 5),
            messages = listOf(msg("i-closed", "u-rao", "Continue the same dose for two more weeks.", 20_000)),
        ), createdBy = "u-rao")

        // 6. Priya's report, not shared yet.
        store += Stored(DataItem(
            id = "i-priya-lipid", ownerId = "u-priya", primaryKind = PrimaryKind.REPORT, title = "Lipid profile", keywords = listOf("Lipid"),
            accessList = listOf(lab), createdByName = "City Diagnostics", createdAt = nowIso(), updatedAt = nowIso(),
            attachments = listOf(Attachment("a5", AttachmentKind.PDF, asset("cbc_report.pdf"), "application/pdf", 3_400, 0, "Lipid_profile.pdf", pageCount = 2, isReport = true)),
        ), createdBy = "l-city")
    }

    private val prefs = mutableMapOf<String, NotificationPrefs>()

    // ---- realtime simulation ----
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val _events = MutableSharedFlow<RealtimeEvent>(extraBufferCapacity = 32)
    override val events: SharedFlow<RealtimeEvent> = _events
    override fun startRealtime() { _events.tryEmit(RealtimeEvent.ConnectionChanged(true)) }
    override fun stopRealtime() {}

    // ---- access (design doc 2.3 on the whole item) ----
    private enum class Access { FULL, META, DENY }

    private fun access(s: Stored, userId: String): Access {
        val it = s.item
        if (it.ownerId == userId) return Access.FULL
        val role = raw(userId).primaryRole
        if ((role == Role.LAB || role == Role.HOSPITAL) && s.createdBy == userId) return Access.FULL
        val direct = it.accessList.any { g -> g.granteeType == GranteeType.USER && g.granteeId == userId }
        val viaHospital = role == Role.DOCTOR && it.accessList.any { g -> g.granteeType == GranteeType.HOSPITAL && affiliation[userId] == g.granteeId }
        if (!direct && !viaHospital) return Access.DENY
        val hasReportFiles = it.attachments.any { a -> a.isReport }
        return if (hasReportFiles && role != Role.DOCTOR) Access.META else Access.FULL
    }

    private fun participants(it: DataItem) = (listOf(it.ownerId) + it.accessList.filter { g -> g.granteeType == GranteeType.USER }.map { g -> g.granteeId }).distinct()

    /** What the signed-in user sees: children, derived kinds/counts/visits, allowed actions. */
    private fun present(s: Stored): DataItem {
        val a = access(s, meId)
        check(a != Access.DENY) { "No access" }
        val base = s.item
        val full = a == Access.FULL
        val owner = base.ownerId == meId
        val appts = base.appointments.map { ap ->
            ap.copy(
                visits = RecurrenceRules.visits(LocalDate.parse(ap.date), LocalTime.parse(ap.time), ap.recurrence, ap.exceptions,
                    s.marks[ap.id].orEmpty(), ap.status == "CANCELLED", today()),
                visitCount = ap.recurrence?.let { RecurrenceRules.visitCount(LocalDate.parse(ap.date), it) } ?: 1,
                notes = if (full) ap.notes else null,
            )
        }
        val alerts = if (full) base.alerts.filter { owner || it.forUser == meId } else emptyList()
        val kinds = buildList {
            add(base.primaryKind.name)
            if (base.attachments.any { it.isReport }) add(PartKind.REPORT)
            if (base.appointments.isNotEmpty()) add(PartKind.APPOINTMENT)
            if (base.messages.isNotEmpty()) add(PartKind.MESSAGE)
            if (base.alerts.isNotEmpty()) add(PartKind.ALERT)
            if (base.attachments.isNotEmpty()) add(PartKind.ATTACHMENT)
        }.distinct()
        val actions = buildList {
            when {
                !full -> add("meta")
                base.status == ItemStatus.OPEN -> addAll(listOf("read", "message", "attach", "book", "alert", "close"))
                else -> add("read")
            }
            if (full && owner) { add("share"); add("revoke") } else if (full && raw(meId).canRefer) add("share")
            if (full && owner && base.status == ItemStatus.OPEN) add("rate")
            if (full && base.status == ItemStatus.CLOSED && (owner || raw(meId).primaryRole == Role.DOCTOR)) add("reopen")
        }
        val messages = if (full) base.messages else emptyList()
        val attachments = if (full) base.attachments.sortedBy { it.position } else emptyList()
        return base.copy(
            kinds = kinds,
            accessList = if (owner) base.accessList else emptyList(),
            messages = messages, attachments = attachments, appointments = appts, alerts = alerts,
            links = if (full) base.links else emptyList(),
            counts = ItemCounts(messages.size, attachments.size, appts.size, alerts.size, unread["$meId|${base.id}"] ?: 0),
            allowedActions = actions,
        )
    }

    private fun stored(id: String) = store.first { it.item.id == id }
    private fun visible(status: ItemStatus) = store.filter { it.item.status == status && access(it, meId) != Access.DENY }
    private suspend fun change(id: String, f: (Stored) -> Unit): DataItem = latency {
        val s = stored(id)
        check(access(s, meId) == Access.FULL) { "No access" }
        f(s)
        s.item = s.item.copy(updatedAt = nowIso())
        _events.tryEmit(RealtimeEvent.ItemChanged(id))
        present(s)
    }

    // ---- reads ----
    override suspend fun me() = view(raw(meId))

    override suspend fun dashboard(): Dashboard = latency {
        val open = visible(ItemStatus.OPEN).map { present(it) }
        val soon = today().plusDays(7).toString()
        Dashboard(
            openReports = open.count { PartKind.REPORT in it.kinds },
            unreadMessages = open.sumOf { it.counts.unreadMessages },
            upcomingAppointments = open.flatMap { it.appointments }.flatMap { it.visits }.count { it.status == "SCHEDULED" && it.date <= soon },
        )
    }

    override suspend fun items(status: ItemStatus, limit: Int) = latency {
        visible(status).map { present(it) }.sortedByDescending { it.updatedAt }.take(limit)
    }

    override suspend fun item(id: String) = latency { present(stored(id)) }
    override suspend fun user(id: String) = latency { view(raw(id)) }

    override suspend fun sharedItems(userId: String): List<DataItem> = latency {
        if (!connected(meId, userId)) emptyList()
        else store.filter { s -> s.item.status == ItemStatus.OPEN &&
            ((s.item.ownerId == meId && access(s, userId) != Access.DENY) || (s.item.ownerId == userId && access(s, meId) != Access.DENY)) }
            .map { present(it) }
    }

    override suspend fun search(query: String, role: Role?) = latency {
        val q = query.trim().lowercase()
        users.filter { it.id != meId && it.id !in deactivated }
            .filter { role == null || it.primaryRole == role }
            .filter { q.isEmpty() || it.displayName.lowercase().contains(q) || it.publicId.lowercase().contains(q) }
            .map(::view)
    }

    override suspend fun connections() = latency { users.filter { it.id != meId && connected(meId, it.id) }.map(::view) }

    override suspend fun hospitalDoctors(hospitalId: String) = latency {
        users.filter { it.primaryRole == Role.DOCTOR && affiliation[it.id] == hospitalId && it.id !in deactivated }.sortedBy { it.displayName }.map(::view)
    }

    override suspend fun conversations(withUser: String?) = latency {
        store.filter { access(it, meId) == Access.FULL && it.item.messages.isNotEmpty() && meId in participants(it.item) }
            .flatMap { s ->
                val last = s.item.messages.last()
                participants(s.item).filter { it != meId && (withUser == null || it == withUser) }.map { other ->
                    Conversation(s.item.id, s.item.title, s.item.primaryKind, view(raw(other)), last.body, last.sentAt, unread["$meId|${s.item.id}"] ?: 0)
                }
            }.sortedByDescending { it.lastMessageAt }
    }

    override suspend fun calendar(from: String, to: String) = latency {
        val isDoctor = raw(meId).primaryRole == Role.DOCTOR
        store.filter { access(it, meId) != Access.DENY }.flatMap { s ->
            present(s).appointments.filter { if (isDoctor) it.doctorId == meId else it.patientId == meId }.flatMap { a ->
                a.visits.filter { it.date in from..to }.map { v ->
                    val other = if (isDoctor) a.patientId else a.doctorId
                    CalendarVisit("${v.date}T${v.time}:00+05:30", a.id, s.item.id, s.item.title, other, raw(other).displayName, v.status)
                }
            }
        }.sortedBy { it.startsAt }
    }

    override suspend fun itemMessages(itemId: String) = latency {
        val s = stored(itemId)
        check(access(s, meId) == Access.FULL) { "No access" }
        unread.remove("$meId|$itemId")
        s.item.messages
    }

    // ---- create ----
    private fun newItem(owner: String, kind: PrimaryKind, title: String, keywords: List<String>, shareWith: List<String>): Stored {
        val grants = shareWith.distinct().filter { it != owner }.map { grantFor(raw(it)) }.toMutableList()
        if (owner != meId && grants.none { it.granteeId == meId }) grants += grantFor(raw(meId))   // uploader keeps access
        return Stored(DataItem(
            id = id(), ownerId = owner, primaryKind = kind, title = title.ifBlank { kind.label }, keywords = keywords,
            accessList = grants, createdByName = raw(meId).displayName, createdAt = nowIso(), updatedAt = nowIso(),
        ), createdBy = meId).also { store.add(0, it) }
    }

    /** Creating for a patient, the uploader may also refer the item to doctors only. */
    private fun doctorsOnly(ids: List<String>) = ids.filter { raw(it).primaryRole == Role.DOCTOR }

    private fun toAttachments(files: List<PendingAttachment>, startAt: Int, isReport: Boolean) =
        files.mapIndexed { i, f -> Attachment(id(), f.kind, f.localUri.toString(), f.mime, f.size, startAt + i, f.name, addedBy = meId, isReport = isReport) }

    override suspend fun createReport(draft: ReportDraft, files: List<PendingAttachment>): DataItem = latency(600) {
        require(files.isNotEmpty() || draft.links.isNotEmpty()) { "A report needs at least one file or link" }
        val s = newItem(draft.ownerId, PrimaryKind.REPORT, draft.title, draft.keywords, if (draft.ownerId == meId) draft.shareWith else doctorsOnly(draft.shareWith))
        s.item = s.item.copy(attachments = toAttachments(files, 0, true), links = draft.links)
        _events.tryEmit(RealtimeEvent.ItemChanged(s.item.id))
        present(s)
    }

    override suspend fun createAppointment(ownerId: String?, appointment: NewAppointment, shareWith: List<String>): DataItem = latency {
        RecurrenceRules.problem(LocalDate.parse(appointment.date), appointment.recurrence)?.let { error(it) }
        val owner = ownerId ?: appointment.patientId
        val doctor = raw(appointment.doctorId)
        val s = newItem(owner, PrimaryKind.APPOINTMENT, "Appointment with ${doctor.displayName}", emptyList(), shareWith + appointment.doctorId)
        s.item = s.item.copy(appointments = listOf(Appointment(id(), s.item.id, appointment.patientId, doctor.id, doctor.displayName, appointment.hospitalId,
            appointment.date, appointment.time, appointment.timezone, appointment.durationMin, appointment.notes, appointment.recurrence)))
        present(s)
    }

    override suspend fun createAlert(title: String, alert: NewAlert, shareWith: List<String>): DataItem = latency {
        val s = newItem(meId, PrimaryKind.ALERT, title.ifBlank { alert.text }, emptyList(), shareWith)
        s.item = s.item.copy(alerts = listOf(Alert(id(), s.item.id, alert.type, alert.text, alert.forUser ?: meId,
            "${alert.date}T${alert.time}:00+05:30", alert.timezone, alert.recurrence)))
        present(s)
    }

    override suspend fun startConversation(userId: String, body: String, alsoWith: List<String>): DataItem {
        val other = raw(userId)
        val me = raw(meId)
        val owner = if (other.primaryRole == Role.PATIENT && me.isClinical) other.id else meId
        val s = latency { newItem(owner, PrimaryKind.MESSAGE, body.lineSequence().first().take(60), emptyList(), if (owner == meId) listOf(userId) + alsoWith else doctorsOnly(alsoWith)) }
        sendItemMessage(s.item.id, body)
        return item(s.item.id)
    }

    // ---- add to an item ----
    override suspend fun sendItemMessage(itemId: String, body: String): Message {
        lateinit var m: Message
        change(itemId) { s ->
            check(s.item.status == ItemStatus.OPEN) { "This item is closed" }
            m = Message(id(), itemId, meId, body.trim(), nowIso())
            s.item = s.item.copy(messages = s.item.messages + m)
            participants(s.item).filter { it != meId }.forEach { unread["$it|$itemId"] = (unread["$it|$itemId"] ?: 0) + 1 }
        }
        // Simulate the other person answering over the WebSocket, so live delivery can be tested.
        val s = stored(itemId)
        val other = participants(s.item).firstOrNull { it != meId }?.let(::raw)
        if (other != null && other.primaryRole != Role.HOSPITAL) scope.launch {
            delay(1500)
            val reply = Message(id(), itemId, other.id, if (other.primaryRole == Role.PATIENT) "Thank you, doctor." else "Noted — I'll review it today.", nowIso())
            s.item = s.item.copy(messages = s.item.messages + reply)
            _events.emit(RealtimeEvent.NewMessage(reply))
        }
        return m
    }

    override suspend fun addAttachments(itemId: String, files: List<PendingAttachment>, isReport: Boolean) = change(itemId) { s ->
        s.item = s.item.copy(attachments = s.item.attachments + toAttachments(files, s.item.attachments.size, isReport))
    }

    override suspend fun bookAppointment(itemId: String, appointment: NewAppointment) = change(itemId) { s ->
        RecurrenceRules.problem(LocalDate.parse(appointment.date), appointment.recurrence)?.let { error(it) }
        val doctor = raw(appointment.doctorId)
        s.item = s.item.copy(
            appointments = s.item.appointments + Appointment(id(), itemId, s.item.ownerId, doctor.id, doctor.displayName, appointment.hospitalId,
                appointment.date, appointment.time, appointment.timezone, appointment.durationMin, appointment.notes, appointment.recurrence),
            accessList = if (s.item.accessList.any { it.granteeId == doctor.id } || doctor.id == s.item.ownerId) s.item.accessList else s.item.accessList + grantFor(doctor),
        )
    }

    override suspend fun visitAction(itemId: String, appointmentId: String, visitDate: String, action: String, newDate: String?, newTime: String?) =
        change(itemId) { s ->
            s.item = s.item.copy(appointments = s.item.appointments.map { a ->
                if (a.id != appointmentId) a else when (action) {
                    "CANCELLED", "MOVED" -> a.copy(exceptions = a.exceptions.filter { it.date != visitDate } + VisitException(visitDate, action, newDate, newTime))
                    else -> { s.marks.getOrPut(a.id) { mutableMapOf() }[visitDate] = action; a }
                }
            })
        }

    override suspend fun cancelAppointment(itemId: String, appointmentId: String) = change(itemId) { s ->
        s.item = s.item.copy(appointments = s.item.appointments.map { if (it.id == appointmentId) it.copy(status = "CANCELLED") else it })
    }

    override suspend fun addAlert(itemId: String, alert: NewAlert) = change(itemId) { s ->
        s.item = s.item.copy(alerts = s.item.alerts + Alert(id(), itemId, alert.type, alert.text, alert.forUser ?: meId,
            "${alert.date}T${alert.time}:00+05:30", alert.timezone, alert.recurrence))
    }

    override suspend fun deleteAlert(itemId: String, alertId: String) = change(itemId) { s ->
        s.item = s.item.copy(alerts = s.item.alerts.filterNot { it.id == alertId })
    }

    override suspend fun closeItem(itemId: String, feedback: String?, rating: Int?) = change(itemId) { s ->
        require(rating == null || s.item.ownerId == meId) { "Only the patient can rate" }
        s.item = s.item.copy(
            status = ItemStatus.CLOSED,
            closure = Closure(nowIso(), meId, feedback?.trim()?.ifEmpty { null }, rating),
            appointments = s.item.appointments.map { it.copy(status = "CANCELLED") },
            alerts = s.item.alerts.map { it.copy(active = false) },
        )
    }

    override suspend fun reopenItem(itemId: String) = change(itemId) { s ->
        s.item = s.item.copy(status = ItemStatus.OPEN, closure = null)
    }

    // ---- sharing ----
    override suspend fun connect(userId: String): UserProfile = latency { connections += setOf(meId, userId); view(raw(userId)) }

    override suspend fun revokeGrant(itemId: String, grantId: String) = change(itemId) { s ->
        s.item = s.item.copy(accessList = s.item.accessList.filterNot { it.grantId == grantId })
    }

    override suspend fun share(itemId: String, granteeId: String) = change(itemId) { s ->
        // The owner shares with anyone; a doctor, lab or hospital may pass it on to a doctor.
        check(s.item.ownerId == meId || (raw(meId).canRefer && raw(granteeId).primaryRole == Role.DOCTOR)) {
            "Only the owner can share this with ${raw(granteeId).displayName}; you can pass it on to doctors"
        }
        if (s.item.accessList.none { it.granteeId == granteeId }) s.item = s.item.copy(accessList = s.item.accessList + grantFor(raw(granteeId)))
    }

    // ---- administration (rule 10: create users and doctors, deactivate doctors only) ----
    private fun myHospital(): String = adminHospital[meId] ?: error("Only hospital administrators can manage accounts")

    private fun adminView(u: UserProfile) =
        AdminAccount(u.id, u.publicId, u.displayName, u.roles, u.headline, u.hospital, u.officialNumber, emails[u.id], u.id !in deactivated)

    override suspend fun adminDoctors(): List<AdminAccount> = latency {
        val h = myHospital()
        users.filter { it.primaryRole == Role.DOCTOR && (affiliation[it.id] == h || (it.id in deactivated && homeHospital[it.id] == h)) }
            .sortedWith(compareBy({ it.id in deactivated }, { it.displayName })).map(::adminView)
    }

    private fun newAccount(a: NewAccount, role: Role): AdminAccount {
        val h = myHospital()
        val email = a.email.trim().lowercase()
        require(a.displayName.isNotBlank()) { "Enter a name" }
        require(Regex("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$").matches(email)) { "Enter a valid email address" }
        require(email !in emails.values) { "An account with that email already exists" }
        val doctor = role == Role.DOCTOR
        val alphabet = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ"
        val publicId = generateSequence { "HL-" + (1..5).map { alphabet.random() }.joinToString("") }.first { p -> users.none { it.publicId == p } }
        val u = UserProfile(
            "n-" + id().take(8), publicId, a.displayName.trim(), listOf(role),
            headline = if (doctor) a.designation?.trim()?.takeIf { it.isNotEmpty() }?.let { "Doctor · $it" } ?: "Doctor" else "Patient",
            hospital = if (doctor) raw(h).displayName else null,
            officialNumber = if (doctor) a.officialNumber?.trim()?.ifEmpty { null } else null,
        )
        users += u
        emails[u.id] = email
        if (doctor) { affiliation[u.id] = h; homeHospital[u.id] = h; connections += setOf(u.id, h) }
        return adminView(u)
    }

    override suspend fun adminCreateUser(account: NewAccount) = latency { newAccount(account, Role.PATIENT) }
    override suspend fun adminCreateDoctor(account: NewAccount) = latency { newAccount(account, Role.DOCTOR) }

    override suspend fun adminDeactivateDoctor(doctorId: String) = latency {
        val h = myHospital()
        val d = raw(doctorId)
        check(d.primaryRole == Role.DOCTOR) { "Users can't be deleted; administrators can delete (deactivate) doctors only" }
        check(affiliation[doctorId] == h || homeHospital[doctorId] == h) { "You can delete only doctors of your own hospital" }
        affiliation.remove(doctorId)
        deactivated += doctorId
    }

    override suspend fun adminReactivateDoctor(doctorId: String) = latency {
        val h = myHospital()
        check(homeHospital[doctorId] == h) { "You can reactivate only doctors of your own hospital" }
        deactivated -= doctorId
        affiliation[doctorId] = h
        adminView(raw(doctorId))
    }

    // ---- account ----
    override suspend fun updateProfile(update: ProfileUpdate): UserProfile = latency {
        val i = users.indexOfFirst { it.id == meId }
        users[i] = users[i].copy(displayName = update.displayName.trim(), location = update.location?.trim()?.ifEmpty { null })
        view(users[i])
    }

    override suspend fun activeShares(): List<ShareGroup> = latency {
        store.filter { it.item.ownerId == meId }.flatMap { s ->
            s.item.accessList.map { g -> OwnedGrant(g.grantId, g.granteeType, g.granteeId, g.granteeName, s.item.id, s.item.title, s.item.primaryKind) }
        }.groupBy { it.granteeId }.map { (id, list) -> ShareGroup(id, list.first().granteeName, list.first().granteeType, list) }
            .sortedBy { it.granteeName }
    }

    override suspend fun notificationPrefs() = latency { prefs[meId] ?: NotificationPrefs() }
    override suspend fun saveNotificationPrefs(prefs: NotificationPrefs) = latency { this.prefs[meId] = prefs; prefs }
    override suspend fun registerDevice(token: String) {}
    override suspend fun unregisterDevice(token: String) {}

    private suspend fun <T> latency(ms: Long = 150, block: () -> T): T { delay(ms); return block() }
}
