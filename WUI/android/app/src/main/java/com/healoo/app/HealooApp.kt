package com.healoo.app

import android.Manifest
import android.app.Application
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.core.content.ContextCompat
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.healoo.app.auth.LoginScreen
import com.healoo.app.auth.SessionState
import com.healoo.app.push.DeepLink
import com.healoo.app.push.DeepLinks
import com.healoo.app.push.Notifications
import com.healoo.app.push.PushRegistrar
import com.healoo.app.ui.profile.EditProfileScreen
import com.healoo.app.ui.sharing.ActiveSharingScreen
import com.healoo.app.ui.theme.Sage
import kotlinx.coroutines.launch
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.healoo.app.data.Role
import com.healoo.app.data.ServiceLocator
import com.healoo.app.ui.components.Tab
import com.healoo.app.ui.item.DataViewScreen
import com.healoo.app.ui.landing.LandingScreen
import com.healoo.app.ui.search.SearchScreen
import com.healoo.app.ui.settings.SettingsScreen
import com.healoo.app.ui.theme.HealooTheme
import com.healoo.app.ui.conversations.ConversationsScreen
import com.healoo.app.ui.discussion.DiscussionScreen
import com.healoo.app.ui.upload.NewItemMode
import com.healoo.app.ui.upload.UploadScreen
import com.healoo.app.ui.user.UserPageScreen
import com.healoo.app.ui.viewer.AttachmentViewer
import android.net.Uri as AUri

class HealooApplication : Application(), coil.ImageLoaderFactory {
    /**
     * Coil caches thumbnails only (full images and PDFs use AttachmentFiles, 90 MB / 15 files):
     * 10 MB on disk, so the two caches stay within about 100 MB together.
     */
    override fun newImageLoader(): coil.ImageLoader = coil.ImageLoader.Builder(this)
        .diskCache { coil.disk.DiskCache.Builder().directory(cacheDir.resolve("thumbnails")).maxSizeBytes(10L * 1024 * 1024).build() }
        .respectCacheHeaders(false)   // presigned URLs carry no useful cache headers
        .build()

    override fun onCreate() {
        super.onCreate()
        ServiceLocator.init(this)
        Notifications.createChannels(this)
        // Live channel only while the app is in the foreground; push covers the rest (doc 4.4 / 8).
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                if (ServiceLocator.auth.session.value is SessionState.SignedIn) ServiceLocator.repository.startRealtime()
            }
            override fun onStop(owner: LifecycleOwner) = ServiceLocator.repository.stopRealtime()
        })
    }
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT))
        super.onCreate(savedInstanceState)
        DeepLinks.handle(intent)
        setContent { HealooTheme { HealooRoot() } }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        DeepLinks.handle(intent)
    }
}

/** After Auth0, developer or demo sign-in: load the profile, open the live channel, register for push. */
private suspend fun completeSignIn(context: Context) {
    val repo = ServiceLocator.repository
    ServiceLocator.auth.signedIn(repo.me())
    repo.startRealtime()
    PushRegistrar.register(context)
}

private suspend fun signOut(context: Context) {
    PushRegistrar.unregister(context)
    ServiceLocator.repository.stopRealtime()
    val auth = ServiceLocator.auth
    if (!BuildConfig.USE_FAKE_DATA) auth.logout(context) else auth.signedOut()
}

@Composable
fun HealooRoot() {
    val context = LocalContext.current
    val auth = ServiceLocator.auth
    val session by auth.session.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        if (auth.session.value is SessionState.Checking) {
            val restored = !BuildConfig.USE_FAKE_DATA && auth.restore()
            if (restored) runCatching { completeSignIn(context) }.onFailure { auth.signedOut() } else auth.signedOut()
        }
    }

    when (val s = session) {
        SessionState.Checking -> Box(Modifier.fillMaxSize().background(Sage.Primary), contentAlignment = Alignment.Center) {
            CircularProgressIndicator(color = androidx.compose.ui.graphics.Color.White)
        }
        SessionState.SignedOut -> LoginScreen(onSignedIn = { completeSignIn(context) })
        is SessionState.SignedIn -> key(s.me.id) {        // new account -> fresh navigation and screens
            HealooNavHost(onSignOut = { scope.launch { signOut(context) } })
        }
    }
}

