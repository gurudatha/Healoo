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

/** Endpoints from design doc 4.3 and DataItem_Design.md section 4. */
interface HealooApi {
    @GET("v1/me") suspend fun me(): UserProfile
    @GET("v1/dashboard") suspend fun dashboard(): Dashboard
    @GET("v1/items") suspend fun items(@Query("status") status: String, @Query("limit") limit: Int, @Query("kind") kind: String? = null): PageResult<DataItem>
    @GET("v1/items/{id}") suspend fun item(@Path("id") id: String): DataItem
    @POST("v1/items") suspend fun createItem(@Body body: NewItemRequest): DataItem
    @POST("v1/items/{id}/close") suspend fun close(@Path("id") id: String, @Body body: CloseRequest): DataItem
    @POST("v1/items/{id}/reopen") suspend fun reopen(@Path("id") id: String): DataItem
    @GET("v1/items/{id}/messages") suspend fun messages(@Path("id") id: String): PageResult<Message>
    @POST("v1/items/{id}/messages") suspend fun send(@Path("id") id: String, @Body body: NewMessage): Message
    @POST("v1/items/{id}/attachments") suspend fun addAttachments(@Path("id") id: String, @Body body: AddAttachmentsRequest): PageResult<Attachment>
    @POST("v1/items/{id}/appointments") suspend fun book(@Path("id") id: String, @Body body: NewAppointment): Appointment
    @PATCH("v1/items/{id}/appointments/{aid}") suspend fun cancelAppointment(@Path("id") id: String, @Path("aid") aid: String, @Body body: AppointmentCancel): Appointment
    @POST("v1/items/{id}/appointments/{aid}/visits/{date}") suspend fun visit(@Path("id") id: String, @Path("aid") aid: String, @Path("date") date: String, @Body body: VisitActionRequest): Appointment
    @POST("v1/items/{id}/alerts") suspend fun addAlert(@Path("id") id: String, @Body body: NewAlert): Alert
    @DELETE("v1/items/{id}/alerts/{alertId}") suspend fun deleteAlert(@Path("id") id: String, @Path("alertId") alertId: String)
    @GET("v1/conversations") suspend fun conversations(@Query("with") with: String?): PageResult<Conversation>
    @GET("v1/appointments") suspend fun calendar(@Query("from") from: String, @Query("to") to: String): PageResult<CalendarVisit>
    @POST("v1/uploads/presign") suspend fun presign(@Body body: PresignRequest): PresignResponse
    @GET("v1/users/{id}") suspend fun user(@Path("id") id: String): UserProfile
    @GET("v1/users/{id}/shared-items") suspend fun shared(@Path("id") id: String): PageResult<DataItem>
    @GET("v1/search") suspend fun search(@Query("q") q: String, @Query("type") type: String?): PageResult<UserProfile>
    @GET("v1/hospitals/{id}/doctors") suspend fun hospitalDoctors(@Path("id") id: String): PageResult<UserProfile>
    @GET("v1/connections") suspend fun connections(): PageResult<UserProfile>
    @GET("v1/admin/doctors") suspend fun adminDoctors(): PageResult<AdminAccount>
    @POST("v1/admin/users") suspend fun adminCreateUser(@Body body: NewAccount): AdminAccount
    @POST("v1/admin/doctors") suspend fun adminCreateDoctor(@Body body: NewAccount): AdminAccount
    @DELETE("v1/admin/doctors/{id}") suspend fun adminDeactivateDoctor(@Path("id") id: String)
    @POST("v1/admin/doctors/{id}/reactivate") suspend fun adminReactivateDoctor(@Path("id") id: String): AdminAccount
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

@Serializable data class GrantRequest(@kotlinx.serialization.SerialName("item_ids") val itemIds: List<String>, @kotlinx.serialization.SerialName("grantee_id") val granteeId: String)

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
    override suspend fun items(status: ItemStatus, limit: Int, kind: String?) = api.items(status.name, limit, kind).data
    override suspend fun item(id: String) = api.item(id).let { it.copy(attachments = it.attachments.sortedBy { a -> a.position }) }
    override suspend fun user(id: String) = api.user(id)
    override suspend fun sharedItems(userId: String) = api.shared(userId).data
    override suspend fun search(query: String, role: Role?) = api.search(query, role?.name).data
    override suspend fun hospitalDoctors(hospitalId: String) = api.hospitalDoctors(hospitalId).data
    override suspend fun connections() = api.connections().data
    override suspend fun connect(userId: String) = api.connect(ConnectRequest(userId))

    override suspend fun revokeGrant(itemId: String, grantId: String): DataItem { api.revoke(grantId); return item(itemId) }
    override suspend fun share(itemId: String, granteeId: String): DataItem { api.grant(GrantRequest(listOf(itemId), granteeId)); return item(itemId) }

