package com.healoo.app.data

import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/** Offline message cache (WUI/app-config.json: message_cache), tests O1–O6. */
class OfflineCacheTest {
    @get:Rule val tmp = TemporaryFolder()

    private val kb = 1024L
    private fun cache(maxKb: Long = 400, trimKb: Long = 20) = OfflineCache(tmp.root, maxKb * kb, trimKb * kb)
    private fun text(kbytes: Int) = "x".repeat(kbytes * 1024)
    private fun message(id: String, body: String, sentAt: String = "2026-10-02T08:00:00+05:30") = Message(id, "item-1", "u1", body, sentAt)
    private val messages = ListSerializer(Message.serializer())

    @Test fun o1_what_is_saved_can_be_read_back_without_a_server() {
        val c = cache()
        c.put(OfflineCache.messages("item-1"), messages, listOf(message("m1", "Hello"), message("m2", "Thank you, doctor.")))
        assertEquals(listOf("Hello", "Thank you, doctor."), c.get(OfflineCache.messages("item-1"), messages)!!.map { it.body })
        assertNull(c.get(OfflineCache.messages("never-opened"), messages))
    }

    @Test fun o2_stays_under_the_limit() {
        val c = cache(maxKb = 400, trimKb = 20)
        repeat(30) { i -> c.put("d$i", String.serializer(), text(30), date = 1_000L * (i + 1)) }      // 900 KB written in all
        assertTrue("${c.sizeBytes() / kb} KB", c.sizeBytes() <= 400 * kb)
    }

    @Test fun o3_when_full_the_oldest_by_date_go_first_and_about_trim_mb_is_freed() {
        val c = cache(maxKb = 400, trimKb = 20)
        // Thirteen 30 KB discussions = 390 KB (just under); dates 1..13, written in a shuffled order.
        listOf(7, 2, 13, 5, 1, 9, 3, 11, 4, 8, 6, 12, 10).forEach { d -> c.put("d$d", String.serializer(), text(30), date = 1_000L * d) }
        assertEquals(13, tmp.root.listFiles()!!.size)
        c.put("new", String.serializer(), text(30), date = 99_000L)                                    // 420 KB: over
        // Down to 380 KB (400 - 20): the two oldest dates are gone, not the two written first.
        val left = tmp.root.listFiles()!!.map { it.name.removeSuffix(".json") }.toSet()
        assertEquals((3..13).map { "d$it" }.toSet() + "new", left)
        assertTrue(c.sizeBytes() <= 380 * kb + 2 * kb)
    }

    @Test fun o4_the_discussion_just_saved_is_never_the_one_removed() {
        val c = cache(maxKb = 100, trimKb = 20)
        c.put("recent", String.serializer(), text(60), date = 50_000L)
        c.put("oldest-but-current", String.serializer(), text(60), date = 1L)                          // oldest date, but being written now
        assertEquals(listOf("oldest-but-current.json"), tmp.root.listFiles()!!.map(File::getName))
    }

    @Test fun o5_a_discussion_is_dated_by_its_newest_message() {
        val list = listOf(message("a", "old", "2026-09-01T10:00:00+05:30"), message("b", "new", "2026-10-02T08:15:00+05:30"), message("c", "bad", "not a date"))
        assertEquals(java.time.OffsetDateTime.parse("2026-10-02T08:15:00+05:30").toInstant().toEpochMilli(), OfflineCache.dateOf(list))
        val c = cache()
        c.put(OfflineCache.messages("item-1"), messages, list, OfflineCache.dateOf(list))
        assertEquals(OfflineCache.dateOf(list) / 1000, tmp.root.listFiles()!!.single().lastModified() / 1000)
    }

