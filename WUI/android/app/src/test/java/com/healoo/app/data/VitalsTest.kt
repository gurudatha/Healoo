package com.healoo.app.data

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Documentation/Vitals_Design.md section 8, tests C1–C4: the app keeps working when the server is
 * newer (extra fields, colours or icons it doesn't know) or older (fields missing).
 */
class VitalsTest {
    // The same settings RemoteRepository uses.
    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false; coerceInputValues = true }

    @Test fun c1_a_newer_servers_extra_fields_are_ignored() {
        val r = json.decodeFromString<VitalReading>(
            """{"reading_id":"r1","user_id":"u1","code":"bp","values":{"sys":150,"dia":95},"taken_at":"2026-10-02T08:15:00+05:30",
                "severity":"YELLOW","schema_version":7,"device":{"model":"cuff-2"},"trend":"rising"}""")
        assertEquals(Severity.YELLOW, r.severity)
        assertEquals(150.0, r.values.getValue("sys"), 0.0)
    }

    @Test fun c2_an_unknown_colour_reads_as_green_instead_of_failing() {
        val a = json.decodeFromString<VitalAlert>("""{"alert_id":"a1","patient_id":"u1","severity":"PURPLE"}""")
        assertEquals(Severity.GREEN, a.severity)
        assertEquals("other", a.icon)                 // and missing fields get defaults
    }

    @Test fun c3_an_older_servers_missing_fields_get_defaults() {
        val t = json.decodeFromString<VitalType>("""{"code":"peak_flow"}""")
        assertEquals(listOf("value"), t.components.map { it.key })
        assertEquals(null, t.marginPct)
        val page = json.decodeFromString<VitalAlertPage>("""{}""")
        assertEquals(0, page.counts[Severity.RED] + page.counts[Severity.YELLOW] + page.counts[Severity.GREEN])
    }

    @Test fun c5_time_can_be_set_only_within_the_last_12_hours() {
        val now = java.time.ZonedDateTime.of(2026, 10, 2, 9, 30, 0, 0, java.time.ZoneId.of("Asia/Kolkata"))
        val at = { h: Int, m: Int -> com.healoo.app.ui.vitals.earlierTime(h, m, 12, now) }
        assertEquals(now.withHour(7).withMinute(15), at(7, 15))                              // earlier today
        assertEquals(now.minusDays(1).withHour(22).withMinute(0), at(22, 0))                 // 22:00 hasn't come yet today: yesterday's
        assertEquals(now.minusDays(1).withHour(21).withMinute(30), at(21, 30))               // exactly 12 hours ago
        assertEquals(null, at(21, 29))                                                       // a minute more than 12 hours
        assertEquals(null, at(10, 0))                                                        // yesterday 10:00 is 23.5 hours ago
        assertEquals(now, at(9, 30))
    }

    @Test fun c6_urine_is_two_values_with_a_percentage() {
        val t = json.decodeFromString<VitalType>(
            """{"code":"urine","label":"Urine","unit":"ml","components":[{"key":"intake","label":"Water intake"},{"key":"output","label":"Urine output"}],
                "ratio":{"numerator":"output","denominator":"intake"}}""")
        assertEquals(87.0, t.ratio!!.percent(mapOf("intake" to 1500.0, "output" to 1305.0))!!, 0.001)
        assertEquals(null, t.ratio!!.percent(mapOf("intake" to 0.0, "output" to 300.0)))    // nothing to take a percentage of
        assertEquals(null, t.ratio!!.greenMinPct)                                            // thresholds aren't sent to the patient
        assertEquals("1500/1305 ml", formatVital(mapOf("output" to 1305.0, "intake" to 1500.0), "ml", t.components.map { it.key }))
        // An app from before ratio vitals existed simply has no "ratio" field: still decodes.
        assertEquals(null, json.decodeFromString<VitalType>("""{"code":"bp"}""").ratio)
    }

    @Test fun c4_values_are_shown_in_the_vitals_own_order() {
        assertEquals("120/80 mmHg", formatVital(mapOf("dia" to 80.0, "sys" to 120.0), "mmHg", listOf("sys", "dia")))
        assertEquals("120/80 mmHg", formatVital(mapOf("dia" to 80.0, "sys" to 120.0), "mmHg"))      // alert rows: no type at hand
        assertEquals("98.6 °F", formatVital(mapOf("value" to 98.6), "°F"))
        assertEquals("72", formatVital(mapOf("value" to 72.0), ""))
    }
}
