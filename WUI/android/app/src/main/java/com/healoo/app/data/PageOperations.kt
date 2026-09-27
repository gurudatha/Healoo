package com.healoo.app.data

import android.content.Context
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

// Bottom-bar operations per page, read from WUI/page-operations.json (shared with iOS and
// bundled as an asset by the healooSharedAssets Gradle task). Change operations there, not here.

/** Operation ids the app knows how to perform. Ids in the config that aren't listed are ignored. */
object OpId {
    const val HOME = "home"
    const val SEARCH = "search"
    const val UPLOAD = "upload"
    const val MESSAGES = "messages"
    const val SETTINGS = "settings"
    const val CONNECT = "connect"
    const val MESSAGE = "message"
    const val HISTORY = "history"
    const val SHARE_DOCUMENT = "share_document"
    const val UPLOAD_FOR = "upload_for"
    const val BOOK_APPOINTMENT = "book_appointment"
    const val SEARCH_DOCTORS = "search_doctors"

    val known = setOf(HOME, SEARCH, UPLOAD, MESSAGES, SETTINGS, CONNECT, MESSAGE, HISTORY, SHARE_DOCUMENT, UPLOAD_FOR, BOOK_APPOINTMENT, SEARCH_DOCTORS)
}

/** Which operation list a page uses. */
enum class PageKind(val key: String) { GLOBAL("global"), USER("user"), DOCTOR("doctor"), HOSPITAL("hospital") }

@Serializable
data class OperationSpec(val label: String, val icon: String, val requires: List<String> = emptyList())

@Serializable
data class OperationsConfig(
    val version: Int = 1,
    val maxVisible: Int = 4,
    val pageForRole: Map<String, String> = emptyMap(),
    val operations: Map<String, OperationSpec> = emptyMap(),
    val pages: Map<String, List<String>> = emptyMap(),
)

/** One resolved operation, ready to draw. */
data class PageOperation(val id: String, val label: String, val icon: String)

/** What `requires` conditions are checked against. */
object OpFact {
    const val CONNECTED = "connected"
    const val NOT_CONNECTED = "not_connected"
    const val VIEWER_PATIENT = "viewer_patient"
    const val VIEWER_CLINICAL = "viewer_clinical"

    fun of(viewer: UserProfile?, subject: UserProfile?): Set<String> = buildSet {
        if (subject != null) add(if (subject.connected) CONNECTED else NOT_CONNECTED)
        if (viewer != null) add(if (viewer.isClinical) VIEWER_CLINICAL else VIEWER_PATIENT)
    }
}

data class ResolvedOperations(val visible: List<PageOperation>, val overflow: List<PageOperation>)

object PageOperations {
    private val json = Json { ignoreUnknownKeys = true }
    var config = OperationsConfig(); private set

    fun load(context: Context) = parse(context.assets.open("page-operations.json").bufferedReader().use { it.readText() })

    /** Uses the config in [text] (the contents of page-operations.json). */
    fun parse(text: String) { config = json.decodeFromString(text) }

    fun pageFor(user: UserProfile): PageKind {
        val key = config.pageForRole[user.primaryRole.name] ?: config.pageForRole["*"] ?: PageKind.USER.key
        return PageKind.entries.firstOrNull { it.key == key } ?: PageKind.USER
    }

    /** The page's operations whose conditions hold, split at maxVisible for the three-dot menu. */
    fun resolve(page: PageKind, facts: Set<String>): ResolvedOperations {
        val ops = config.pages[page.key].orEmpty().mapNotNull { id ->
            val spec = config.operations[id] ?: return@mapNotNull null
            if (id !in OpId.known || !facts.containsAll(spec.requires)) null else PageOperation(id, spec.label, spec.icon)
        }
        val max = config.maxVisible.coerceAtLeast(1)
        return if (ops.size <= max) ResolvedOperations(ops, emptyList()) else ResolvedOperations(ops.take(max), ops.drop(max))
    }
}
