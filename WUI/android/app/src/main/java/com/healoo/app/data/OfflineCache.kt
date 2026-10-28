package com.healoo.app.data

import android.content.Context
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.File
import java.time.OffsetDateTime

// App settings from WUI/app-config.json (shared with iOS, bundled as an asset by the
// healooSharedAssets Gradle task), and the offline cache that uses them.

@Serializable
data class MessageCacheConfig(@SerialName("max_mb") val maxMb: Int = 40, @SerialName("trim_mb") val trimMb: Int = 2)

@Serializable
data class FileCacheConfig(@SerialName("max_files") val maxFiles: Int = 15, @SerialName("max_mb") val maxMb: Int = 90)

/** Share of the screen height each header takes (design doc 3.2). */
@Serializable
data class LayoutConfig(
    @SerialName("home_header") val homeHeader: Float = 0.20f,
    @SerialName("person_header_expanded") val personHeaderExpanded: Float = 0.25f,
    @SerialName("person_header_collapsed") val personHeaderCollapsed: Float = 0.10f,
    @SerialName("pinned_header") val pinnedHeader: Float = 0.10f,
) {
    /** Values that would break the screens (a header taller than the screen, expanded below collapsed) fall back to the defaults. */
    fun checked(): LayoutConfig {
        val ok = listOf(homeHeader, personHeaderExpanded, personHeaderCollapsed, pinnedHeader).all { it in 0.05f..0.6f } && personHeaderExpanded >= personHeaderCollapsed
        return if (ok) this else LayoutConfig()
    }
}

@Serializable
data class ListsConfig(@SerialName("home_preview") val homePreview: Int = 20, @SerialName("home_all") val homeAll: Int = 100)

@Serializable
data class ProfilePhotoConfig(@SerialName("max_px") val maxPx: Int = 512)

/** Every setting has a default, so a missing value or a missing file changes nothing. */
@Serializable
data class AppConfig(
    val layout: LayoutConfig = LayoutConfig(),
    val lists: ListsConfig = ListsConfig(),
    @SerialName("profile_photo") val profilePhoto: ProfilePhotoConfig = ProfilePhotoConfig(),
    /** Choices offered when an administrator adds a doctor; any other can be typed. */
    val designations: List<String> = listOf("Cardiologist", "Urologist", "Paediatrician", "General Medicine"),
    @SerialName("message_cache") val messageCache: MessageCacheConfig = MessageCacheConfig(),
    @SerialName("file_cache") val fileCache: FileCacheConfig = FileCacheConfig(),
) {
    companion object {
        private val json = Json { ignoreUnknownKeys = true; coerceInputValues = true }
        @Volatile var current = AppConfig(); private set

        fun parse(text: String): AppConfig = json.decodeFromString<AppConfig>(text).let { c ->
            c.copy(
                layout = c.layout.checked(),
                lists = ListsConfig(c.lists.homePreview.coerceIn(1, 100), c.lists.homeAll.coerceIn(c.lists.homePreview.coerceIn(1, 100), 100)),
                profilePhoto = ProfilePhotoConfig(c.profilePhoto.maxPx.coerceIn(64, 4096)),
            )
        }

        fun load(context: Context) {
            current = runCatching { parse(context.assets.open("app-config.json").bufferedReader().use { it.readText() }) }.getOrDefault(AppConfig())
        }
    }
}

/**
 * What the Messages screens need, kept on the phone so discussions can be read without the
 * server: the user's profile, the conversation list, and for every opened discussion its item and
 * messages. Each is one JSON file dated with its newest content.
 *
 * At most [maxBytes] in total (message_cache.max_mb). When a write goes over, the files with the
 * oldest dates are removed until about [trimBytes] (message_cache.trim_mb) is free again. That is
 * deliberately simple rather than efficient. The file just written is never removed.
 *
 * It is only a copy: the server stays the source of truth, and nothing here is sent anywhere.
 */
class OfflineCache(private val dir: File, private val maxBytes: Long, private val trimBytes: Long) {
    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false; coerceInputValues = true }

    private fun file(key: String) = File(dir, key.replace(Regex("[^A-Za-z0-9._-]"), "_") + ".json")

    /** Stores [value] under [key]; [date] (epoch millis) is what trimming goes by. */
    @Synchronized
    fun <T> put(key: String, serializer: KSerializer<T>, value: T, date: Long = System.currentTimeMillis()) {
        runCatching {
            dir.mkdirs()
            val f = file(key)
            f.writeText(json.encodeToString(serializer, value))
            f.setLastModified(date)
            trim(keep = f)
        }
    }

    /** The stored value, or null when there is none or it can no longer be read. */
    @Synchronized
    fun <T> get(key: String, serializer: KSerializer<T>): T? =
        runCatching { file(key).takeIf { it.isFile }?.let { json.decodeFromString(serializer, it.readText()) } }.getOrNull()

    @Synchronized
    fun clear() { dir.listFiles()?.forEach { it.delete() } }

    fun sizeBytes(): Long = dir.listFiles().orEmpty().sumOf { it.length() }

    /** Over the limit: delete the oldest-dated files until [trimBytes] is free (and it fits). */
    @Synchronized
    fun trim(keep: File? = null) {
        val files = dir.listFiles { f: File -> f.isFile }.orEmpty()
        var total = files.sumOf { it.length() }
        if (total <= maxBytes) return
        val target = (maxBytes - trimBytes).coerceAtLeast(0)
        for (f in files.filter { it != keep }.sortedBy { it.lastModified() }) {
            if (total <= target) break
            total -= f.length()
            f.delete()
        }
    }

    companion object {
        const val ME = "me"
        const val CONVERSATIONS = "conversations"
        fun item(id: String) = "item-$id"
        fun messages(itemId: String) = "messages-$itemId"

        fun create(context: Context, config: MessageCacheConfig = AppConfig.current.messageCache) =
            OfflineCache(File(context.filesDir, "offline"), config.maxMb * 1024L * 1024L, config.trimMb * 1024L * 1024L)

        /** The date of a discussion: when its newest message was sent (now if there are none). */
        fun dateOf(messages: List<Message>): Long =
            messages.maxOfOrNull { m -> runCatching { OffsetDateTime.parse(m.sentAt).toInstant().toEpochMilli() }.getOrDefault(0L) }
                ?.takeIf { it > 0 } ?: System.currentTimeMillis()
    }
}
