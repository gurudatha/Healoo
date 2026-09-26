package com.healoo.app.data

import java.time.LocalDate
import java.time.LocalTime

/**
 * Same rules as the server's recurrence module (Documentation/DataItem_Design.md 3.7), so the
 * booking sheet can show the number of visits and reject invalid combinations before sending.
 */
object RecurrenceRules {

    fun nthDate(start: LocalDate, f: Frequency, n: Int): LocalDate = when (f) {
        Frequency.DAILY -> start.plusDays(n.toLong())
        Frequency.WEEKLY -> start.plusWeeks(n.toLong())
        Frequency.BIWEEKLY -> start.plusWeeks(2L * n)
        Frequency.MONTHLY -> start.plusMonths(n.toLong())          // keeps day-of-month, clamps to month end
        Frequency.QUARTERLY -> start.plusMonths(3L * n)
    }

    fun endDate(start: LocalDate, r: Recurrence): LocalDate? = r.period?.let { start.plusMonths(it.months.toLong()) }

    /** Number of visits, or null when the series runs until cancelled. */
    fun visitCount(start: LocalDate, r: Recurrence): Int? {
        val end = endDate(start, r) ?: return null
        var n = 0
        while (nthDate(start, r.frequency, n) < end) n++
        return n
    }

    /** Null when valid, otherwise the message to show. */
    fun problem(start: LocalDate, r: Recurrence?): String? {
        if (r == null) return null
        if (r.frequency == Frequency.DAILY && r.period == null) return "Daily repeats need a period."
        val n = visitCount(start, r)
        if (n != null && n <= 1) return "This repeat and period give only one visit. Turn repeat off instead."
        return null
    }

    /** Visits from [from] (inclusive), at most [max], with exceptions and marks applied. */
    fun visits(
        start: LocalDate, time: LocalTime, r: Recurrence?, exceptions: List<VisitException>,
        marks: Map<String, String>, cancelled: Boolean, from: LocalDate, max: Int = 12,
    ): List<Visit> {
        val originals = if (r == null) listOf(start) else {
            val end = endDate(start, r)
            generateSequence(0) { it + 1 }.map { nthDate(start, r.frequency, it) }
                .takeWhile { d -> (end == null || d < end) && d <= from.plusYears(2) }.toList()
        }
        return originals.map { orig ->
            val ex = exceptions.firstOrNull { it.date == orig.toString() }
            val date = ex?.newDate ?: orig.toString()
            val t = ex?.newTime ?: "%02d:%02d".format(time.hour, time.minute)
            val status = when {
                cancelled || ex?.action == "CANCELLED" -> "CANCELLED"
                else -> marks[orig.toString()] ?: "SCHEDULED"
            }
            Visit(date, t, orig.toString(), status)
        }.filter { LocalDate.parse(it.date) >= from }.sortedBy { it.date + it.time }.take(max)
    }
}
