package com.healoo.app.push

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.google.firebase.FirebaseApp
import com.google.firebase.messaging.FirebaseMessaging
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import com.healoo.app.MainActivity
import com.healoo.app.R
import com.healoo.app.auth.SessionState
import com.healoo.app.data.ServiceLocator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await

/**
 * Push payload (data message, design doc 8):
 *   type     = "message" | "report" | "alert" | "booking"
 *   title, body
 *   item_id  = open this item (optional)
 *   user_id  = open the conversation with this user (optional)
 */
object PushRegistrar {
    private var lastToken: String? = null

    /** Firebase is only available when app/google-services.json was present at build time. */
    fun available(context: Context): Boolean =
        runCatching { FirebaseApp.getApps(context).isNotEmpty() || FirebaseApp.initializeApp(context) != null }.getOrDefault(false)

    suspend fun register(context: Context) {
        if (!available(context)) return
        runCatching {
            val token = FirebaseMessaging.getInstance().token.await()
            ServiceLocator.repository.registerDevice(token)
            lastToken = token
        }
    }

    /** On logout: stop pushes to this device for the old account. */
    suspend fun unregister(context: Context) {
        if (!available(context)) return
        runCatching {
            val token = lastToken ?: FirebaseMessaging.getInstance().token.await()
            ServiceLocator.repository.unregisterDevice(token)
            FirebaseMessaging.getInstance().deleteToken().await()
        }
        lastToken = null
    }
}

class HealooMessagingService : FirebaseMessagingService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onNewToken(token: String) {
        if (ServiceLocator.auth.session.value is SessionState.SignedIn) {
            scope.launch { runCatching { ServiceLocator.repository.registerDevice(token) } }
        }
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val data = message.data
        val title = data["title"] ?: message.notification?.title ?: "Healoo"
        val body = data["body"] ?: message.notification?.body ?: ""
        Notifications.show(this, data["type"] ?: "message", title, body, data["item_id"], data["user_id"])
    }
}

object Notifications {
    const val CHANNEL_MESSAGES = "messages"
    const val CHANNEL_RECORDS = "records"
    const val EXTRA_ITEM = "healoo.item_id"
    const val EXTRA_USER = "healoo.user_id"

    fun createChannels(context: Context) {
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL_MESSAGES, "Messages & alerts", NotificationManager.IMPORTANCE_HIGH))
        nm.createNotificationChannel(NotificationChannel(CHANNEL_RECORDS, "New reports & bookings", NotificationManager.IMPORTANCE_DEFAULT))
    }

    fun show(context: Context, type: String, title: String, body: String, itemId: String?, userId: String?) {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            itemId?.let { putExtra(EXTRA_ITEM, it) }
            userId?.let { putExtra(EXTRA_USER, it) }
        }
        val pending = PendingIntent.getActivity(context, (itemId ?: userId ?: title).hashCode(), intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val channel = if (type == "message" || type == "alert") CHANNEL_MESSAGES else CHANNEL_RECORDS
        val n = NotificationCompat.Builder(context, channel)
            .setSmallIcon(R.drawable.ic_stat_healoo)
            .setColor(0xFF2F6B5E.toInt())
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setAutoCancel(true)
            .setContentIntent(pending)
            .build()
        // Permission may be missing on Android 13+ if the user declined; then nothing is shown.
        runCatching { NotificationManagerCompat.from(context).notify((itemId ?: userId ?: title).hashCode(), n) }
    }
}

/** Where to go when the app is opened from a notification. */
sealed interface DeepLink {
    data class Item(val id: String) : DeepLink
    data class Conversation(val userId: String) : DeepLink
}

object DeepLinks {
    private val _links = MutableSharedFlow<DeepLink>(replay = 1, extraBufferCapacity = 4)
    val links: SharedFlow<DeepLink> = _links

    fun handle(intent: Intent?) {
        intent ?: return
        intent.getStringExtra(Notifications.EXTRA_ITEM)?.let { _links.tryEmit(DeepLink.Item(it)) }
            ?: intent.getStringExtra(Notifications.EXTRA_USER)?.let { _links.tryEmit(DeepLink.Conversation(it)) }
        intent.removeExtra(Notifications.EXTRA_ITEM)
        intent.removeExtra(Notifications.EXTRA_USER)
    }

    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    fun consumed() = _links.resetReplayCache()
}