    // ---- create ----

    override suspend fun createReport(draft: ReportDraft, files: List<PendingAttachment>): DataItem =
        api.createItem(NewItemRequest(
            ownerId = draft.ownerId, keywords = draft.keywords, shareWith = draft.shareWith,
            report = NewReport(draft.title, uploadFiles(files), draft.links),
        ))

    override suspend fun createAppointment(ownerId: String?, appointment: NewAppointment, shareWith: List<String>) =
        api.createItem(NewItemRequest(ownerId = ownerId, shareWith = shareWith, appointment = appointment))

    override suspend fun createAlert(title: String, alert: NewAlert, shareWith: List<String>) =
        api.createItem(NewItemRequest(title = title, shareWith = shareWith, alert = alert))

    override suspend fun startConversation(userId: String, body: String, alsoWith: List<String>): DataItem {
        val me = api.me()
        val other = api.user(userId)
        // The patient in the pair owns the discussion; a clinician starts it on the patient's behalf.
        val (owner, share) = if (other.primaryRole == Role.PATIENT && me.isClinical) other.id to alsoWith else me.id to listOf(userId) + alsoWith
        return api.createItem(NewItemRequest(ownerId = owner, shareWith = share.distinct(), message = NewMessage(body, clientId())))
    }

    // ---- add to an item ----

    override suspend fun itemMessages(itemId: String) = api.messages(itemId).data
    override suspend fun sendItemMessage(itemId: String, body: String) = api.send(itemId, NewMessage(body, clientId()))
    override suspend fun addAttachments(itemId: String, files: List<PendingAttachment>, isReport: Boolean): DataItem {
        api.addAttachments(itemId, AddAttachmentsRequest(uploadFiles(files), isReport)); return item(itemId)
    }
    override suspend fun bookAppointment(itemId: String, appointment: NewAppointment): DataItem { api.book(itemId, appointment); return item(itemId) }
    override suspend fun visitAction(itemId: String, appointmentId: String, visitDate: String, action: String, newDate: String?, newTime: String?): DataItem {
        api.visit(itemId, appointmentId, visitDate, VisitActionRequest(action, newDate, newTime)); return item(itemId)
    }
    override suspend fun cancelAppointment(itemId: String, appointmentId: String): DataItem {
        api.cancelAppointment(itemId, appointmentId, AppointmentCancel()); return item(itemId)
    }
    override suspend fun addAlert(itemId: String, alert: NewAlert): DataItem { api.addAlert(itemId, alert); return item(itemId) }
    override suspend fun deleteAlert(itemId: String, alertId: String): DataItem { api.deleteAlert(itemId, alertId); return item(itemId) }
    override suspend fun closeItem(itemId: String, feedback: String?, rating: Int?) = api.close(itemId, CloseRequest(feedback, rating))
    override suspend fun reopenItem(itemId: String) = api.reopen(itemId)

    override suspend fun conversations(withUser: String?) = api.conversations(withUser).data
    override suspend fun calendar(from: String, to: String) = api.calendar(from, to).data

    // ---- administration ----

    override suspend fun adminDoctors() = api.adminDoctors().data
    override suspend fun adminCreateUser(account: NewAccount) = api.adminCreateUser(account)
    override suspend fun adminCreateDoctor(account: NewAccount) = api.adminCreateDoctor(account)
    override suspend fun adminDeactivateDoctor(doctorId: String) = api.adminDeactivateDoctor(doctorId)
    override suspend fun adminReactivateDoctor(doctorId: String) = api.adminReactivateDoctor(doctorId)

    // ---- account ----

    override suspend fun updateProfile(update: ProfileUpdate) = api.updateMe(update)

    override suspend fun activeShares(): List<ShareGroup> =
        api.ownedGrants().data.groupBy { it.granteeId }.map { (id, grants) ->
            ShareGroup(id, grants.first().granteeName, grants.first().granteeType, grants.sortedBy { it.itemTitle })
        }.sortedBy { it.granteeName }

    override suspend fun notificationPrefs() = api.prefs()
    override suspend fun saveNotificationPrefs(prefs: NotificationPrefs) = api.savePrefs(prefs)
    override suspend fun registerDevice(token: String) = api.registerDevice(DeviceRegistration("android", token))
    override suspend fun unregisterDevice(token: String) = api.unregisterDevice(token)

    private fun clientId() = java.util.UUID.randomUUID().toString()

    /** Presign all files in one call and PUT each to storage; returns them ready to attach. */
    private suspend fun uploadFiles(files: List<PendingAttachment>): List<NewAttachment> {
        if (files.isEmpty()) return emptyList()
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
        return files.zip(presigned).map { (f, p) -> NewAttachment(f.kind, p.uri, f.mime, f.size, f.name) }
    }
}
