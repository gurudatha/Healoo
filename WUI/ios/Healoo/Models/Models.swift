import Foundation

// DataItem v2 (Documentation/DataItem_Design.md): a container for one case with a fixed primary
// kind plus messages, attachments, appointments and alerts. JSON is snake_case; the decoder
// converts it. Swift's synthesized decoding needs every non-optional key, so fields the server
// may omit are optional here.

/// What an item started as (fixed at creation).
enum PrimaryKind: String, Codable, CaseIterable, Identifiable {
    case appointment = "APPOINTMENT", message = "MESSAGE", alert = "ALERT", report = "REPORT"
    var id: String { rawValue }
    var label: String { rawValue.capitalized }
}

/// Part kinds listed in DataItem.kinds.
enum PartKind {
    static let appointment = "APPOINTMENT", message = "MESSAGE", alert = "ALERT", report = "REPORT", attachment = "ATTACHMENT"
}

enum ItemStatus: String, Codable { case open = "OPEN", closed = "CLOSED" }
enum AttachmentKind: String, Codable { case image = "IMAGE", pdf = "PDF" }
enum Role: String, Codable, CaseIterable {
    case patient = "PATIENT", doctor = "DOCTOR", assistant = "ASSISTANT", lab = "LAB",
         hospital = "HOSPITAL", administrator = "ADMINISTRATOR", appAdministrator = "APP_ADMINISTRATOR"
}
enum GranteeType: String, Codable { case user = "USER", hospital = "HOSPITAL" }

enum Frequency: String, Codable, CaseIterable, Identifiable {
    case daily = "DAILY", weekly = "WEEKLY", biweekly = "BIWEEKLY", monthly = "MONTHLY", quarterly = "QUARTERLY"
    var id: String { rawValue }
    var label: String {
        switch self {
        case .daily: "Daily"; case .weekly: "Weekly"; case .biweekly: "Every 2 weeks"
        case .monthly: "Monthly"; case .quarterly: "Every 3 months"
        }
    }
}

enum Period: String, Codable, CaseIterable, Identifiable {
    case oneMonth = "ONE_MONTH", twoMonths = "TWO_MONTHS", threeMonths = "THREE_MONTHS", sixMonths = "SIX_MONTHS"
    var id: String { rawValue }
    var months: Int { switch self { case .oneMonth: 1; case .twoMonths: 2; case .threeMonths: 3; case .sixMonths: 6 } }
    var label: String { months == 1 ? "1 month" : "\(months) months" }
}

struct Recurrence: Codable, Hashable {
    var frequency: Frequency
    var period: Period?
    var label: String { frequency.label + (period.map { " for \($0.label)" } ?? " until cancelled") }
}

struct Attachment: Codable, Hashable, Identifiable {
    var attachmentId: String?
    var kind: AttachmentKind
    var uri: String
    var mime: String
    var size: Int64
    var position: Int
    var name: String
    var pageCount: Int?
    var thumbUri: String?
    var addedBy: String?
    var isReport: Bool?
    var id: String { attachmentId ?? uri }
    var report: Bool { isReport ?? false }
}

struct Grant: Codable, Hashable, Identifiable {
    var grantId: String
    var granteeType: GranteeType
    var granteeId: String
    var granteeName: String
    var viaHospitalId: String?
    var id: String { grantId }
}

struct UserProfile: Codable, Hashable, Identifiable {
    var userId: String
    var publicId: String
    var displayName: String
    var roles: [Role]
    var headline: String = ""
    var photoUri: String?
    var location: String?
    var hospital: String?
    var officialNumber: String?
    var connected: Bool = false

    var id: String { userId }
    var primaryRole: Role { roles.first ?? .patient }
    var initials: String {
        displayName.replacingOccurrences(of: "Dr. ", with: "")
            .split(separator: " ").prefix(2).compactMap(\.first).map { String($0).uppercased() }.joined()
    }
}

/// A message in an item's discussion (every message belongs to an item, design D5).
struct Message: Codable, Hashable, Identifiable {
    var messageId: String
    var itemId: String
    var senderId: String
    var body: String
    var sentAt: String                  // ISO timestamp with offset
    var attachmentIds: [String]?
    var id: String { messageId }
}

/// One occurrence of an appointment. status: SCHEDULED | COMPLETED | CANCELLED | NO_SHOW
struct Visit: Codable, Hashable, Identifiable {
    var date: String                    // YYYY-MM-DD (after any move)
    var time: String                    // HH:MM
    var originalDate: String            // the series date; used for visit actions
    var status: String
    var id: String { originalDate }
}

