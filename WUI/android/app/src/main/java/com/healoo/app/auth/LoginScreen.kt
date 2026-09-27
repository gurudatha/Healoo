package com.healoo.app.auth

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.healoo.app.BuildConfig
import com.healoo.app.R
import com.healoo.app.data.FakeRepository
import com.healoo.app.data.ServiceLocator
import com.healoo.app.data.UserProfile
import com.healoo.app.ui.components.Avatar
import com.healoo.app.ui.components.PrimaryButton
import com.healoo.app.ui.components.verticalScrollWithBar
import com.healoo.app.ui.upload.SageTextField
import com.healoo.app.ui.theme.*
import kotlinx.coroutines.launch

@Composable
fun LoginScreen(onSignedIn: suspend () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val auth = ServiceLocator.auth
    val demo = ServiceLocator.repository as? FakeRepository
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    Column(Modifier.fillMaxSize().background(Sage.Background)) {
        Column(
            Modifier.fillMaxWidth()
                .clip(RoundedCornerShape(bottomStart = Radius.header, bottomEnd = Radius.header))
                .background(Sage.Primary).statusBarsPadding()
                .padding(horizontal = 24.dp, vertical = 40.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                // Decorative: the name next to it is the accessible label.
                Image(painterResource(R.drawable.healoo_mark), contentDescription = null, modifier = Modifier.size(52.dp))
                Text("Healoo", style = HType.screenTitle.copy(fontSize = HType.screenTitle.fontSize * 1.4f), color = Sage.OnPrimary)
            }
            Text("Your reports, doctors and messages in one place. You decide who sees what.",
                style = HType.body, color = Sage.OnPrimarySoft)
        }

        Column(
            Modifier.fillMaxWidth().weight(1f).verticalScrollWithBar(rememberScrollState()).navigationBarsPadding().imePadding().padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (demo != null) {
                Text("Demo mode — choose who to sign in as", style = HType.section, color = Sage.Ink)
                Text("Sample data only. Switch accounts from Settings → Log out.", style = HType.caption, color = Sage.Muted)
                demo.demoAccounts.forEach { u -> DemoAccountRow(u, enabled = !busy) {
                    busy = true
                    scope.launch { demo.signInAs(u.id); onSignedIn(); busy = false }
                } }
            } else {
                if (auth.isConfigured) {
                    PrimaryButton(if (busy) "Opening sign-in…" else "Sign in or create account", {
                        busy = true; error = null
                        scope.launch {
                            runCatching { auth.login(context); onSignedIn() }
                                .onFailure { error = "Sign-in didn't finish. Check your connection and try again." }
                            busy = false
                        }
                    }, Modifier.fillMaxWidth(), enabled = !busy)
                    Text("You'll sign in on a secure Healoo page, then come back here.", style = HType.caption, color = Sage.Muted)
                }
                if (auth.dev.isEnabled) {
                    if (auth.isConfigured) Spacer(Modifier.height(12.dp))
                    DevSignInSection(busy = busy, setBusy = { busy = it }, onSignedIn = onSignedIn)
                }
                if (!auth.isConfigured && !auth.dev.isEnabled) {
                    Text("Sign-in isn't set up in this build. Add healoo.auth0.domain and healoo.auth0.clientId to gradle.properties, " +
                        "set healoo.devSignIn=true for the trial server's test accounts, or set healoo.useFakeData=true for demo data.",
                        style = HType.body, color = Sage.Clay)
                }
            }
            error?.let { Text(it, style = HType.small, color = Sage.Clay) }
            if (BuildConfig.DEBUG) Text("API: ${BuildConfig.API_BASE_URL}", style = HType.small, color = Sage.Muted)
        }
    }
}

/** Debug builds only: sign in as one of the trial server's seeded accounts (AUTH_MODE=dev). */
@Composable
private fun DevSignInSection(busy: Boolean, setBusy: (Boolean) -> Unit, onSignedIn: suspend () -> Unit) {
    val auth = ServiceLocator.auth
    val scope = rememberCoroutineScope()
    var accounts by remember { mutableStateOf<List<DevAccount>?>(null) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var healooId by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var reload by remember { mutableIntStateOf(0) }

    LaunchedEffect(reload) {
        loadError = null
        runCatching { auth.dev.accounts() }
            .onSuccess { accounts = it }
            .onFailure { loadError = it.message ?: "Couldn't load test accounts." }
    }

    fun signIn(id: String) {
        setBusy(true); error = null
        scope.launch {
            runCatching { auth.devLogin(id); onSignedIn() }
                .onFailure { error = it.message ?: "Sign-in didn't finish." }
            setBusy(false)
        }
    }

    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.SandTint).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Developer sign-in", style = HType.section, color = Sage.Ink)
        Text("Trial server only. Signs in without Auth0 using the server's test accounts. Not available in release builds.",
            style = HType.caption, color = Sage.Muted)

        when {
            loadError != null -> {
                Text(loadError!!, style = HType.small, color = Sage.Clay)
                Text("Try again", style = HType.bodyStrong, color = Sage.Accent,
                    modifier = Modifier.clickable(enabled = !busy) { reload++ }.padding(vertical = 4.dp))
            }
            accounts == null -> Text("Loading test accounts…", style = HType.small, color = Sage.Muted)
            accounts!!.isEmpty() -> Text("The server has no test accounts yet. Run care-seed on the server.",
                style = HType.small, color = Sage.Muted)
            else -> accounts!!.forEach { a ->
                AccountRow(a.initials, a.displayName, "${a.roleLabel} · ${a.publicId}", enabled = !busy) { signIn(a.publicId) }
            }
        }

        SageTextField("Or enter a Healoo ID", healooId, { healooId = it.uppercase().take(8) }, placeholder = "HL-2M9P4")
        PrimaryButton(if (busy) "Signing in…" else "Sign in with this ID", { signIn(healooId) },
            Modifier.fillMaxWidth(), enabled = !busy && healooId.isNotBlank())
        error?.let { Text(it, style = HType.small, color = Sage.Clay) }
    }
}

@Composable
private fun AccountRow(initials: String, name: String, detail: String, enabled: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface)
            .clickable(enabled = enabled, onClick = onClick).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Avatar(initials, 44.dp)
        Column {
            Text(name, style = HType.bodyStrong, color = Sage.Ink)
            Text(detail, style = HType.small, color = Sage.Muted)
        }
    }
}

@Composable
private fun DemoAccountRow(u: UserProfile, enabled: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(Radius.card)).background(Sage.Surface)
            .clickable(enabled = enabled, onClick = onClick).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Avatar(u.initials, 44.dp, photoUrl = u.photoUri)
        Column {
            Text(u.displayName, style = HType.bodyStrong, color = Sage.Ink)
            Text("${u.headline} · ${u.publicId}", style = HType.small, color = Sage.Muted)
        }
    }
}
