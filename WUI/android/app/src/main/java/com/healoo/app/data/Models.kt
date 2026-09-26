package com.healoo.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Mirrors design doc 2.2 (DataItem) and 4.3 (REST API, JSON).

@Serializable
enum class CoreItemType { REPORT, ALERT, BOOKING, FEEDBACK, PAYMENT, MESSAGE;
    val label: String get() = name.lowercase().replaceFirstChar { it.uppercase() }
}

@Serializable enum class ItemStatus { OPEN, CLOSED }
@Serializable enum class AttachmentKind { IMAGE, PDF }
@Serializable enum class Role { PATIENT, DOCTOR, ASSISTANT, LAB, HOSPITAL, ADMINISTRATOR, APP_ADMINISTRATOR }
@Serializable enum class GranteeType { USER, HOSPITAL }

@Serializable
data class Attachment(
    val kind: AttachmentKind,
    val uri: String,
    val mime: String,
    val size: Long,
    val position: Int,
    val name: String,
    @SerialName("page_count") val pageCount: Int? = null,
    @SerialName("thumb_uri") val thumbUri: String? = null,
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
data class DataItem(
    @SerialName("item_id") val id: String,
    val date: String,                                   // ISO date, e.g. 2026-09-20
    @SerialName("core_item_type") val type: CoreItemType,
    val title: String,
    val subtitle: String = "",
    val keywords: List<String> = emptyList(),
    @SerialName("core_item_data") val attachments: List<Attachment> = emptyList(),
    val links: List<String> = emptyList(),
    @SerialName("owner_id") val ownerId: String,
    @SerialName("created_by_name") val createdByName: String = "",
    @SerialName("access_list") val accessList: List<Grant> = emptyList(),
    @SerialName("pointer_to_message") val pointerToMessage: String? = null,
    @SerialName("pointer_item_id") val pointerItemId: String? = null,
    val rating: Int? = null,
    val status: ItemStatus = ItemStatus.OPEN,
    @SerialName("allowed_actions") val allowedActions: List<String> = emptyList(),
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

@Serializable
data class Message(
    @SerialName("message_id") val id: String,
    @SerialName("thread_id") val threadId: String,
    @SerialName("sender_id") val senderId: String,
    val body: String,
    @SerialName("sent_at") val sentAt: String,          // display time for the trial
    @SerialName("linked_item_id") val linkedItemId: String? = null,
)

@Serializable
data class ThreadSummary(
    @SerialName("thread_id") val id: String,
    @SerialName("other_user") val other: UserProfile,
    @SerialName("last_message") val lastMessage: String,
    @SerialName("last_message_at") val lastMessageAt: String,
    val unread: Int = 0,
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

@Serializable
data class NewItemRequest(
    @SerialName("owner_id") val ownerId: String,
    @SerialName("core_item_type") val type: CoreItemType,
    val title: String,
    val date: String,
    val keywords: List<String>,
    @SerialName("core_item_data") val attachments: List<Attachment>,
    val links: List<String>,
    val status: ItemStatus,
    @SerialName("share_with") val shareWith: List<String>,   // user or "hospital:<id>" keys
    @SerialName("pointer_to_message") val pointerToMessage: String?,
)

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
    @SerialName("core_item_type") val type: CoreItemType,
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

val UserProfile.isClinical: Boolean
    get() = primaryRole == Role.DOCTOR || primaryRole == Role.ASSISTANT || primaryRole == Role.LAB
