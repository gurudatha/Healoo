package com.healoo.app.data

import android.content.Context
import retrofit2.converter.kotlinx.serialization.asConverterFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import com.healoo.app.auth.AuthManager
import com.healoo.app.realtime.RealtimeClient
import retrofit2.http.PUT
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okio.BufferedSink
import okio.source
import retrofit2.Retrofit
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.PATCH
import retrofit2.http.POST
import retrofit2.http.Path
import retrofit2.http.Query

/** Endpoints from design doc 4.3. */
interface HealooApi {
    @GET("v1/me") suspend fun me(): UserProfile
    @GET("v1/dashboard") suspend fun dashboard(): Dashboard
    @GET("v1/items") suspend fun items(@Query("status") status: String, @Query("limit") limit: Int): PageResult<DataItem>
    @GET("v1/items/{id}") suspend fun item(@Path("id") id: String): DataItem
    @GET("v1/items/{id}/attachments") suspend fun attachments(@Path("id") id: String): PageResult<Attachment>
    @PATCH("v1/items/{id}") suspend fun patchItem(@Path("id") id: String, @Body body: StatusPatch): DataItem
    @POST("v1/items") suspend fun createItem(@Body body: NewItemRequest): DataItem
    @POST("v1/uploads/presign") suspend fun presign(@Body body: PresignRequest): PresignResponse
    @GET("v1/users/{id}") suspend fun user(@Path("id") id: String): UserProfile
    @GET("v1/users/{id}/shared-items") suspend fun shared(@Path("id") id: String, @Query("status") status: String = "OPEN"): PageResult<DataItem>
    @GET("v1/threads") suspend fun threads(): PageResult<ThreadSummary>
    @GET("v1/threads") suspend fun threadWith(@Query("with") userId: String): PageResult<ThreadSummary>
    @GET("v1/threads/{id}/messages") suspend fun messages(@Path("id") threadId: String): PageResult<Message>
    @POST("v1/threads/{id}/messages") suspend fun send(@Path("id") threadId: String, @Body body: SendMessage): Message
    @GET("v1/search") suspend fun search(@Query("q") q: String, @Query("type") type: String?): PageResult<UserProfile>
    @GET("v1/connections") suspend fun connections(): PageResult<UserProfile>
    @POST("v1/connections") suspend fun connect(@Body body: ConnectRequest): UserProfile
    @DELETE("v1/grants/{id}") suspend fun revoke(@Path("id") grantId: String)
    @POST("v1/grants") suspend fun grant(@Body body: GrantRequest)
    @GET("v1/grants") suspend fun ownedGrants(@Query("owner") owner: String = "me"): PageResult<OwnedGrant>
    @PATCH("v1/me") suspend fun updateMe(@Body body: ProfileUpdate): UserProfile
    @GET("v1/me/notification-prefs") suspend fun prefs(): NotificationPrefs
    @PUT("v1/me/notification-prefs") suspend fun savePrefs(@Body body: NotificationPrefs): NotificationPrefs
    @POST("v1/devices") suspend fun registerDevice(@Body body: DeviceRegistration)
    @DELETE("v1/devices/{token}") suspend fun unregisterDevice(@Path("token") token: String)
}

@Serializable data class StatusPatch(val status: ItemStatus)
@Serializable data class GrantRequest(@kotlinx.serialization.SerialName("item_ids") val itemIds: List<String>, @kotlinx.serialization.SerialName("grantee_id") val granteeId: String)
@Serializable data class SendMessage(val body: String, @kotlinx.serialization.SerialName("client_msg_id") val clientMsgId: String)

