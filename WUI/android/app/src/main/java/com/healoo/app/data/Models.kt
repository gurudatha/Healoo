package com.healoo.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// DataItem v2 (Documentation/DataItem_Design.md): a container for one case with a fixed
// primary kind plus messages, attachments, appointments and alerts. JSON matches the server.

/** What an item started as (fixed at creation). */
@Serializable
enum class PrimaryKind { APPOINTMENT, MESSAGE, ALERT, REPORT;
    val label: String get() = name.lowercase().replaceFirstChar { it.uppercase() }
}

/** Part kinds listed in DataItem.kinds. */
object PartKind {
    const val APPOINTMENT = "APPOINTMENT"
    const val MESSAGE = "MESSAGE"
    const val ALERT = "ALERT"
    const val REPORT = "REPORT"
    const val ATTACHMENT = "ATTACHMENT"
}

@Serializable enum class ItemStatus { OPEN, CLOSED }
@Serializable enum class AttachmentKind { IMAGE, PDF }
@Serializable enum class Role { PATIENT, DOCTOR, ASSISTANT, LAB, HOSPITAL, ADMINISTRATOR, APP_ADMINISTRATOR }
@Serializable enum class GranteeType { USER, HOSPITAL }
@Serializable enum class Frequency { DAILY, WEEKLY, BIWEEKLY, MONTHLY, QUARTERLY;
    val label: String get() = when (this) {
        DAILY -> "Daily"; WEEKLY -> "Weekly"; BIWEEKLY -> "Every 2 weeks"; MONTHLY -> "Monthly"; QUARTERLY -> "Every 3 months"
    }
}
@Serializable enum class Period { ONE_MONTH, TWO_MONTHS, THREE_MONTHS, SIX_MONTHS;
    val months: Int get() = when (this) { ONE_MONTH -> 1; TWO_MONTHS -> 2; THREE_MONTHS -> 3; SIX_MONTHS -> 6 }
    val label: String get() = if (months == 1) "1 month" else "$months months"
}

@Serializable
data class Recurrence(val frequency: Frequency, val period: Period? = null) {
    val label: String get() = frequency.label + (period?.let { " for ${it.label}" } ?: " until cancelled")
}

@Serializable
data class Attachment(
    @SerialName("attachment_id") val id: String = "",
    val kind: AttachmentKind,
    val uri: String,
    val mime: String,
    val size: Long,
    val position: Int,
    val name: String,
    @SerialName("page_count") val pageCount: Int? = null,
    @SerialName("thumb_uri") val thumbUri: String? = null,
    @SerialName("added_by") val addedBy: String = "",
    @SerialName("is_report") val isReport: Boolean = false,
)

@Serializable
data class Grant(
    @SerialName("grant_id") val grantId: String,
    @SerialName("grantee_type") val granteeType: GranteeType,
    @SerialName("grantee_id") val granteeId: String,
    @SerialName("grantee_name") val granteeName: String,
    @SerialName("via_hospital_id") val viaHospitalId: String? = null,
)

@Serializable
data class UserProfile(
    @SerialName("user_id") val id: String,
    @SerialName("public_id") val publicId: String,
    @SerialName("display_name") val displayName: String,
    val roles: List<Role>,
    val headline: String = "",                          // e.g. "Doctor · General Medicine"
    @SerialName("photo_uri") val photoUri: String? = null,
    val location: String? = null,
    val hospital: String? = null,
    @SerialName("official_number") val officialNumber: String? = null,
    val connected: Boolean = false,
) {
    val initials: String get() = displayName.replace("Dr. ", "").split(" ")
        .filter { it.isNotBlank() }.take(2).joinToString("") { it.first().uppercase() }
    val primaryRole: Role get() = roles.firstOrNull() ?: Role.PATIENT
}

/** A message in an item's discussion (every message belongs to an item, design D5). */
@Serializable
data class Message(
    @SerialName("message_id") val id: String,
    @SerialName("item_id") val itemId: String,
    @SerialName("sender_id") val senderId: String,
    val body: String,
    @SerialName("sent_at") val sentAt: String,          // ISO timestamp with offset
    @SerialName("attachment_ids") val attachmentIds: List<String> = emptyList(),
)

/** One occurrence of an appointment. status: SCHEDULED | COMPLETED | CANCELLED | NO_SHOW */
@Serializable
data class Visit(
    val date: String,                                   // YYYY-MM-DD (after any move)
    val time: String,                                   // HH:MM
    @SerialName("original_date") val originalDate: String,  // the series date; used for visit actions
    val status: String,
)

