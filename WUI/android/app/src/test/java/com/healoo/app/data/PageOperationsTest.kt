package com.healoo.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.Test
import java.io.File

/**
 * Checks the shared WUI/page-operations.json against Documentation/PageOperations_Design.md
 * section 7 (tests B1–B7). Run: gradle :app:testDebugUnitTest
 */
class PageOperationsTest {
    companion object {
        @BeforeClass @JvmStatic
        fun loadSharedConfig() {
            // Unit tests run with the app module as the working directory.
            val file = listOf("../../page-operations.json", "../page-operations.json", "WUI/page-operations.json")
                .map(::File).first { it.isFile }
            PageOperations.parse(file.readText())
        }

        private fun user(role: Role, connected: Boolean = true) =
            UserProfile("id-$role", "HL-TEST", "Test $role", listOf(role), connected = connected)
        private val patient = user(Role.PATIENT)
        private val doctorViewer = user(Role.DOCTOR)
    }

    private fun ids(page: PageKind, viewer: UserProfile, subject: UserProfile?): Pair<List<String>, List<String>> =
        PageOperations.resolve(page, OpFact.of(viewer, subject)).let { r -> r.visible.map { it.id } to r.overflow.map { it.id } }

    @Test fun b1_global_page_shows_four_then_more() =
        assertEquals(listOf("home", "search", "upload", "messages") to listOf("settings"), ids(PageKind.GLOBAL, patient, null))

    @Test fun b2_user_page_is_for_that_user() =
        assertEquals(listOf("message", "history", "share_document", "upload_for") to emptyList<String>(),
            ids(PageKind.USER, doctorViewer, user(Role.PATIENT)))

    @Test fun b3_doctor_page_adds_booking_and_moves_the_fifth_to_more() =
        assertEquals(listOf("message", "history", "share_document", "book_appointment") to listOf("upload_for"),
            ids(PageKind.DOCTOR, patient, user(Role.DOCTOR)))

    @Test fun b4_hospital_page_searches_doctors_and_books() =
        assertEquals(listOf("search_doctors", "book_appointment") to emptyList<String>(),
            ids(PageKind.HOSPITAL, patient, user(Role.HOSPITAL)))

    @Test fun b5_not_connected_offers_add_contact_first() {
        assertEquals(listOf("connect") to emptyList<String>(), ids(PageKind.USER, patient, user(Role.PATIENT, connected = false)))
        assertEquals(listOf("connect", "book_appointment") to emptyList<String>(), ids(PageKind.DOCTOR, patient, user(Role.DOCTOR, connected = false)))
    }

    @Test fun b6_role_decides_the_page() {
        assertEquals(PageKind.DOCTOR, PageOperations.pageFor(user(Role.DOCTOR)))
        assertEquals(PageKind.HOSPITAL, PageOperations.pageFor(user(Role.HOSPITAL)))
        for (r in listOf(Role.PATIENT, Role.LAB, Role.ASSISTANT)) assertEquals(PageKind.USER, PageOperations.pageFor(user(r)))
    }

    @Test fun b7_never_more_than_max_visible() {
        for (page in PageKind.entries) for (subject in listOf(null, patient, user(Role.DOCTOR), user(Role.PATIENT, false))) {
            val r = PageOperations.resolve(page, OpFact.of(doctorViewer, subject))
            assertTrue("$page shows ${r.visible.size}", r.visible.size <= PageOperations.config.maxVisible)
            assertTrue("$page: More only when the bar is full", r.overflow.isEmpty() || r.visible.size == PageOperations.config.maxVisible)
        }
    }
}