class RemoteRepository(
    private val context: Context,
    baseUrl: String,
    private val auth: AuthManager,
) : HealooRepository {
    private val token: () -> String? = { TokenStore.accessToken }

    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false }

    private val http = OkHttpClient.Builder()
        .addInterceptor { chain ->
            val req = chain.request().newBuilder().apply {
                token()?.let { header("Authorization", "Bearer $it") }
            }.build()
            chain.proceed(req)
        }
        // Expired access token -> refresh once through Auth0, then retry (doc 5.4).
        .authenticator { _, response ->
            if (response.priorResponse != null) return@authenticator null
            val fresh = runBlocking { auth.freshToken() } ?: return@authenticator null
            response.request.newBuilder().header("Authorization", "Bearer $fresh").build()
        }
        .build()

    private val realtime = RealtimeClient(baseUrl, http, token, json, refreshToken = { auth.freshToken() })
    override val events = realtime.events
    override fun startRealtime() = realtime.start()
    override fun stopRealtime() = realtime.stop()

    private val api: HealooApi = Retrofit.Builder()
        .baseUrl(baseUrl)
        .client(http)
        .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
        .build()
        .create(HealooApi::class.java)

    override suspend fun me() = api.me()
    override suspend fun dashboard() = api.dashboard()
    override suspend fun openItems(limit: Int) = api.items("OPEN", limit).data
    override suspend fun item(id: String): DataItem {
        val item = api.item(id)
        // Attachment view URLs are short-lived and fetched separately (doc 3.6 / 4.3).
        val files = runCatching { api.attachments(id).data }.getOrDefault(item.attachments)
        return item.copy(attachments = files.sortedBy { it.position })
    }
    override suspend fun user(id: String) = api.user(id)
    override suspend fun sharedItems(userId: String) = api.shared(userId).data

    override suspend fun messages(userId: String): List<Message> {
        val thread = api.threadWith(userId).data.firstOrNull() ?: return emptyList()
        return api.messages(thread.id).data
    }

    override suspend fun sendMessage(userId: String, body: String): Message {
        val thread = api.threadWith(userId).data.first()
        return api.send(thread.id, SendMessage(body, java.util.UUID.randomUUID().toString()))
    }

    override suspend fun threads() = api.threads().data
    override suspend fun search(query: String, role: Role?) = api.search(query, role?.name).data
    override suspend fun connections() = api.connections().data
    override suspend fun connect(userId: String) = api.connect(ConnectRequest(userId))
    override suspend fun setStatus(itemId: String, status: ItemStatus) = api.patchItem(itemId, StatusPatch(status))

    override suspend fun revokeGrant(itemId: String, grantId: String): DataItem {
        api.revoke(grantId)
        return item(itemId)
    }

    override suspend fun share(itemId: String, granteeId: String): DataItem {
        api.grant(GrantRequest(listOf(itemId), granteeId))
        return item(itemId)
    }

    override suspend fun updateProfile(update: ProfileUpdate) = api.updateMe(update)

    override suspend fun activeShares(): List<ShareGroup> =
        api.ownedGrants().data.groupBy { it.granteeId }.map { (id, grants) ->
            ShareGroup(id, grants.first().granteeName, grants.first().granteeType, grants.sortedBy { it.itemTitle })
        }.sortedBy { it.granteeName }

    override suspend fun notificationPrefs() = api.prefs()
    override suspend fun saveNotificationPrefs(prefs: NotificationPrefs) = api.savePrefs(prefs)
    override suspend fun registerDevice(token: String) = api.registerDevice(DeviceRegistration("android", token))
    override suspend fun unregisterDevice(token: String) = api.unregisterDevice(token)

    /** Presign all files in one call, PUT each to storage, then create the DataItem. */
    override suspend fun upload(draft: UploadDraft, files: List<PendingAttachment>): DataItem {
        val presigned = api.presign(PresignRequest(files.map { PresignFile(it.name, it.mime, it.size) })).uploads
        withContext(Dispatchers.IO) {
            files.zip(presigned).forEach { (file, target) ->
                val body = object : RequestBody() {
                    override fun contentType() = file.mime.toMediaType()
                    override fun contentLength() = file.size
                    override fun writeTo(sink: BufferedSink) {
                        context.contentResolver.openInputStream(file.localUri)!!.source().use { sink.writeAll(it) }
                    }
                }
                http.newCall(Request.Builder().url(target.uploadUrl).put(body).build()).execute().use { resp ->
                    check(resp.isSuccessful) { "Upload of ${file.name} failed (${resp.code})" }
                }
            }
        }
        val attachments = files.zip(presigned).mapIndexed { i, (f, p) ->
            Attachment(f.kind, p.uri, f.mime, f.size, position = i, name = f.name)
        }
        return api.createItem(
            NewItemRequest(draft.ownerId, draft.type, draft.title, draft.date, draft.keywords, attachments,
                draft.links, draft.status, draft.shareWith, draft.pointerToMessage)
        )
    }
}