/** A per-visit change. action: CANCELLED | MOVED */
@Serializable
data class VisitException(
    val date: String,
    val action: String,
    @SerialName("new_date") val newDate: String? = null,
    @SerialName("new_time") val newTime: String? = null,
)

@Serializable
data class Appointment(
    @SerialName("appointment_id") val id: String,
    @SerialName("item_id") val itemId: String,
    @SerialName("patient_id") val patientId: String,
    @SerialName("doctor_id") val doctorId: String,
    @SerialName("doctor_name") val doctorName: String = "",
    @SerialName("hospital_id") val hospitalId: String? = null,
    val date: String,                                   // first visit, YYYY-MM-DD
    val time: String,                                   // HH:MM
    val timezone: String = "Asia/Kolkata",
    @SerialName("duration_min") val durationMin: Int = 15,
    val notes: String? = null,
    val recurrence: Recurrence? = null,
    val exceptions: List<VisitException> = emptyList(),
    val status: String = "SCHEDULED",                   // series: SCHEDULED | COMPLETED | CANCELLED
    @SerialName("visit_count") val visitCount: Int? = null,   // null = until cancelled
    val visits: List<Visit> = emptyList(),              // upcoming visits (server sends the next 12)
) {
    val nextVisit: Visit? get() = visits.firstOrNull { it.status == "SCHEDULED" }
}

/** type: MEDICATION | APPOINTMENT_REMINDER | FOLLOW_UP | RESULT_READY | CUSTOM */
@Serializable
data class Alert(
    @SerialName("alert_id") val id: String,
    @SerialName("item_id") val itemId: String,
    val type: String,
    val text: String,
    @SerialName("for_user") val forUser: String,
    @SerialName("fires_at") val firesAt: String,        // ISO timestamp with offset (first firing)
    val timezone: String = "Asia/Kolkata",
    val recurrence: Recurrence? = null,
    @SerialName("appointment_id") val appointmentId: String? = null,
    val active: Boolean = true,
)

/** Present when the item is closed. Rating is only accepted from the owner (patient). */
@Serializable
data class Closure(
    @SerialName("closed_at") val closedAt: String,
    @SerialName("closed_by") val closedBy: String,
    val feedback: String? = null,
    val rating: Int? = null,
)

@Serializable
data class ItemCounts(
    val messages: Int = 0,
    val attachments: Int = 0,
    val appointments: Int = 0,
    val alerts: Int = 0,
    @SerialName("unread_messages") val unreadMessages: Int = 0,
)

/** The container: a small header plus child lists (DataItem_Design.md 1 and 3.1). */
@Serializable
data class DataItem(
    @SerialName("item_id") val id: String,
    @SerialName("owner_id") val ownerId: String,
    @SerialName("primary_kind") val primaryKind: PrimaryKind,
    val kinds: List<String> = emptyList(),              // PartKind values present in the item
    val title: String,
    val keywords: List<String> = emptyList(),
    val status: ItemStatus = ItemStatus.OPEN,
    val closure: Closure? = null,
    @SerialName("access_list") val accessList: List<Grant> = emptyList(),   // owner only
    val counts: ItemCounts = ItemCounts(),
    val messages: List<Message> = emptyList(),
    val attachments: List<Attachment> = emptyList(),
    val appointments: List<Appointment> = emptyList(),
    val alerts: List<Alert> = emptyList(),
    val links: List<String> = emptyList(),
    @SerialName("created_by_name") val createdByName: String = "",
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
    @SerialName("allowed_actions") val allowedActions: List<String> = emptyList(),
) {
    /** read, meta, message, attach, book, alert, close, share, revoke, rate, reopen */
    fun can(action: String): Boolean = action in allowedActions

    val nextVisit: Visit? get() = appointments.mapNotNull { it.nextVisit }.minByOrNull { it.date + it.time }

    /** Date shown in lists: next visit for appointments, otherwise last activity. */
    val date: String get() = (if (primaryKind == PrimaryKind.APPOINTMENT) nextVisit?.date else null)
        ?: updatedAt.take(10).ifEmpty { createdAt.take(10) }

    /** One-line summary for lists, based on the primary part. */
    val subtitle: String get() {
        val extra = buildList {
            if (primaryKind != PrimaryKind.MESSAGE && counts.messages > 0) add("${counts.messages} msg")
            if (primaryKind != PrimaryKind.APPOINTMENT && counts.appointments > 0) add("appointment")
        }.joinToString(" · ")
        val main = when (primaryKind) {
            PrimaryKind.REPORT -> listOf(createdByName.ifEmpty { "Report" },
                if (counts.attachments > 0) "${counts.attachments} file${if (counts.attachments == 1) "" else "s"}" else "")
                .filter { it.isNotEmpty() }.joinToString(" · ")
            PrimaryKind.APPOINTMENT -> appointments.firstOrNull()?.let { a ->
                a.doctorName + (nextVisit?.let { " · ${it.date} ${it.time}" } ?: if (status == ItemStatus.CLOSED) " · closed" else "")
            } ?: "Appointment"
            PrimaryKind.MESSAGE -> messages.lastOrNull()?.body ?: "Discussion"
            PrimaryKind.ALERT -> alerts.firstOrNull()?.let { "${it.text} · ${it.firesAt.drop(11).take(5)}" } ?: "Alert"
        }
        return listOf(main, extra).filter { it.isNotEmpty() }.joinToString(" · ")
    }
}