/// A per-visit change. action: CANCELLED | MOVED
struct VisitException: Codable, Hashable {
    var date: String
    var action: String
    var newDate: String?
    var newTime: String?
}

struct Appointment: Codable, Hashable, Identifiable {
    var appointmentId: String
    var itemId: String
    var patientId: String
    var doctorId: String
    var doctorName: String
    var hospitalId: String?
    var date: String                    // first visit
    var time: String
    var timezone: String
    var durationMin: Int
    var notes: String?
    var recurrence: Recurrence?
    var exceptions: [VisitException]
    var status: String                  // series: SCHEDULED | COMPLETED | CANCELLED
    var visitCount: Int?                // nil = until cancelled
    var visits: [Visit]                 // upcoming visits (server sends the next 12)

    var id: String { appointmentId }
    var nextVisit: Visit? { visits.first { $0.status == "SCHEDULED" } }
}

/// type: MEDICATION | APPOINTMENT_REMINDER | FOLLOW_UP | RESULT_READY | CUSTOM
struct Alert: Codable, Hashable, Identifiable {
    var alertId: String
    var itemId: String
    var type: String
    var text: String
    var forUser: String
    var firesAt: String                 // ISO timestamp with offset (first firing)
    var timezone: String
    var recurrence: Recurrence?
    var appointmentId: String?
    var active: Bool
    var id: String { alertId }
}

/// Present when the item is closed. Rating is only accepted from the owner (patient).
struct Closure: Codable, Hashable {
    var closedAt: String
    var closedBy: String
    var feedback: String?
    var rating: Int?
}

struct ItemCounts: Codable, Hashable {
    var messages = 0, attachments = 0, appointments = 0, alerts = 0, unreadMessages = 0
}

/// The container: a small header plus child lists (DataItem_Design.md 1 and 3.1).
struct DataItem: Codable, Hashable, Identifiable {
    var itemId: String
    var ownerId: String
    var primaryKind: PrimaryKind
    var kinds: [String]
    var title: String
    var keywords: [String]
    var status: ItemStatus
    var closure: Closure?
    var accessList: [Grant]             // owner only
    var counts: ItemCounts
    var messages: [Message]
    var attachments: [Attachment]
    var appointments: [Appointment]
    var alerts: [Alert]
    var links: [String]
    var createdByName: String
    var createdAt: String
    var updatedAt: String
    var allowedActions: [String]

    var id: String { itemId }
    /// read, meta, message, attach, book, alert, close, share, revoke, rate, reopen
    func can(_ action: String) -> Bool { allowedActions.contains(action) }
    var sortedAttachments: [Attachment] { attachments.sorted { $0.position < $1.position } }
    var nextVisit: Visit? { appointments.compactMap(\.nextVisit).min { $0.date + $0.time < $1.date + $1.time } }

    /// Date shown in lists: next visit for appointments, otherwise last activity.
    var date: String {
        if primaryKind == .appointment, let v = nextVisit { return v.date }
        let d = String(updatedAt.prefix(10))
        return d.isEmpty ? String(createdAt.prefix(10)) : d
    }

    /// One-line summary for lists, based on the primary part.
    var subtitle: String {
        var extra: [String] = []
        if primaryKind != .message && counts.messages > 0 { extra.append("\(counts.messages) msg") }
        if primaryKind != .appointment && counts.appointments > 0 { extra.append("appointment") }
        let main: String
        switch primaryKind {
        case .report:
            let files = counts.attachments > 0 ? "\(counts.attachments) file\(counts.attachments == 1 ? "" : "s")" : ""
            main = [createdByName.isEmpty ? "Report" : createdByName, files].filter { !$0.isEmpty }.joined(separator: " · ")
        case .appointment:
            if let a = appointments.first {
                main = a.doctorName + (nextVisit.map { " · \(DateText.short($0.date)) \($0.time)" } ?? (status == .closed ? " · closed" : ""))
            } else { main = "Appointment" }
        case .message: main = messages.last?.body ?? "Discussion"
        case .alert: main = alerts.first.map { "\($0.text) · \(DateText.clock($0.firesAt))" } ?? "Alert"
        }
        return ([main] + extra).filter { !$0.isEmpty }.joined(separator: " · ")
    }