object Routes {
    const val HOME = "home"
    const val SEARCH = "search?q={q}&role={role}"
    const val UPLOAD = "upload?to={to}&item={item}&mode={mode}&doctor={doctor}&hospital={hospital}"
    const val MESSAGES = "messages"
    const val SETTINGS = "settings"
    const val USER = "user/{id}?messages={messages}"
    const val ITEM = "item/{id}"
    const val DISCUSSION = "discussion/{itemId}"
    const val VIEWER = "viewer/{id}/{index}"
    const val EDIT_PROFILE = "profile/edit"
    const val SHARING = "sharing"
    const val ADMIN = "admin"

    fun search(q: String = "", role: Role? = null) = "search?q=${AUri.encode(q)}&role=${role?.name ?: ""}"
    /** New item; [to] is the person whose page it came from (always a recipient). */
    fun upload(to: String? = null) = "upload?to=${to ?: ""}&item=&mode=&doctor=&hospital="
    /** Add files to an existing item (UploadScreen in add-files mode). */
    fun addFiles(itemId: String) = "upload?to=&item=$itemId&mode=&doctor=&hospital="
    /** New appointment with [doctorId], optionally through a hospital's page. */
    fun book(doctorId: String, hospitalId: String?) = "upload?to=${if (hospitalId == null) doctorId else ""}&item=&mode=APPOINTMENT&doctor=$doctorId&hospital=${hospitalId ?: ""}"
    fun discussion(itemId: String) = "discussion/$itemId"
    fun user(id: String, messages: Boolean = false) = "user/$id?messages=$messages"
    fun item(id: String) = "item/$id"
    fun viewer(id: String, index: Int) = "viewer/$id/$index"
}