/** Messages tab row: an item with a discussion, with one other person. */
@Serializable
data class Conversation(
    @SerialName("item_id") val itemId: String,
    @SerialName("item_title") val itemTitle: String,
    @SerialName("primary_kind") val primaryKind: PrimaryKind,
    val status: ItemStatus = ItemStatus.OPEN,
    @SerialName("other_user") val other: UserProfile,
    @SerialName("last_message") val lastMessage: String,
    @SerialName("last_message_at") val lastMessageAt: String,
    val unread: Int = 0,
)

/** One visit in the calendar (GET /v1/appointments). */
@Serializable
data class CalendarVisit(
    @SerialName("starts_at") val startsAt: String,
    @SerialName("appointment_id") val appointmentId: String,
    @SerialName("item_id") val itemId: String,
    @SerialName("item_title") val itemTitle: String,
    @SerialName("with_user_id") val withUserId: String,
    @SerialName("with_user_name") val withUserName: String,
    val status: String,
)

@Serializable
data class Dashboard(
    @SerialName("open_reports") val openReports: Int,
    @SerialName("unread_messages") val unreadMessages: Int,
    @SerialName("upcoming_appointments") val upcomingAppointments: Int,
)

@Serializable
data class PageResult<T>(val data: List<T>, @SerialName("page_state") val pageState: String? = null)

// ---- Upload ----

@Serializable
data class PresignRequest(val files: List<PresignFile>)

@Serializable
data class PresignFile(val name: String, val mime: String, val size: Long)

@Serializable
data class PresignResponse(val uploads: List<PresignedUpload>)

@Serializable
data class PresignedUpload(@SerialName("upload_url") val uploadUrl: String, val uri: String)

/** A file already uploaded through presign, to attach to an item. */
@Serializable
data class NewAttachment(val kind: AttachmentKind, val uri: String, val mime: String, val size: Long, val name: String)

@Serializable
data class NewReport(val title: String, val attachments: List<NewAttachment>, val links: List<String> = emptyList())

@Serializable
data class NewMessage(val body: String, @SerialName("client_msg_id") val clientMsgId: String)

@Serializable
data class NewAppointment(
    @SerialName("patient_id") val patientId: String,
    @SerialName("doctor_id") val doctorId: String,
    val date: String,                                   // YYYY-MM-DD
    val time: String,                                   // HH:MM
    val timezone: String = "Asia/Kolkata",
    @SerialName("duration_min") val durationMin: Int = 15,
    val notes: String? = null,
    val recurrence: Recurrence? = null,
    @SerialName("hospital_id") val hospitalId: String? = null,   // booked through a hospital's page
)

@Serializable
data class NewAlert(
    val type: String,
    val text: String,
    @SerialName("for_user") val forUser: String? = null,
    val date: String,
    val time: String,
    val timezone: String = "Asia/Kolkata",
    val recurrence: Recurrence? = null,
)

/** POST /v1/items: exactly one of appointment, message, alert or report. */
@Serializable
data class NewItemRequest(
    @SerialName("owner_id") val ownerId: String? = null,
    val title: String = "",
    val keywords: List<String> = emptyList(),
    @SerialName("share_with") val shareWith: List<String> = emptyList(),
    val appointment: NewAppointment? = null,
    val message: NewMessage? = null,
    val alert: NewAlert? = null,
    val report: NewReport? = null,
)

