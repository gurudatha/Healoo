import Foundation

/// Same rules as the server's recurrence module (Documentation/DataItem_Design.md 3.7), so the
/// booking sheet can show the number of visits and reject invalid combinations before sending.
enum RecurrenceRules {
    private static var cal: Calendar { var c = Calendar(identifier: .gregorian); c.timeZone = TimeZone(identifier: "Asia/Kolkata")!; return c }

    /// Monthly steps keep the day of month and clamp to the month's last day (Calendar does this).
    static func nthDate(_ start: Date, _ f: Frequency, _ n: Int) -> Date {
        switch f {
        case .daily: cal.date(byAdding: .day, value: n, to: start)!
        case .weekly: cal.date(byAdding: .day, value: 7 * n, to: start)!
        case .biweekly: cal.date(byAdding: .day, value: 14 * n, to: start)!
        case .monthly: cal.date(byAdding: .month, value: n, to: start)!
        case .quarterly: cal.date(byAdding: .month, value: 3 * n, to: start)!
        }
    }

    static func endDate(_ start: Date, _ r: Recurrence) -> Date? { r.period.map { cal.date(byAdding: .month, value: $0.months, to: start)! } }

    /// Number of visits, or nil when the series runs until cancelled.
    static func visitCount(_ start: Date, _ r: Recurrence) -> Int? {
        guard let end = endDate(start, r) else { return nil }
        var n = 0
        while nthDate(start, r.frequency, n) < end { n += 1 }
        return n
    }

    /// nil when valid, otherwise the message to show.
    static func problem(_ start: Date, _ r: Recurrence?) -> String? {
        guard let r else { return nil }
        if r.frequency == .daily && r.period == nil { return "Daily repeats need a period." }
        if let n = visitCount(start, r), n <= 1 { return "This repeat and period give only one visit. Turn repeat off instead." }
        return nil
    }

    /// Visits from `from` (inclusive), at most `max`, with exceptions and marks applied.
    static func visits(start: String, time: String, _ r: Recurrence?, exceptions: [VisitException],
                       marks: [String: String], cancelled: Bool, from: Date, max: Int = 12) -> [Visit] {
        guard let s = DateText.date(start) else { return [] }
        var originals: [Date] = []
        if let r {
            let end = endDate(s, r), limit = cal.date(byAdding: .year, value: 2, to: from)!
            var n = 0
            while true {
                let d = nthDate(s, r.frequency, n)
                if let end, d >= end { break }
                if d > limit { break }
                originals.append(d); n += 1
            }
        } else { originals = [s] }
        let fromText = DateText.string(from)
        return originals.map { o -> Visit in
            let orig = DateText.string(o)
            let ex = exceptions.first { $0.date == orig }
            let status = cancelled || ex?.action == "CANCELLED" ? "CANCELLED" : (marks[orig] ?? "SCHEDULED")
            return Visit(date: ex?.newDate ?? orig, time: ex?.newTime ?? time, originalDate: orig, status: status)
        }
        .filter { $0.date >= fromText }
        .sorted { $0.date + $0.time < $1.date + $1.time }
        .prefix(max).map { $0 }
    }
}