    @Test fun o6_limits_come_from_app_config_with_defaults() {
        val shared = listOf("../../app-config.json", "../app-config.json", "WUI/app-config.json").map(::File).first { it.isFile }
        val cfg = AppConfig.parse(shared.readText())
        assertEquals(40 to 2, cfg.messageCache.maxMb to cfg.messageCache.trimMb)
        assertEquals(15 to 90, cfg.fileCache.maxFiles to cfg.fileCache.maxMb)
        // An older or partial file: whatever is missing keeps its default; unknown settings are ignored.
        val partial = AppConfig.parse("""{"message_cache":{"max_mb":10},"future_setting":true}""")
        assertEquals(10 to 2, partial.messageCache.maxMb to partial.messageCache.trimMb)
        assertEquals(15, partial.fileCache.maxFiles)
        // Signing out removes everything.
        val c = cache().also { it.put("me", String.serializer(), "profile") }
        c.clear()
        assertEquals(0L, c.sizeBytes())
    }
}

/** Settings that used to be constants (Documentation/Configuration.md), tests K1–K4. */
class AppConfigTest {
    private val shared = listOf("../../app-config.json", "../app-config.json", "WUI/app-config.json").map(::File).first { it.isFile }.readText()

    @Test fun k1_the_shipped_file_holds_the_values_the_code_used_to_have() {
        val c = AppConfig.parse(shared)
        assertEquals(listOf(0.20f, 0.25f, 0.10f, 0.10f), c.layout.run { listOf(homeHeader, personHeaderExpanded, personHeaderCollapsed, pinnedHeader) })
        assertEquals(20 to 100, c.lists.homePreview to c.lists.homeAll)
        assertEquals(512, c.profilePhoto.maxPx)
        assertEquals(listOf("Cardiologist", "Urologist", "Paediatrician", "General Medicine"), c.designations)
        assertEquals(AppConfig(), c)                      // and the compiled-in defaults say the same
    }

    @Test fun k2_editing_the_file_changes_the_app() {
        val c = AppConfig.parse(shared.replace("\"home_header\": 0.20", "\"home_header\": 0.30").replace("\"home_preview\": 20", "\"home_preview\": 5")
            .replace("\"Urologist\", ", "\"Urologist\", \"Dermatologist\", "))
        assertEquals(0.30f, c.layout.homeHeader)
        assertEquals(5, c.lists.homePreview)
        assertTrue("Dermatologist" in c.designations)
    }

    @Test fun k3_values_that_would_break_the_screens_fall_back() {
        assertEquals(LayoutConfig(), AppConfig.parse("""{"layout":{"home_header":3.0}}""").layout)                       // taller than the screen
        assertEquals(LayoutConfig(), AppConfig.parse("""{"layout":{"person_header_expanded":0.1,"person_header_collapsed":0.3}}""").layout)
        assertEquals(1 to 1, AppConfig.parse("""{"lists":{"home_preview":0,"home_all":0}}""").lists.run { homePreview to homeAll })
        assertEquals(4096, AppConfig.parse("""{"profile_photo":{"max_px":999999}}""").profilePhoto.maxPx)
    }

    @Test fun k4_upload_limits_follow_the_server() {
        val json = kotlinx.serialization.json.Json { ignoreUnknownKeys = true }
        val before = Triple(Limits.MAX_ATTACHMENTS, Limits.IMAGE_MAX_BYTES, Limits.PDF_MAX_BYTES)
        assertEquals(Triple(20, 10L * 1024 * 1024, 25L * 1024 * 1024), before)                                           // defaults until the server answers
        // What GET /v1/config returns, with a field this app doesn't know.
        Limits.apply(json.decodeFromString<ServerConfig>(
            """{"network_mode":"lan","limits":{"attachments_per_item":8,"image_max_bytes":5242880,"pdf_max_bytes":52428800,"message_max_chars":4000}}""").limits)
        assertEquals(Triple(8, 5L * 1024 * 1024, 50L * 1024 * 1024), Triple(Limits.MAX_ATTACHMENTS, Limits.IMAGE_MAX_BYTES, Limits.PDF_MAX_BYTES))
        // An older server without limits, or nonsense values, changes nothing.
        Limits.apply(json.decodeFromString<ServerConfig>("""{"network_mode":"lan"}""").limits)
        Limits.apply(ServerLimits(attachmentsPerItem = 0, imageMaxBytes = -1))
        assertEquals(8, Limits.MAX_ATTACHMENTS)
        Limits.apply(ServerLimits(before.first, before.second, before.third))                                            // leave it as found
    }
}