@Serializable data class CloseRequest(val feedback: String? = null, val rating: Int? = null)
@Serializable data class AddAttachmentsRequest(val attachments: List<NewAttachment>, @SerialName("is_report") val isReport: Boolean = false)
@Serializable data class VisitActionRequest(val action: String, @SerialName("new_date") val newDate: String? = null, @SerialName("new_time") val newTime: String? = null)
@Serializable data class AppointmentCancel(val status: String = "CANCELLED")

@Serializable
data class ConnectRequest(@SerialName("user_id") val userId: String)

/** A local file the user picked, before upload. */
data class PendingAttachment(
    val localUri: android.net.Uri,
    val kind: AttachmentKind,
    val name: String,
    val mime: String,
    val size: Long,
)

// ---- Profile, sharing overview, notification prefs, devices (added in 0.2) ----

@Serializable
data class ProfileUpdate(
    @SerialName("display_name") val displayName: String,
    val location: String?,
)

@Serializable
data class PhotoUpdate(val uri: String)

@Serializable
data class NotificationPrefs(
    @SerialName("push_messages") val pushMessages: Boolean = true,
    @SerialName("push_reports") val pushReports: Boolean = true,
    @SerialName("quiet_hours") val quietHours: Boolean = false,
    @SerialName("quiet_start") val quietStart: String = "22:00",
    @SerialName("quiet_end") val quietEnd: String = "07:00",
)

/** One grant on an item the caller owns — GET /v1/grants?owner=me */
@Serializable
data class OwnedGrant(
    @SerialName("grant_id") val grantId: String,
    @SerialName("grantee_type") val granteeType: GranteeType,
    @SerialName("grantee_id") val granteeId: String,
    @SerialName("grantee_name") val granteeName: String,
    @SerialName("item_id") val itemId: String,
    @SerialName("item_title") val itemTitle: String,
    @SerialName("primary_kind") val primaryKind: PrimaryKind = PrimaryKind.REPORT,
)

/** Active-sharing overview: everything shared with one person or hospital. */
data class ShareGroup(
    val granteeId: String,
    val granteeName: String,
    val granteeType: GranteeType,
    val grants: List<OwnedGrant>,
)

@Serializable
data class DeviceRegistration(val platform: String, val token: String)

/** Pushed over the WebSocket (design doc 4, real-time channel). */
sealed interface RealtimeEvent {
    data class NewMessage(val message: Message) : RealtimeEvent
    data class ItemChanged(val itemId: String) : RealtimeEvent
    data class ConnectionChanged(val connected: Boolean) : RealtimeEvent
}

/** Can create items for a patient in their contacts (the patient owns them). */
val UserProfile.isClinical: Boolean
    get() = primaryRole == Role.DOCTOR || primaryRole == Role.ASSISTANT || primaryRole == Role.LAB || primaryRole == Role.HOSPITAL

/** Hospital (or platform) administrator: manages users and doctors (Administration_Design.md). */
val UserProfile.isAdministrator: Boolean
    get() = primaryRole == Role.ADMINISTRATOR || primaryRole == Role.APP_ADMINISTRATOR

/** An account as an administrator sees it (GET/POST /v1/admin/...): profile plus email and status. */
@Serializable
data class AdminAccount(
    @SerialName("user_id") val id: String,
    @SerialName("public_id") val publicId: String,
    @SerialName("display_name") val displayName: String,
    val roles: List<Role>,
    val headline: String = "",
    val hospital: String? = null,
    @SerialName("official_number") val officialNumber: String? = null,
    val email: String? = null,
    val active: Boolean = true,
) {
    val profile: UserProfile get() = UserProfile(id, publicId, displayName, roles, headline, hospital = hospital, officialNumber = officialNumber)
}

/** POST /v1/admin/users and /v1/admin/doctors. Designation and number apply to doctors. */
@Serializable
data class NewAccount(
    @SerialName("display_name") val displayName: String,
    val email: String,
    val location: String? = null,
    val designation: String? = null,
    @SerialName("official_number") val officialNumber: String? = null,
)

/** Can pass an item they don't own on to a doctor (referral). */
val UserProfile.canRefer: Boolean
    get() = primaryRole == Role.DOCTOR || primaryRole == Role.LAB || primaryRole == Role.HOSPITAL
