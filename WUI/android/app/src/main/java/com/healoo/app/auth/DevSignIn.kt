package com.healoo.app.auth

import android.content.Context
import com.healoo.app.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

@Serializable
data class DevAccount(
    @SerialName("public_id") val publicId: String,
    @SerialName("display_name") val displayName: String,
    val role: String,
) {
    val initials: String get() = displayName.replace("Dr. ", "").split(" ")
        .filter { it.isNotBlank() }.take(2).joinToString("") { it.first().uppercase() }
    /** "DOCTOR" -> "Doctor", "APP_ADMINISTRATOR" -> "App administrator" */
    val roleLabel: String get() = role.lowercase().replace('_', ' ').replaceFirstChar { it.uppercase() }
}

@Serializable private data class DevAccounts(val data: List<DevAccount>)
@Serializable private data class DevTokenRequest(@SerialName("public_id") val publicId: String)
@Serializable private data class DevTokenResponse(
    @SerialName("access_token") val accessToken: String,
    @SerialName("expires_in") val expiresIn: Long,
)

class DevSignInException(message: String) : Exception(message)

/**
 * Developer sign-in for the local-network trial (backend AUTH_MODE=dev, NETWORK_MODE=lan).
 * Uses the trial server's test accounts: GET /dev/users lists them, POST /dev/token returns a
 * 12-hour token. The Healoo ID is remembered so an expired token is renewed silently.
 *
 * Only in debug builds with healoo.devSignIn=true and healoo.useFakeData=false; release builds
 * compile it off. The backend never offers these endpoints in internet mode.
 */
class DevSignIn(context: Context, baseUrl: String) {
    val isEnabled: Boolean = BuildConfig.DEBUG && BuildConfig.DEV_SIGN_IN && !BuildConfig.USE_FAKE_DATA

    private val base = baseUrl.trimEnd('/') + "/"
    private val prefs = context.getSharedPreferences("healoo_dev_auth", Context.MODE_PRIVATE)
    private val json = Json { ignoreUnknownKeys = true }
    private val http = OkHttpClient.Builder().callTimeout(10, TimeUnit.SECONDS).build()

    /** Seeded accounts offered by the server. */
    suspend fun accounts(): List<DevAccount> = withContext(Dispatchers.IO) {
        val body = call(Request.Builder().url(base + "dev/users").build(), notFound = DISABLED)
        json.decodeFromString(DevAccounts.serializer(), body).data
    }

    /** Gets a token for [publicId] and remembers it. Returns the access token. */
    suspend fun signIn(publicId: String): String = withContext(Dispatchers.IO) {
        val id = publicId.trim().uppercase()
        if (!Regex("HL-[A-Z0-9]{5}").matches(id)) throw DevSignInException("A Healoo ID looks like HL-2M9P4.")
        val payload = json.encodeToString(DevTokenRequest.serializer(), DevTokenRequest(id))
        val req = Request.Builder().url(base + "dev/token")
            .post(payload.toRequestBody("application/json".toMediaType())).build()
        val res = json.decodeFromString(DevTokenResponse.serializer(), call(req, notFound = null))
        prefs.edit()
            .putString(KEY_ID, id)
            .putString(KEY_TOKEN, res.accessToken)
            .putLong(KEY_EXPIRES, System.currentTimeMillis() + res.expiresIn * 1000)
            .apply()
        res.accessToken
    }

    /** A stored token that is still valid for at least a minute, if any. */
    fun storedToken(): String? {
        if (!isEnabled) return null
        val expires = prefs.getLong(KEY_EXPIRES, 0)
        return prefs.getString(KEY_TOKEN, null)?.takeIf { expires - System.currentTimeMillis() > 60_000 }
    }

    val hasStoredAccount: Boolean get() = isEnabled && prefs.getString(KEY_ID, null) != null

    /** New token for the remembered account, or null (then the user signs in again). */
    suspend fun renew(): String? {
        val id = prefs.getString(KEY_ID, null) ?: return null
        return runCatching { signIn(id) }.getOrNull()
    }

    fun clear() { prefs.edit().clear().apply() }

    private fun call(req: Request, notFound: String?): String {
        val response = try {
            http.newCall(req).execute()
        } catch (e: IOException) {
            // Say which kind of failure it was: certificate problems and network problems need different fixes.
            val cause = when (e) {
                is javax.net.ssl.SSLHandshakeException, is javax.net.ssl.SSLPeerUnverifiedException ->
                    "The connection worked but the certificate isn't trusted: install healoo-local-ca.crt on this device."
                is java.net.ConnectException, is java.net.SocketTimeoutException, is java.net.NoRouteToHostException ->
                    "No connection: check the server is running, the address, the firewall (port 8443) and the network."
                is java.net.UnknownHostException -> "Unknown host name: check healoo.apiBaseUrl."
                else -> "Network error."
            }
            throw DevSignInException("Can't reach the trial server at $base. $cause (${e.javaClass.simpleName}: ${e.message})")
        }
        response.use {
            val text = it.body?.string().orEmpty()
            if (it.isSuccessful) return text
            if (it.code == 404 && notFound != null) throw DevSignInException(notFound)
            val serverMessage = runCatching {
                json.parseToJsonElement(text).jsonObject["error"]?.jsonObject?.get("message")?.jsonPrimitive?.content
            }.getOrNull()
            throw DevSignInException(serverMessage ?: "The server answered ${it.code}.")
        }
    }

    private companion object {
        const val KEY_ID = "public_id"
        const val KEY_TOKEN = "token"
        const val KEY_EXPIRES = "expires_at"
        const val DISABLED = "This server doesn't offer developer sign-in. Start it with NETWORK_MODE=lan and AUTH_MODE=dev."
    }
}
