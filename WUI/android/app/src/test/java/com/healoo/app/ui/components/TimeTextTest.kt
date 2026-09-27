package com.healoo.app.ui.components

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.ZoneId
import java.time.ZonedDateTime

/** List rows show "6.37pm, Fri 4/16" this year and "6.37pm 4/16/2025" for earlier years, in local time. */
class TimeTextTest {
    private val india = ZoneId.of("Asia/Kolkata")
    private val now = ZonedDateTime.of(2026, 9, 27, 20, 0, 0, 0, india)

    @Test fun thisYearShowsWeekdayAndMonthDay() =
        assertEquals("6.37pm, Thu 4/16", TimeText.activity("2026-04-16T13:07:00+00:00", india, now))   // 13:07 UTC = 18:37 IST

    @Test fun earlierYearShowsFullDate() =
        assertEquals("6.37pm 4/16/2025", TimeText.activity("2025-04-16T13:07:00Z", india, now))

    @Test fun morningAndLocalConversion() =
        assertEquals("9.05am, Sun 9/27", TimeText.activity("2026-09-27T03:35:00.000+00:00", india, now))

    @Test fun unparsableIsShownAsIs() = assertEquals("soon", TimeText.activity("soon", india, now))
}