    /// Empty header, used by the demo repository.
    static func new(id: String, owner: String, kind: PrimaryKind, title: String, keywords: [String] = [],
                    accessList: [Grant] = [], createdByName: String, now: String) -> DataItem {
        DataItem(itemId: id, ownerId: owner, primaryKind: kind, kinds: [kind.rawValue], title: title, keywords: keywords,
                 status: .open, closure: nil, accessList: accessList, counts: ItemCounts(), messages: [], attachments: [],
                 appointments: [], alerts: [], links: [], createdByName: createdByName, createdAt: now, updatedAt: now,
                 allowedActions: [])
    }
}

/// Messages tab row: an item with a discussion, with one other person.
struct Conversation: Codable, Hashable, Identifiable {
    var itemId: String
    var itemTitle: String
    var primaryKind: PrimaryKind
    /// OPEN or CLOSED (server v0.3+); discussions of closed items are shown grey.
    var status: ItemStatus? = nil
    var otherUser: UserProfile
    var lastMessage: String
    var lastMessageAt: String
    var unread: Int
    var id: String { "\(itemId)|\(otherUser.id)" }
}

/// One visit in the calendar (GET /v1/appointments).
struct CalendarVisit: Codable, Hashable, Identifiable {
    var startsAt: String
    var appointmentId: String
    var itemId: String
    var itemTitle: String
    var withUserId: String
    var withUserName: String
    var status: String
    var id: String { appointmentId + startsAt }
}

struct Dashboard: Codable, Hashable {
    var openReports: Int
    var unreadMessages: Int
    var upcomingAppointments: Int
}

struct PageResult<T: Codable>: Codable {
    var data: [T]
    var pageState: String?
}

// MARK: - Requests (encoded snake_case)

/// A file already uploaded through presign, to attach to an item.
struct NewAttachment: Codable, Hashable { var kind: AttachmentKind; var uri: String; var mime: String; var size: Int64; var name: String }
struct NewReport: Codable { var title: String; var attachments: [NewAttachment]; var links: [String] }
struct NewMessage: Codable { var body: String; var clientMsgId: String }

struct NewAppointment: Codable {
    var patientId: String
    var doctorId: String
    var date: String                    // YYYY-MM-DD
    var time: String                    // HH:MM
    var timezone = "Asia/Kolkata"
    var durationMin = 15
    var notes: String?
    var recurrence: Recurrence?
    var hospitalId: String?             // booked through a hospital's page
}

struct NewAlert: Codable {
    var type: String
    var text: String
    var forUser: String?
    var date: String
    var time: String
    var timezone = "Asia/Kolkata"
    var recurrence: Recurrence?
}

/// POST /v1/items: exactly one of appointment, message, alert or report.
struct NewItemRequest: Encodable {
    var ownerId: String?
    var title = ""
    var keywords: [String] = []
    var shareWith: [String] = []
    var appointment: NewAppointment?
    var message: NewMessage?
    var alert: NewAlert?
    var report: NewReport?
}

struct CloseRequest: Encodable { var feedback: String?; var rating: Int? }
struct AddAttachmentsRequest: Encodable { var attachments: [NewAttachment]; var isReport: Bool }
struct VisitActionRequest: Encodable { var action: String; var newDate: String?; var newTime: String? }

/// A file the user picked, copied into the app's temporary folder, before upload.
struct PhotoUpdate: Codable { var uri: String }

struct PendingAttachment: Identifiable, Hashable {
    let id = UUID()
    var localURL: URL
    var kind: AttachmentKind
    var name: String
    var mime: String
    var size: Int64
}

/// Upload screen: a new report.
struct ReportDraft {
    var ownerId: String
    var title: String
    var keywords: [String]
    var links: [String]
    var shareWith: [String]
}

enum Limits {
    static let maxAttachments = 20
    static let imageMaxBytes: Int64 = 10 * 1024 * 1024
    static let pdfMaxBytes: Int64 = 25 * 1024 * 1024
}

enum DateText {
    private static let iso: DateFormatter = { let f = DateFormatter(); f.dateFormat = "yyyy-MM-dd"; f.locale = Locale(identifier: "en_US_POSIX"); return f }()
    private static let shortFmt: DateFormatter = { let f = DateFormatter(); f.dateFormat = "d MMM"; return f }()
    private static let longFmt: DateFormatter = { let f = DateFormatter(); f.dateFormat = "d MMM yyyy"; return f }()

