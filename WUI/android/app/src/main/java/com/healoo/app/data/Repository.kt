package com.healoo.app.data

import android.content.Context
import android.net.Uri
import com.healoo.app.BuildConfig

/** Everything the WUI needs. Fake and remote implementations are interchangeable. */
interface HealooRepository {
    suspend fun me(): UserProfile
    suspend fun dashboard(): Dashboard
    /** Visible items; [kind] (a PartKind) keeps only items containing that part, as GET /v1/items?kind= does. */
    suspend fun items(status: ItemStatus = ItemStatus.OPEN, limit: Int = 20, kind: String? = null): List<DataItem>
    suspend fun openItems(limit: Int = 20, kind: String? = null): List<DataItem> = items(ItemStatus.OPEN, limit, kind)
    /** Full item with its messages, attachments, appointments and alerts. */
    suspend fun item(id: String): DataItem
    suspend fun user(id: String): UserProfile
    /** Items shared between the caller and [userId]. Empty unless connected (doc 2.3 rule 8). */
    suspend fun sharedItems(userId: String): List<DataItem>
    /** Global search by Healoo ID or name. Returns profiles only, never data. */
    suspend fun search(query: String, role: Role?): List<UserProfile>
    /** Doctors currently working at a hospital (profiles only). */
    suspend fun hospitalDoctors(hospitalId: String): List<UserProfile>
    suspend fun connections(): List<UserProfile>
    suspend fun connect(userId: String): UserProfile
    suspend fun revokeGrant(itemId: String, grantId: String): DataItem
    /** Shares an item (POST /v1/grants): the owner with anyone; a doctor, lab or hospital with doctors. */
    suspend fun share(itemId: String, granteeId: String): DataItem

    // ---- DataItem v2: creating items from one primary part ----
    /** New REPORT item from picked files (presign, upload, create). */
    suspend fun createReport(draft: ReportDraft, files: List<PendingAttachment>): DataItem
    /** New APPOINTMENT item. */
    suspend fun createAppointment(ownerId: String?, appointment: NewAppointment, shareWith: List<String> = emptyList()): DataItem
    /** New ALERT item. */
    suspend fun createAlert(title: String, alert: NewAlert, shareWith: List<String> = emptyList()): DataItem
    /**
     * New MESSAGE item: starts a discussion with [userId] (the patient in the pair owns it), also
     * shared with [alsoWith]. A clinician writing to a patient can add doctors only.
     */
    suspend fun startConversation(userId: String, body: String, alsoWith: List<String> = emptyList()): DataItem

    // ---- DataItem v2: adding to an existing item ----
    suspend fun itemMessages(itemId: String): List<Message>
    suspend fun sendItemMessage(itemId: String, body: String): Message
    suspend fun addAttachments(itemId: String, files: List<PendingAttachment>, isReport: Boolean): DataItem
    suspend fun bookAppointment(itemId: String, appointment: NewAppointment): DataItem
    /** One visit: CANCELLED, MOVED (newDate/newTime), COMPLETED or NO_SHOW. */
    suspend fun visitAction(itemId: String, appointmentId: String, visitDate: String, action: String, newDate: String? = null, newTime: String? = null): DataItem
    suspend fun cancelAppointment(itemId: String, appointmentId: String): DataItem
    suspend fun addAlert(itemId: String, alert: NewAlert): DataItem
    suspend fun deleteAlert(itemId: String, alertId: String): DataItem
    /** Close with optional feedback; rating (1–5) only when the patient closes. */
    suspend fun closeItem(itemId: String, feedback: String?, rating: Int?): DataItem
    suspend fun reopenItem(itemId: String): DataItem

    /** Messages tab: items with a discussion, per person (optionally only with [withUser]). */
    suspend fun conversations(withUser: String? = null): List<Conversation>
    /** Upcoming visits across items (YYYY-MM-DD, up to 62 days). */
    suspend fun calendar(from: String, to: String): List<CalendarVisit>

    // ---- administration (hospital administrators; users can't be deleted) ----
    /** The administrator's hospital's doctors, deactivated ones included. */
    suspend fun adminDoctors(): List<AdminAccount>
    suspend fun adminCreateUser(account: NewAccount): AdminAccount
    suspend fun adminCreateDoctor(account: NewAccount): AdminAccount
    /** "Delete" a doctor: deactivates the account and ends the hospital affiliation. */
    suspend fun adminDeactivateDoctor(doctorId: String)
    suspend fun adminReactivateDoctor(doctorId: String): AdminAccount

    // ---- account ----
    suspend fun updateProfile(update: ProfileUpdate): UserProfile
    /** Everything the caller has shared, grouped by who it is shared with. */
    suspend fun activeShares(): List<ShareGroup>
    suspend fun notificationPrefs(): NotificationPrefs
    suspend fun saveNotificationPrefs(prefs: NotificationPrefs): NotificationPrefs
    /** FCM token registration, so the push service can reach this device (doc 8). */
    suspend fun registerDevice(token: String)
    suspend fun unregisterDevice(token: String)

    /** Live events from the WebSocket; collect while a screen is visible. */
    val events: kotlinx.coroutines.flow.SharedFlow<RealtimeEvent>
    fun startRealtime()
    fun stopRealtime()
}

/** Upload screen: a new report, or files added to an existing item ([addToItemId]). */
data class ReportDraft(
    val ownerId: String,
    val title: String,
    val keywords: List<String>,
    val links: List<String>,
    val shareWith: List<String>,
)

object Limits {
    const val MAX_ATTACHMENTS = 20
    const val IMAGE_MAX_BYTES = 10L * 1024 * 1024
    const val PDF_MAX_BYTES = 25L * 1024 * 1024
}

/** Tiny service locator — enough for the trial; swap for Hilt later if wanted. */
object ServiceLocator {
    lateinit var repository: HealooRepository
        private set
    lateinit var appContext: Context
        private set

    lateinit var auth: com.healoo.app.auth.AuthManager
        private set

    fun init(context: Context) {
        appContext = context.applicationContext
        PageOperations.load(appContext)
        auth = com.healoo.app.auth.AuthManager(appContext)
        repository = if (BuildConfig.USE_FAKE_DATA) FakeRepository()
        else RemoteRepository(appContext, BuildConfig.API_BASE_URL, auth)
    }
}

/** Current Auth0 access token (design doc 5.4); kept fresh by AuthManager. */
object TokenStore {
    @Volatile var accessToken: String? = null
}

fun Uri.isPdf(mime: String?) = mime == "application/pdf" || toString().endsWith(".pdf", ignoreCase = true)
