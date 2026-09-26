package com.healoo.app.data

import android.content.Context
import android.net.Uri
import com.healoo.app.BuildConfig

/** Everything the WUI needs. Fake and remote implementations are interchangeable. */
interface HealooRepository {
    suspend fun me(): UserProfile
    suspend fun dashboard(): Dashboard
    suspend fun openItems(limit: Int = 20): List<DataItem>
    suspend fun item(id: String): DataItem
    suspend fun user(id: String): UserProfile
    /** Items shared between the caller and [userId]. Empty unless connected (doc 2.3 rule 8). */
    suspend fun sharedItems(userId: String): List<DataItem>
    suspend fun messages(userId: String): List<Message>
    suspend fun sendMessage(userId: String, body: String): Message
    suspend fun threads(): List<ThreadSummary>
    /** Global search by Healoo ID or name. Returns profiles only, never data. */
    suspend fun search(query: String, role: Role?): List<UserProfile>
    suspend fun connections(): List<UserProfile>
    suspend fun connect(userId: String): UserProfile
    suspend fun setStatus(itemId: String, status: ItemStatus): DataItem
    suspend fun revokeGrant(itemId: String, grantId: String): DataItem
    /** Patient shares an item with a user or hospital (POST /v1/grants). */
    suspend fun share(itemId: String, granteeId: String): DataItem
    suspend fun upload(draft: UploadDraft, files: List<PendingAttachment>): DataItem

    // ---- added in 0.2 ----
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

data class UploadDraft(
    val ownerId: String,
    val type: CoreItemType,
    val title: String,
    val date: String,
    val keywords: List<String>,
    val links: List<String>,
    val status: ItemStatus,
    val shareWith: List<String>,
    val pointerToMessage: String?,
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