private fun NavHostController.openTab(tab: Tab) {
    // Home always means the landing page. Restoring Home's saved stack would bring back
    // whatever was opened from it (e.g. Search), so pop straight back to it instead.
    if (tab == Tab.HOME) {
        if (!popBackStack(Routes.HOME, inclusive = false)) navigate(Routes.HOME) { launchSingleTop = true }
        return
    }
    val route = when (tab) {
        Tab.HOME -> Routes.HOME
        Tab.SEARCH -> Routes.search()
        Tab.UPLOAD -> Routes.upload()
        Tab.MESSAGES -> Routes.MESSAGES
        Tab.SETTINGS -> Routes.SETTINGS
    }
    navigate(route) {
        popUpTo(Routes.HOME) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}

@Composable
fun HealooNavHost(onSignOut: () -> Unit) {
    val nav = rememberNavController()
    val context = LocalContext.current

    // Android 13+: ask once for notification permission after sign-in.
    val askNotifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    // Opened from a notification -> go to the item or conversation.
    LaunchedEffect(Unit) {
        DeepLinks.links.collect { link ->
            when (link) {
                is DeepLink.Item -> nav.navigate(Routes.item(link.id))
                is DeepLink.Discussion -> nav.navigate(Routes.discussion(link.itemId))
                is DeepLink.Conversation -> nav.navigate(Routes.user(link.userId, messages = true))
            }
            DeepLinks.consumed()
        }
    }

    NavHost(nav, startDestination = Routes.HOME) {
        composable(Routes.HOME) {
            LandingScreen(
                onOpenItem = { nav.navigate(Routes.item(it)) },
                onSearch = { q, role -> nav.navigate(Routes.search(q, role)) },
                onOpenProfile = { nav.openTab(Tab.SETTINGS) },
                onTab = nav::openTab,
            )
        }
        composable(
            Routes.SEARCH,
            arguments = listOf(
                navArgument("q") { type = NavType.StringType; defaultValue = "" },
                navArgument("role") { type = NavType.StringType; defaultValue = "" },
            ),
        ) { entry ->
            val role = entry.arguments?.getString("role")?.takeIf { it.isNotBlank() }?.let { Role.valueOf(it) }
            SearchScreen(
                initialQuery = entry.arguments?.getString("q").orEmpty(),
                initialRole = role,
                onOpenUser = { nav.navigate(Routes.user(it)) },
                onTab = nav::openTab,
            )
        }
        composable(
            Routes.UPLOAD,
            arguments = listOf("to", "item", "mode", "doctor", "hospital").map { name ->
                navArgument(name) { type = NavType.StringType; defaultValue = "" }
            },
        ) { entry ->
            fun arg(name: String) = entry.arguments?.getString(name)?.takeIf { it.isNotBlank() }
            val addTo = arg("item")
            UploadScreen(
                toUserId = arg("to"),
                addToItemId = addTo,
                startMode = arg("mode")?.let { m -> NewItemMode.entries.firstOrNull { it.name == m } },
                doctorId = arg("doctor"),
                hospitalId = arg("hospital"),
                onClose = { if (!nav.popBackStack()) nav.openTab(Tab.HOME) },
                onUploaded = { id ->
                    // Adding files returns to the item already on the stack; a new item opens fresh.
                    if (addTo != null) nav.popBackStack() else nav.navigate(Routes.item(id)) { popUpTo(Routes.HOME) }
                },
            )
        }
        composable(Routes.MESSAGES) {
            ConversationsScreen(onOpenDiscussion = { nav.navigate(Routes.discussion(it)) }, onTab = nav::openTab)
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                onTab = nav::openTab,
                onEditProfile = { nav.navigate(Routes.EDIT_PROFILE) },
                onActiveSharing = { nav.navigate(Routes.SHARING) },
                onContacts = { nav.navigate(Routes.search()) },
                onLogout = onSignOut,
                onAdministration = { nav.navigate(Routes.ADMIN) },
            )
        }
        composable(Routes.EDIT_PROFILE) { EditProfileScreen(onClose = { nav.popBackStack() }) }
        composable(Routes.ADMIN) { com.healoo.app.ui.admin.AdminScreen(onBack = { nav.popBackStack() }) }
        composable(Routes.SHARING) {
            ActiveSharingScreen(onBack = { nav.popBackStack() }, onOpenItem = { nav.navigate(Routes.item(it)) })
        }
        composable(
            Routes.USER,
            arguments = listOf(
                navArgument("id") { type = NavType.StringType },
                navArgument("messages") { type = NavType.BoolType; defaultValue = false },
            ),
        ) { entry ->
            UserPageScreen(
                userId = entry.arguments!!.getString("id")!!,
                startOnMessages = entry.arguments!!.getBoolean("messages"),
                onBack = { nav.popBackStack() },
                onOpenItem = { nav.navigate(Routes.item(it)) },
                onOpenDiscussion = { nav.navigate(Routes.discussion(it)) },
                onOpenUser = { nav.navigate(Routes.user(it)) },
                onUploadFor = { nav.navigate(Routes.upload(it)) },
                onBook = { doctor, hospital -> nav.navigate(Routes.book(doctor, hospital)) },
            )
        }
        composable(Routes.ITEM, arguments = listOf(navArgument("id") { type = NavType.StringType })) { entry ->
            DataViewScreen(
                itemId = entry.arguments!!.getString("id")!!,
                onBack = { nav.popBackStack() },
                onOpenAttachment = { id, index -> nav.navigate(Routes.viewer(id, index)) },
                onOpenDiscussion = { nav.navigate(Routes.discussion(it)) },
                onAddFiles = { nav.navigate(Routes.addFiles(it)) },
            )
        }
        composable(Routes.DISCUSSION, arguments = listOf(navArgument("itemId") { type = NavType.StringType })) { entry ->
            val itemId = entry.arguments!!.getString("itemId")!!
            DiscussionScreen(
                itemId = itemId,
                onBack = { nav.popBackStack() },
                onOpenItem = { nav.navigate(Routes.item(it)) { launchSingleTop = true } },
            )
        }
        composable(
            Routes.VIEWER,
            arguments = listOf(navArgument("id") { type = NavType.StringType }, navArgument("index") { type = NavType.IntType }),
        ) { entry ->
            AttachmentViewer(
                itemId = entry.arguments!!.getString("id")!!,
                startIndex = entry.arguments!!.getInt("index"),
                onClose = { nav.popBackStack() },
            )
        }
    }
}
