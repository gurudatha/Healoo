import Foundation

// Mirrors design doc 2.2 (DataItem) and 4.3 (REST API). JSON is snake_case; the decoder converts it.

enum CoreItemType: String, Codable, CaseIterable, Identifiable {
    case report = "REPORT", alert = "ALERT", booking = "BOOKING", feedback = "FEEDBACK", payment = "PAYMENT", message = "MESSAGE"
    var id: String { rawValue }
    var label: String { rawValue.capitalized }
}

enum ItemStatus: String, Codable { case open = "OPEN", closed = "CLOSED" }
enum AttachmentKind: String, Codable { case image = "IMAGE", pdf = "PDF" }
enum Role: String, Codable, CaseIterable {
    case patient = "PATIENT", doctor = "DOCTOR", assistant = "ASSISTANT", lab = "LAB",
         hospital = "HOSPITAL", administrator = "ADMINISTRATOR", appAdministrator = "APP_ADMINISTRATOR"
}
enum GranteeType: String, Codable { case user = "USER", hospital = "HOSPITAL" }

struct Attachment: Codable, Hashable, Identifiable {
    var kind: AttachmentKind
    var uri: String
    var mime: String
    var size: Int64
    var position: Int
    var name: String
    var pageCount: Int?
    var thumbUri: String?
    var id: String { uri }
}

struct Grant: Codable, Hashable, Identifiable {
    var grantId: String
    var granteeType: GranteeType
    var granteeId: String
    var granteeName: String
    var viaHospitalId: String?
    var id: String { grantId }
}

struct DataItem: Codable, Hashable, Identifiable {
    var itemId: String
    var date: String
    var coreItemType: CoreItemType
    var title: String
    var subtitle: String = ""
    var keywords: [String] = []
    var coreItemData: [Attachment] = []
    var links: [String] = []
    var ownerId: String
    var createdByName: String = ""
    var accessList: [Grant] = []
    var pointerToMessage: String?
    var pointerItemId: String?
    var rating: Int?
    var status: ItemStatus = .open
    var allowedActions: [String] = []

    var id: String { itemId }
    var type: CoreItemType { coreItemType }
    var attachments: [Attachment] { coreItemData.sorted { $0.position < $1.position } }
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

struct Message: Codable, Hashable, Identifiable {
    var messageId: String
    var threadId: String
    var senderId: String
    var body: String
    var sentAt: String
    var linkedItemId: String?
    var id: String { messageId }
}

struct ThreadSummary: Codable, Hashable, Identifiable {
    var threadId: String
    var otherUser: UserProfile
    var lastMessage: String
    var lastMessageAt: String
    var unread: Int = 0
    var id: String { threadId }
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

/// A file the user picked, copied into the app's temporary folder, before upload.
struct PendingAttachment: Identifiable, Hashable {
    let id = UUID()
    var localURL: URL
    var kind: AttachmentKind
    var name: String
    var mime: String
    var size: Int64
}

struct UploadDraft {
    var ownerId: String
    var type: CoreItemType
    var title: String
    var date: String
    var keywords: [String]
    var links: [String]
    var status: ItemStatus
    var shareWith: [String]
    var pointerToMessage: String?
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
        guard let d = iso.date(from: s) else { return s }
        return Calendar.current.isDateInToday(d) ? "Today" : shortFmt.string(from: d)
    }
    static func long(_ s: String) -> String { iso.date(from: s).map(longFmt.string) ?? s }
    static func today() -> String { iso.string(from: Date()) }
}

// MARK: - Added in 0.2: profile, sharing overview, notification prefs, realtime

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
    var coreItemType: CoreItemType
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
    var isClinical: Bool { [.doctor, .assistant, .lab].contains(primaryRole) }
}
