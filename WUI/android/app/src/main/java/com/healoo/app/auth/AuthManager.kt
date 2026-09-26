package com.healoo.app.auth

import android.content.Context
import com.auth0.android.Auth0
import com.auth0.android.authentication.AuthenticationAPIClient
import com.auth0.android.authentication.storage.CredentialsManager
import com.auth0.android.authentication.storage.SharedPreferencesStorage
import com.auth0.android.provider.WebAuthProvider
import com.auth0.android.result.Credentials
import com.healoo.app.BuildConfig
import com.healoo.app.data.TokenStore
import com.healoo.app.data.UserProfile
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

sealed interface SessionState {
    data object Checking : SessionState
    data object SignedOut : SessionState
    data class SignedIn(val me: UserProfile) : SessionState
}

/**
 * Auth0 Universal Login (design doc 5).
 * - login(): opens the hosted login page in a Custom Tab; PKCE is handled by the SDK.
 * - Credentials (incl. refresh token, scope offline_access) are stored by CredentialsManager.
 * - freshToken(): returns a valid access token, refreshing it when it is about to expire.
 * When AUTH0_DOMAIN is empty the app runs in demo mode and this class is not used for sign-in.
 *
 * Debug builds can also use [DevSignIn] (trial server with AUTH_MODE=dev). The manager remembers
 * which method signed in, so restore, token renewal and logout go to the right place.
 */
class AuthManager(context: Context) {
    val isConfigured: Boolean = BuildConfig.AUTH0_DOMAIN.isNotBlank() && BuildConfig.AUTH0_CLIENT_ID.isNotBlank()

    val dev = DevSignIn(context, BuildConfig.API_BASE_URL)

    private enum class Method { AUTH0, DEV }
    @Volatile private var method: Method? = null

    private val account: Auth0? = if (isConfigured) Auth0(BuildConfig.AUTH0_CLIENT_ID, BuildConfig.AUTH0_DOMAIN) else null
    private val credentials: CredentialsManager? = account?.let {
        CredentialsManager(AuthenticationAPIClient(it), SharedPreferencesStorage(context, "healoo_auth"))
    }
    private val refreshLock = Mutex()

    private val _session = MutableStateFlow<SessionState>(SessionState.Checking)
    val session: StateFlow<SessionState> = _session

    fun signedIn(me: UserProfile) { _session.value = SessionState.SignedIn(me) }
    fun signedOut() { TokenStore.accessToken = null; _session.value = SessionState.SignedOut }

    /** True if a stored session was restored (tokens refreshed if needed). */
    suspend fun restore(): Boolean {
        if (dev.hasStoredAccount) {
            val token = dev.storedToken() ?: dev.renew()
            if (token != null) { useDev(token); return true }
        }
        val cm = credentials ?: return false
        if (!cm.hasValidCredentials()) return false
        return runCatching { use(cm.awaitCredentials()) }.isSuccess
    }

    /** Developer sign-in with a seeded Healoo ID (debug builds, trial server only). */
    suspend fun devLogin(publicId: String) {
        check(dev.isEnabled) { "Developer sign-in is off in this build" }
        useDev(dev.signIn(publicId))
    }

    suspend fun login(activityContext: Context) {
        val acc = requireNotNull(account) { "Auth0 is not configured" }
        val result = WebAuthProvider.login(acc)
            .withScheme(BuildConfig.AUTH0_SCHEME)
            .withAudience(BuildConfig.AUTH0_AUDIENCE)
            .withScope("openid profile email offline_access")
            .await(activityContext)
        credentials?.saveCredentials(result)
        dev.clear()
        use(result)
    }

    suspend fun logout(activityContext: Context) {
        if (method == Method.AUTH0) account?.let { acc ->
            runCatching { WebAuthProvider.logout(acc).withScheme(BuildConfig.AUTH0_SCHEME).await(activityContext) }
        }
        credentials?.clearCredentials()
        dev.clear()
        method = null
        signedOut()
    }

    /** Called by the HTTP client on 401: refresh once, return the new token or null. */
    suspend fun freshToken(): String? = refreshLock.withLock {
        if (method == Method.DEV) return dev.renew()?.let { useDev(it) }
        val cm = credentials ?: return null
        runCatching { use(cm.awaitCredentials(null, 60)) }.getOrNull()
    }

    private fun use(c: Credentials): String {
        method = Method.AUTH0
        TokenStore.accessToken = c.accessToken
        return c.accessToken
    }

    private fun useDev(token: String): String {
        method = Method.DEV
        TokenStore.accessToken = token
        return token
    }
}