    static func short(_ s: String) -> String {
        guard let d = iso.date(from: String(s.prefix(10))) else { return s }
        return Calendar.current.isDateInToday(d) ? "Today" : shortFmt.string(from: d)
    }
    static func long(_ s: String) -> String { iso.date(from: String(s.prefix(10))).map(longFmt.string) ?? s }
    static func today() -> String { iso.string(from: Date()) }

    private static let instant: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]; return f
    }()
    private static let instantNoFraction = ISO8601DateFormatter()
    private static func fmt(_ pattern: String) -> DateFormatter {
        let f = DateFormatter(); f.dateFormat = pattern; f.locale = Locale(identifier: "en_US_POSIX"); return f
    }
    private static let clockFmt = fmt("h.mma"), thisYearFmt = fmt("EEE M/d"), otherYearFmt = fmt("M/d/yyyy")

    /// When an item last changed, in the phone's time zone, for list rows:
    /// this year "6.37pm, Fri 4/16"; an earlier year "6.37pm 4/16/2025" (same as Android's TimeText).
    static func activity(_ s: String, now: Date = Date()) -> String {
        guard let d = instant.date(from: s) ?? instantNoFraction.date(from: s) else { return s }
        let clock = clockFmt.string(from: d).lowercased()
        let sameYear = Calendar.current.component(.year, from: d) == Calendar.current.component(.year, from: now)
        return sameYear ? "\(clock), \(thisYearFmt.string(from: d))" : "\(clock) \(otherYearFmt.string(from: d))"
    }
    static func string(_ d: Date) -> String { iso.string(from: d) }
    static func date(_ s: String) -> Date? { iso.date(from: String(s.prefix(10))) }
    /// "HH:MM" from an ISO timestamp such as 2026-09-21T20:00:00+05:30.
    static func clock(_ s: String) -> String { s.count >= 16 ? String(s.dropFirst(11).prefix(5)) : s }
}

// MARK: - Profile, sharing overview, notification prefs, realtime

struct ProfileUpdate: Codable { var displayName: String; var location: String? }

struct NotificationPrefs: Codable, Equatable {
    var pushMessages = true
    var pushReports = true
    var quietHours = false
    var quietStart = "22:00"
    var quietEnd = "07:00"
}

/// One grant on an item the caller owns — GET /v1/grants?owner=me
struct OwnedGrant: Codable, Hashable, Identifiable {
    var grantId: String
    var granteeType: GranteeType
    var granteeId: String
    var granteeName: String
    var itemId: String
    var itemTitle: String
    var primaryKind: PrimaryKind
    var id: String { grantId }
}

struct ShareGroup: Identifiable, Hashable {
    var granteeId: String
    var granteeName: String
    var granteeType: GranteeType
    var grants: [OwnedGrant]
    var id: String { granteeId }
}

/// Pushed over the WebSocket (design doc 4.4).
enum RealtimeEvent {
    case newMessage(Message)
    case itemChanged(String)
    case connection(Bool)
}

extension UserProfile {
    /// Can create items for a patient in their contacts (the patient owns them).
    var isClinical: Bool { [.doctor, .assistant, .lab, .hospital].contains(primaryRole) }
    /// Hospital (or platform) administrator: manages users and doctors (Administration_Design.md).
    var isAdministrator: Bool { [.administrator, .appAdministrator].contains(primaryRole) }
    /// Can pass an item they don't own on to a doctor (referral).
    var canRefer: Bool { [.doctor, .lab, .hospital].contains(primaryRole) }
    func matches(_ q: String) -> Bool { displayName.lowercased().contains(q) || publicId.lowercased().contains(q) }
}

/// An account as an administrator sees it (GET/POST /v1/admin/...): profile plus email and status.
struct AdminAccount: Codable, Hashable, Identifiable {
    var userId: String
    var publicId: String
    var displayName: String
    var roles: [Role]
    var headline: String = ""
    var hospital: String?
    var officialNumber: String?
    var email: String?
    var active: Bool = true
    var id: String { userId }
    var profile: UserProfile {
        UserProfile(userId: userId, publicId: publicId, displayName: displayName, roles: roles, headline: headline,
                    hospital: hospital, officialNumber: officialNumber)
    }
}

/// POST /v1/admin/users and /v1/admin/doctors. Designation and number apply to doctors.
struct NewAccount: Codable {
    var displayName: String
    var email: String
    var location: String?
    var designation: String?
    var officialNumber: String?
}
