package com.healoo.app.ui.components

import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * When an item last changed, in the phone's time zone, for list rows:
 * this year "6.37pm, Fri 4/16"; an earlier year "6.37pm 4/16/2025".
 */
object TimeText {
    private val time = DateTimeFormatter.ofPattern("h.mma", Locale.ENGLISH)
    private val thisYear = DateTimeFormatter.ofPattern("EEE M/d", Locale.ENGLISH)
    private val otherYear = DateTimeFormatter.ofPattern("M/d/yyyy", Locale.ENGLISH)

    /** [iso] is a server timestamp such as 2026-09-27T13:57:13.017+00:00; unparsable input is shown as is. */
    fun activity(iso: String, zone: ZoneId = ZoneId.systemDefault(), now: ZonedDateTime = ZonedDateTime.now(zone)): String {
        val t = runCatching { OffsetDateTime.parse(iso).atZoneSameInstant(zone) }.getOrNull() ?: return iso
        val clock = t.format(time).lowercase(Locale.ENGLISH)          // 6.37pm
        return if (t.year == now.year) "$clock, ${t.format(thisYear)}" else "$clock ${t.format(otherYear)}"
    }
}
