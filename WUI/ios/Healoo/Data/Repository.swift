import Foundation
import Combine
import CryptoKit

/// Everything the WUI needs. Mock and remote implementations are interchangeable.
protocol HealooRepository: AnyObject {
    func me() async throws -> UserProfile
    func dashboard() async throws -> Dashboard
    /// Visible items; `kind` (a PartKind) keeps only items containing that part, as GET /v1/items?kind= does.
    func items(status: ItemStatus, limit: Int, kind: String?) async throws -> [DataItem]
    /// Full item with its messages, attachments, appointments and alerts.
    func item(_ id: String) async throws -> DataItem
    func user(_ id: String) async throws -> UserProfile
    /// Items shared between the caller and `userId`. Empty unless connected (doc 2.3 rule 8).
    func sharedItems(with userId: String) async throws -> [DataItem]
    /// Global search by Healoo ID or name. Profiles only, never data.
    func search(_ query: String, role: Role?) async throws -> [UserProfile]
    /// Doctors currently working at a hospital (profiles only).
    func hospitalDoctors(_ hospitalId: String) async throws -> [UserProfile]
    func connections() async throws -> [UserProfile]
    func connect(_ userId: String) async throws -> UserProfile
    func share(_ itemId: String, with granteeId: String) async throws -> DataItem
    func revoke(_ itemId: String, grantId: String) async throws -> DataItem

    // DataItem v2: creating an item from one primary part
    func createReport(_ draft: ReportDraft, files: [PendingAttachment]) async throws -> DataItem
    func createAppointment(ownerId: String?, _ appointment: NewAppointment, shareWith: [String]) async throws -> DataItem
    func createAlert(title: String, _ alert: NewAlert, shareWith: [String]) async throws -> DataItem
    /// New MESSAGE item: starts a discussion with `userId` (the patient in the pair owns it).
    /// Also shared with `alsoWith`; a clinician writing to a patient can add doctors only.
    func startConversation(with userId: String, body: String, alsoWith: [String]) async throws -> DataItem

    // DataItem v2: adding to an existing item
    func itemMessages(_ itemId: String) async throws -> [Message]
    func sendItemMessage(_ itemId: String, body: String) async throws -> Message
    func addAttachments(_ itemId: String, files: [PendingAttachment], isReport: Bool) async throws -> DataItem
    func bookAppointment(_ itemId: String, _ appointment: NewAppointment) async throws -> DataItem
    /// One visit: CANCELLED, MOVED (newDate/newTime), COMPLETED or NO_SHOW.
    func visitAction(_ itemId: String, appointmentId: String, visitDate: String, action: String, newDate: String?, newTime: String?) async throws -> DataItem
    func cancelAppointment(_ itemId: String, appointmentId: String) async throws -> DataItem
    func addAlert(_ itemId: String, _ alert: NewAlert) async throws -> DataItem
    func deleteAlert(_ itemId: String, alertId: String) async throws -> DataItem
    /// Close with optional feedback; rating (1–5) only when the patient closes.
    func closeItem(_ itemId: String, feedback: String?, rating: Int?) async throws -> DataItem
    func reopenItem(_ itemId: String) async throws -> DataItem

    /// Messages tab: items with a discussion, per person (optionally only with `withUser`).
    func conversations(with withUser: String?) async throws -> [Conversation]
    /// Upcoming visits across items (YYYY-MM-DD, up to 62 days).
    func calendar(from: String, to: String) async throws -> [CalendarVisit]

    // Administration (hospital administrators; users can't be deleted)
    /// The administrator's hospital's doctors, deactivated ones included.
    func adminDoctors() async throws -> [AdminAccount]
    func adminCreateUser(_ account: NewAccount) async throws -> AdminAccount
    func adminCreateDoctor(_ account: NewAccount) async throws -> AdminAccount
    /// "Delete" a doctor: deactivates the account and ends the hospital affiliation.
    func adminDeactivateDoctor(_ doctorId: String) async throws
    func adminReactivateDoctor(_ doctorId: String) async throws -> AdminAccount

    // Account
    func updateProfile(_ update: ProfileUpdate) async throws -> UserProfile
    /// Profile picture: a JPEG prepared by `ProfilePhotos.prepare` (PUT /v1/me/photo). Own profile only.
    func setProfilePhoto(_ photo: PendingAttachment) async throws -> UserProfile
    /// Back to initials (DELETE /v1/me/photo).
    func removeProfilePhoto() async throws -> UserProfile
    func activeShares() async throws -> [ShareGroup]
    func notificationPrefs() async throws -> NotificationPrefs
    func saveNotificationPrefs(_ prefs: NotificationPrefs) async throws -> NotificationPrefs
    /// FCM registration token for this device (doc 8).
    func registerDevice(token: String) async throws
    func unregisterDevice(token: String) async throws

    /// Live events from the WebSocket (doc 4.4). Observe with `.onReceive`.
    var events: AnyPublisher<RealtimeEvent, Never> { get }
    func startRealtime()
    func stopRealtime()
}

extension HealooRepository {
    func items(status: ItemStatus, limit: Int) async throws -> [DataItem] { try await items(status: status, limit: limit, kind: nil) }
    func openItems(limit: Int = 20, kind: String? = nil) async throws -> [DataItem] { try await items(status: .open, limit: limit, kind: kind) }
    func conversations() async throws -> [Conversation] { try await conversations(with: nil) }
}

struct RepoError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

@Observable
final class AppEnvironment {
    let repo: HealooRepository
    let auth: AuthManager
    let useFakeData: Bool

    init() {
        let info = Bundle.main.infoDictionary ?? [:]
        useFakeData = (info["HealooUseFakeData"] as? Bool) ?? true
        let base = URL(string: (info["HealooAPIBaseURL"] as? String) ?? "https://careconnect.localhost/")!
        let auth = AuthManager(info: info, baseURL: base, useFakeData: useFakeData)
        self.auth = auth
        repo = useFakeData ? MockRepository() : RemoteRepository(baseURL: base, auth: auth)
    }

    /// After Auth0, developer or demo sign-in: load the profile, open the live channel, register for push.
    @MainActor func completeSignIn() async throws {
        auth.session = .signedIn(try await repo.me())
        repo.startRealtime()
        PushManager.shared.registerIfPossible(repo: repo)
    }

    @MainActor func signOut() async {
        Drafts.clear()   // another account must not see this one's unsent text
        await PushManager.shared.unregister(repo: repo)
        repo.stopRealtime()
        if !useFakeData { await auth.logout() } else { auth.signedOut() }
    }
}

/// Current Auth0 access token (design doc 5.4); kept fresh by AuthManager.
enum TokenStore { nonisolated(unsafe) static var accessToken: String? }

// MARK: - Mock (multi-user demo backend)

/// Seeded demo backend for DataItem v2, evaluated from the signed-in user's point of view so the
/// patient, doctor and lab flows can all be tried. Access follows design doc 2.3 applied to the
/// whole item (DataItem_Design.md 3.8). Mirrors the Android FakeRepository.
final class MockRepository: HealooRepository {
    private func sample(_ name: String) -> String { "bundle://\(name)" }

    private var users: [UserProfile] = [
        UserProfile(userId: "u-lakshmi", publicId: "HL-2M9P4", displayName: "Lakshmi K.", roles: [.patient], headline: "Patient", location: "[City]"),
        UserProfile(userId: "u-rao", publicId: "HL-7R2C9", displayName: "Dr. Anitha Rao", roles: [.doctor], headline: "Doctor · General Medicine",
                    location: "[City]", hospital: "Test Hospital A", officialNumber: "[REG-NO]"),
        UserProfile(userId: "h-a", publicId: "HL-1H0A1", displayName: "Test Hospital A", roles: [.hospital], headline: "Hospital"),
        UserProfile(userId: "l-city", publicId: "HL-6L3D2", displayName: "City Diagnostics", roles: [.lab], headline: "Lab"),
        UserProfile(userId: "u-srao", publicId: "HL-3K8M1", displayName: "Dr. Srinivas Rao", roles: [.doctor], headline: "Doctor · Independent"),
        UserProfile(userId: "h-raoheart", publicId: "HL-9P4T6", displayName: "Rao Heart Clinic", roles: [.hospital], headline: "Hospital"),
        UserProfile(userId: "l-rao", publicId: "HL-5W1Q3", displayName: "Rao Diagnostics", roles: [.lab], headline: "Lab"),
        UserProfile(userId: "u-priya", publicId: "HL-4K7Q2", displayName: "Priya Rao", roles: [.patient], headline: "Patient"),
        UserProfile(userId: "u-menon", publicId: "HL-2D5M8", displayName: "Dr. Kavya Menon", roles: [.doctor], headline: "Doctor · Cardiology",
                    hospital: "Test Hospital A"),
        UserProfile(userId: "a-admin", publicId: "HL-8A2D4", displayName: "Hospital A Admin", roles: [.administrator], headline: "Hospital administrator",
                    hospital: "Test Hospital A"),
    ]

    var demoAccounts: [UserProfile] { users.filter { ["u-lakshmi", "u-rao", "l-city", "h-a", "a-admin"].contains($0.id) } }
    private var meId = "u-lakshmi"
    func signIn(as userId: String) { meId = userId }

    private var affiliation = ["u-rao": "h-a", "u-menon": "h-a"]   // doctor -> current hospital
    private let adminHospital = ["a-admin": "h-a"]                  // administrator -> hospital
    private var homeHospital = ["u-rao": "h-a", "u-menon": "h-a"]
    private var deactivated = Set<String>()
    private var emails: [String: String] = [:]                      // user -> email (admin-created)
    private var links: Set<Set<String>> = [
        ["u-lakshmi", "u-rao"], ["u-lakshmi", "h-a"], ["u-lakshmi", "l-city"], ["u-lakshmi", "u-srao"],
        ["u-rao", "h-a"], ["u-rao", "l-city"], ["u-priya", "l-city"],
    ]

    private func raw(_ id: String) -> UserProfile { users.first { $0.id == id }! }
    private func isConnected(_ a: String, _ b: String) -> Bool { links.contains([a, b]) }
    private func view(_ u: UserProfile) -> UserProfile { var v = u; v.connected = isConnected(meId, u.id); return v }
    private func grant(for u: UserProfile) -> Grant {
        Grant(grantId: UUID().uuidString, granteeType: u.primaryRole == .hospital ? .hospital : .user, granteeId: u.id, granteeName: u.displayName)
    }
    private func newId() -> String { UUID().uuidString }
    private static let isoTime: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter(); f.timeZone = TimeZone(identifier: "Asia/Kolkata"); f.formatOptions = [.withInternetDateTime]; return f
    }()
    private func iso(_ d: Date = Date()) -> String { Self.isoTime.string(from: d) }
    private func day(_ offset: Int) -> String { DateText.string(Calendar.current.date(byAdding: .day, value: offset, to: Date())!) }

    // Stored items keep their children; derived fields are computed per viewer in `present`.
    private final class Stored {
        var item: DataItem
        let createdBy: String
        var marks: [String: [String: String]] = [:]         // appointment -> date -> COMPLETED/NO_SHOW
        init(_ item: DataItem, createdBy: String) { self.item = item; self.createdBy = createdBy }
    }
    private var store: [Stored] = []
    private var unread: [String: Int] = [:]                  // "user|item" -> count
    private var prefs: [String: NotificationPrefs] = [:]

    private func msg(_ itemId: String, _ from: String, _ body: String, minutesAgo: Double) -> Message {
        Message(messageId: newId(), itemId: itemId, senderId: from, body: body, sentAt: iso(Date().addingTimeInterval(-minutesAgo * 60)), attachmentIds: [])
    }
    private func appt(_ itemId: String, doctor: String, patient: String, date: String, time: String, _ rec: Recurrence?, notes: String?,
                      hospital: String? = nil, timezone: String = "Asia/Kolkata", duration: Int = 15) -> Appointment {
        Appointment(appointmentId: newId(), itemId: itemId, patientId: patient, doctorId: doctor, doctorName: raw(doctor).displayName,
                    hospitalId: hospital, date: date, time: time, timezone: timezone, durationMin: duration, notes: notes,
                    recurrence: rec, exceptions: [], status: "SCHEDULED", visitCount: nil, visits: [])
    }
    private func alert(_ itemId: String, forUser: String, _ type: String, _ text: String, date: String, time: String, _ rec: Recurrence?) -> Alert {
        Alert(alertId: newId(), itemId: itemId, type: type, text: text, forUser: forUser, firesAt: "\(date)T\(time):00+05:30",
              timezone: "Asia/Kolkata", recurrence: rec, appointmentId: nil, active: true)
    }
    private func att(_ id: String, _ kind: AttachmentKind, _ file: String, _ mime: String, _ size: Int64, _ pos: Int, _ name: String, pages: Int? = nil) -> Attachment {
        Attachment(attachmentId: id, kind: kind, uri: sample(file), mime: mime, size: size, position: pos, name: name, pageCount: pages, isReport: true)
    }

    init() {
        let rao = Grant(grantId: "g-rao", granteeType: .user, granteeId: "u-rao", granteeName: "Dr. Anitha Rao")
        let hospA = Grant(grantId: "g-ha", granteeType: .hospital, granteeId: "h-a", granteeName: "Test Hospital A")
        let lab = Grant(grantId: "g-lab", granteeType: .user, granteeId: "l-city", granteeName: "City Diagnostics")
        let now = iso()

        // 1. Lab report that grew into a full case.
        var cbc = DataItem.new(id: "i-cbc", owner: "u-lakshmi", kind: .report, title: "CBC – Complete blood count",
                               keywords: ["CBC", "Haemoglobin", "Routine check"], accessList: [hospA, rao, lab], createdByName: "City Diagnostics", now: now)
        cbc.attachments = [
            att("a1", .image, "cbc_scan_1.jpg", "image/jpeg", 91_000, 0, "cbc_scan_1.jpg"),
            att("a2", .image, "cbc_scan_2.jpg", "image/jpeg", 92_000, 1, "cbc_scan_2.jpg"),
            att("a3", .pdf, "cbc_report.pdf", "application/pdf", 3_400, 2, "CBC_report.pdf", pages: 3),
            att("a4", .pdf, "reference_ranges.pdf", "application/pdf", 2_500, 3, "Reference_ranges.pdf", pages: 2),
        ]
        cbc.messages = [msg("i-cbc", "u-rao", "Haemoglobin is slightly low. Let's review every two weeks for a while.", minutesAgo: 180),
                        msg("i-cbc", "u-lakshmi", "Thank you, doctor. I'll book the visits.", minutesAgo: 170)]
        cbc.appointments = [appt("i-cbc", doctor: "u-rao", patient: "u-lakshmi", date: day(7), time: "11:30",
                                 Recurrence(frequency: .biweekly, period: .threeMonths), notes: "Haemoglobin review", hospital: "h-a")]
        cbc.alerts = [alert("i-cbc", forUser: "u-lakshmi", "MEDICATION", "Iron tablet after dinner", date: day(0), time: "20:30",
                            Recurrence(frequency: .daily, period: .oneMonth))]
        store.append(Stored(cbc, createdBy: "l-city"))
        unread["u-lakshmi|i-cbc"] = 1

        // 2. Standalone discussion.
        var bp = DataItem.new(id: "i-bp", owner: "u-lakshmi", kind: .message, title: "Latest BP readings", keywords: ["BP"],
                              accessList: [rao], createdByName: "Dr. Anitha Rao", now: now)
        bp.messages = [msg("i-bp", "u-rao", "Please share your latest BP readings before Thursday.", minutesAgo: 60),
                       msg("i-bp", "u-lakshmi", "Sure, I will upload them tonight.", minutesAgo: 50)]
        store.append(Stored(bp, createdBy: "u-rao"))

        // 3. Standalone appointment.
        var follow = DataItem.new(id: "i-followup", owner: "u-lakshmi", kind: .appointment, title: "Appointment with Dr. Anitha Rao",
                                  keywords: ["Follow-up"], accessList: [rao, hospA], createdByName: "Lakshmi K.", now: now)
        follow.appointments = [appt("i-followup", doctor: "u-rao", patient: "u-lakshmi", date: day(3), time: "10:00", nil, notes: "General follow-up", hospital: "h-a")]
        store.append(Stored(follow, createdBy: "u-lakshmi"))

        // 4. Standalone alert.
        var check = DataItem.new(id: "i-bpcheck", owner: "u-lakshmi", kind: .alert, title: "Check blood pressure", keywords: ["BP"],
                                 accessList: [rao], createdByName: "Lakshmi K.", now: now)
        check.alerts = [alert("i-bpcheck", forUser: "u-lakshmi", "CUSTOM", "Check blood pressure", date: day(0), time: "08:00",
                              Recurrence(frequency: .weekly, period: .threeMonths))]
        store.append(Stored(check, createdBy: "u-lakshmi"))

        // 5. Closed item with feedback and rating.
        var closed = DataItem.new(id: "i-closed", owner: "u-lakshmi", kind: .message, title: "Consultation on 10 Sep",
                                  accessList: [rao], createdByName: "Dr. Anitha Rao", now: now)
        closed.status = .closed
        closed.closure = Closure(closedAt: now, closedBy: "u-lakshmi", feedback: "Explained everything clearly.", rating: 5)
        closed.messages = [msg("i-closed", "u-rao", "Continue the same dose for two more weeks.", minutesAgo: 20_000)]
        store.append(Stored(closed, createdBy: "u-rao"))

        // 6. Priya's report, not shared yet.
        var lipid = DataItem.new(id: "i-priya-lipid", owner: "u-priya", kind: .report, title: "Lipid profile", keywords: ["Lipid"],
                                 accessList: [lab], createdByName: "City Diagnostics", now: now)
        lipid.attachments = [att("a5", .pdf, "cbc_report.pdf", "application/pdf", 3_400, 0, "Lipid_profile.pdf", pages: 2)]
        store.append(Stored(lipid, createdBy: "l-city"))
    }

    private let subject = PassthroughSubject<RealtimeEvent, Never>()
    var events: AnyPublisher<RealtimeEvent, Never> { subject.receive(on: DispatchQueue.main).eraseToAnyPublisher() }
    func startRealtime() { subject.send(.connection(true)) }
    func stopRealtime() {}

    private func latency(_ ms: UInt64 = 150) async { try? await Task.sleep(nanoseconds: ms * 1_000_000) }

    // MARK: Access (design doc 2.3 on the whole item)
    private enum Access { case full, meta, deny }

    private func access(_ s: Stored, _ userId: String) -> Access {
        let it = s.item
        if it.ownerId == userId { return .full }
        let role = raw(userId).primaryRole
        if (role == .lab || role == .hospital) && s.createdBy == userId { return .full }
        let direct = it.accessList.contains { $0.granteeType == .user && $0.granteeId == userId }
        let viaHospital = role == .doctor && it.accessList.contains { $0.granteeType == .hospital && affiliation[userId] == $0.granteeId }
        guard direct || viaHospital else { return .deny }
        // Rule 2: report files are readable by doctors only; others get metadata.
        return it.attachments.contains(where: \.report) && role != .doctor ? .meta : .full
    }

    private func participants(_ it: DataItem) -> [String] {
        var out = [it.ownerId]
        for g in it.accessList where g.granteeType == .user && !out.contains(g.granteeId) { out.append(g.granteeId) }
        return out
    }

    /// What the signed-in user sees: children, derived kinds/counts/visits, allowed actions.
    private func present(_ s: Stored) throws -> DataItem {
        let a = access(s, meId)
        guard a != .deny else { throw RepoError("No access") }
        let base = s.item, full = a == .full, owner = base.ownerId == meId
        var out = base
        out.appointments = base.appointments.map { ap in
            var v = ap
            v.visits = RecurrenceRules.visits(start: ap.date, time: ap.time, ap.recurrence, exceptions: ap.exceptions,
                                              marks: s.marks[ap.id] ?? [:], cancelled: ap.status == "CANCELLED", from: Date())
            v.visitCount = ap.recurrence.flatMap { r in DateText.date(ap.date).flatMap { RecurrenceRules.visitCount($0, r) } } ?? (ap.recurrence == nil ? 1 : nil)
            if !full { v.notes = nil }
            return v
        }
        out.alerts = full ? base.alerts.filter { owner || $0.forUser == meId } : []
        var kinds = [base.primaryKind.rawValue]
        func add(_ k: String, _ yes: Bool) { if yes && !kinds.contains(k) { kinds.append(k) } }
        add(PartKind.report, base.attachments.contains(where: \.report))
        add(PartKind.appointment, !base.appointments.isEmpty)
        add(PartKind.message, !base.messages.isEmpty)
        add(PartKind.alert, !base.alerts.isEmpty)
        add(PartKind.attachment, !base.attachments.isEmpty)
        out.kinds = kinds
        var actions: [String] = []
        if !full { actions.append("meta") }
        else if base.status == .open { actions += ["read", "message", "attach", "book", "alert", "close"] }
        else { actions.append("read") }
        if full && owner { actions += ["share", "revoke"] } else if full && raw(meId).canRefer { actions.append("share") }
        if full && owner && base.status == .open { actions.append("rate") }
        if full && base.status == .closed && (owner || raw(meId).primaryRole == .doctor) { actions.append("reopen") }
        out.allowedActions = actions
        out.messages = full ? base.messages : []
        out.attachments = full ? base.sortedAttachments : []
        out.accessList = owner ? base.accessList : []
        out.links = full ? base.links : []
        out.counts = ItemCounts(messages: out.messages.count, attachments: out.attachments.count, appointments: out.appointments.count,
                                alerts: out.alerts.count, unreadMessages: unread["\(meId)|\(base.id)"] ?? 0)
        return out
    }

    private func stored(_ id: String) throws -> Stored {
        guard let s = store.first(where: { $0.item.id == id }) else { throw RepoError("Item not found") }
        return s
    }
    private func visible(_ status: ItemStatus) -> [Stored] { store.filter { $0.item.status == status && access($0, meId) != .deny } }

    /// Change an item the caller has full access to, then return the caller's view of it.
    private func change(_ id: String, _ f: (Stored) throws -> Void) async throws -> DataItem {
        await latency()
        let s = try stored(id)
        guard access(s, meId) == .full else { throw RepoError("No access") }
        try f(s)
        s.item.updatedAt = iso()
        subject.send(.itemChanged(id))
        return try present(s)
    }

    // MARK: Reads
    func me() async throws -> UserProfile { view(raw(meId)) }

    func dashboard() async throws -> Dashboard {
        await latency()
        let open = visible(.open).compactMap { try? present($0) }
        let soon = day(7)
        return Dashboard(openReports: open.filter { $0.kinds.contains(PartKind.report) }.count,
                         unreadMessages: open.reduce(0) { $0 + $1.counts.unreadMessages },
                         upcomingAppointments: open.flatMap(\.appointments).flatMap(\.visits).filter { $0.status == "SCHEDULED" && $0.date <= soon }.count)
    }

    func items(status: ItemStatus, limit: Int, kind: String?) async throws -> [DataItem] {
        await latency()
        return Array(visible(status).compactMap { try? present($0) }.filter { kind == nil || $0.kinds.contains(kind!) }.sorted { $0.updatedAt > $1.updatedAt }.prefix(limit))
    }

    func item(_ id: String) async throws -> DataItem { await latency(); return try present(try stored(id)) }
    func user(_ id: String) async throws -> UserProfile { await latency(); return view(raw(id)) }

    func sharedItems(with userId: String) async throws -> [DataItem] {
        await latency()
        guard isConnected(meId, userId) else { return [] }
        // Open and closed, newest first (closed ones are shown grey), as the server does.
        return store.filter { s in
            (s.item.ownerId == meId && access(s, userId) != .deny) || (s.item.ownerId == userId && access(s, meId) != .deny) }
            .compactMap { try? present($0) }.sorted { $0.updatedAt > $1.updatedAt }
    }

    func search(_ query: String, role: Role?) async throws -> [UserProfile] {
        await latency()
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        return users.filter { $0.id != meId && !deactivated.contains($0.id) }
            .filter { role == nil || $0.primaryRole == role }
            .filter { q.isEmpty || $0.displayName.lowercased().contains(q) || $0.publicId.lowercased().contains(q) }
            .map(view)
    }

    func connections() async throws -> [UserProfile] { await latency(); return users.filter { $0.id != meId && isConnected(meId, $0.id) }.map(view) }
    func hospitalDoctors(_ hospitalId: String) async throws -> [UserProfile] {
        await latency()
        return users.filter { $0.primaryRole == .doctor && affiliation[$0.id] == hospitalId && !deactivated.contains($0.id) }.sorted { $0.displayName < $1.displayName }.map(view)
    }
    func connect(_ userId: String) async throws -> UserProfile { await latency(); links.insert([meId, userId]); return view(raw(userId)) }

    func conversations(with withUser: String?) async throws -> [Conversation] {
        await latency()
        return store.filter { access($0, meId) == .full && !$0.item.messages.isEmpty && participants($0.item).contains(meId) }
            .flatMap { s -> [Conversation] in
                let last = s.item.messages.last!
                return participants(s.item).filter { $0 != meId && (withUser == nil || $0 == withUser) }.map { other in
                    Conversation(itemId: s.item.id, itemTitle: s.item.title, primaryKind: s.item.primaryKind, otherUser: view(raw(other)),
                                 lastMessage: last.body, lastMessageAt: last.sentAt, unread: unread["\(meId)|\(s.item.id)"] ?? 0)
                }
            }
            .sorted { $0.lastMessageAt > $1.lastMessageAt }
    }

    func calendar(from: String, to: String) async throws -> [CalendarVisit] {
        await latency()
        let isDoctor = raw(meId).primaryRole == .doctor
        return store.filter { access($0, meId) != .deny }.flatMap { s -> [CalendarVisit] in
            guard let it = try? present(s) else { return [] }
            return it.appointments.filter { isDoctor ? $0.doctorId == meId : $0.patientId == meId }.flatMap { a in
                a.visits.filter { $0.date >= from && $0.date <= to }.map { v in
                    let other = isDoctor ? a.patientId : a.doctorId
                    return CalendarVisit(startsAt: "\(v.date)T\(v.time):00+05:30", appointmentId: a.id, itemId: it.id, itemTitle: it.title,
                                         withUserId: other, withUserName: raw(other).displayName, status: v.status)
                }
            }
        }.sorted { $0.startsAt < $1.startsAt }
    }

    func itemMessages(_ itemId: String) async throws -> [Message] {
        await latency()
        let s = try stored(itemId)
        guard access(s, meId) == .full else { throw RepoError("No access") }
        unread["\(meId)|\(itemId)"] = nil
        return s.item.messages
    }

    // MARK: Create
    private func newItem(owner: String, kind: PrimaryKind, title: String, keywords: [String], shareWith: [String]) -> Stored {
        var grants = Array(Set(shareWith)).filter { $0 != owner }.map { grant(for: raw($0)) }
        if owner != meId && !grants.contains(where: { $0.granteeId == meId }) { grants.append(grant(for: raw(meId))) }   // uploader keeps access
        let s = Stored(DataItem.new(id: newId(), owner: owner, kind: kind, title: title.isEmpty ? kind.label : title, keywords: keywords,
                                    accessList: grants, createdByName: raw(meId).displayName, now: iso()), createdBy: meId)
        store.insert(s, at: 0)
        return s
    }

    /// Creating for a patient, the uploader may also refer the item to doctors only.
    private func doctorsOnly(_ ids: [String]) -> [String] { ids.filter { raw($0).primaryRole == .doctor } }

    private func toAttachments(_ files: [PendingAttachment], startAt: Int, isReport: Bool) -> [Attachment] {
        files.enumerated().map { i, f in
            Attachment(attachmentId: newId(), kind: f.kind, uri: f.localURL.absoluteString, mime: f.mime, size: f.size,
                       position: startAt + i, name: f.name, addedBy: meId, isReport: isReport)
        }
    }

    private func appointment(for itemId: String, owner: String, _ a: NewAppointment) throws -> Appointment {
        if let start = DateText.date(a.date), let p = RecurrenceRules.problem(start, a.recurrence) { throw RepoError(p) }
        guard raw(a.doctorId).primaryRole == .doctor else { throw RepoError("Choose a doctor for the appointment") }
        return appt(itemId, doctor: a.doctorId, patient: owner, date: a.date, time: a.time, a.recurrence, notes: a.notes,
                    hospital: a.hospitalId, timezone: a.timezone, duration: a.durationMin)
    }

    func createReport(_ draft: ReportDraft, files: [PendingAttachment]) async throws -> DataItem {
        await latency(600)
        guard !files.isEmpty || !draft.links.isEmpty else { throw RepoError("A report needs at least one file or link") }
        let s = newItem(owner: draft.ownerId, kind: .report, title: draft.title, keywords: draft.keywords,
                        shareWith: draft.ownerId == meId ? draft.shareWith : doctorsOnly(draft.shareWith))
        s.item.attachments = toAttachments(files, startAt: 0, isReport: true)
        s.item.links = draft.links
        subject.send(.itemChanged(s.item.id))
        return try present(s)
    }

    func createAppointment(ownerId: String?, _ appointment: NewAppointment, shareWith: [String]) async throws -> DataItem {
        await latency()
        let owner = ownerId ?? appointment.patientId
        let a = try self.appointment(for: "", owner: owner, appointment)
        let s = newItem(owner: owner, kind: .appointment, title: "Appointment with \(raw(appointment.doctorId).displayName)",
                        keywords: [], shareWith: shareWith + [appointment.doctorId])
        var ap = a; ap.itemId = s.item.id
        s.item.appointments = [ap]
        return try present(s)
    }

    func createAlert(title: String, _ alert: NewAlert, shareWith: [String]) async throws -> DataItem {
        await latency()
        let s = newItem(owner: meId, kind: .alert, title: title.isEmpty ? alert.text : title, keywords: [], shareWith: shareWith)
        s.item.alerts = [self.alert(s.item.id, forUser: alert.forUser ?? meId, alert.type, alert.text, date: alert.date, time: alert.time, alert.recurrence)]
        return try present(s)
    }

    func startConversation(with userId: String, body: String, alsoWith: [String]) async throws -> DataItem {
        await latency()
        let other = raw(userId), me = raw(meId)
        let owner = other.primaryRole == .patient && me.isClinical ? other.id : meId
        let firstLine = body.split(separator: "\n").first.map(String.init) ?? body
        let s = newItem(owner: owner, kind: .message, title: String(firstLine.prefix(60)), keywords: [], shareWith: owner == meId ? [userId] + alsoWith : doctorsOnly(alsoWith))
        _ = try await sendItemMessage(s.item.id, body: body)
        return try await item(s.item.id)
    }

    // MARK: Add to an item
    func sendItemMessage(_ itemId: String, body: String) async throws -> Message {
        var sent: Message?
        _ = try await change(itemId) { s in
            guard s.item.status == .open else { throw RepoError("This item is closed") }
            let m = Message(messageId: newId(), itemId: itemId, senderId: meId, body: body.trimmingCharacters(in: .whitespacesAndNewlines),
                            sentAt: iso(), attachmentIds: [])
            s.item.messages.append(m)
            for p in participants(s.item) where p != meId { unread["\(p)|\(itemId)", default: 0] += 1 }
            sent = m
        }
        // Simulate the other person answering over the WebSocket, so live delivery can be tested.
        let s = try stored(itemId)
        if let other = participants(s.item).first(where: { $0 != meId }).map(raw), other.primaryRole != .hospital {
            Task {
                try? await Task.sleep(nanoseconds: 1_500_000_000)
                let reply = Message(messageId: newId(), itemId: itemId, senderId: other.id,
                                    body: other.primaryRole == .patient ? "Thank you, doctor." : "Noted — I'll review it today.", sentAt: iso(), attachmentIds: [])
                s.item.messages.append(reply)
                subject.send(.newMessage(reply))
            }
        }
        return sent!
    }

    func addAttachments(_ itemId: String, files: [PendingAttachment], isReport: Bool) async throws -> DataItem {
        try await change(itemId) { s in s.item.attachments += toAttachments(files, startAt: s.item.attachments.count, isReport: isReport) }
    }

    func bookAppointment(_ itemId: String, _ appointment: NewAppointment) async throws -> DataItem {
        try await change(itemId) { s in
            guard s.item.status == .open else { throw RepoError("This item is closed") }
            s.item.appointments.append(try self.appointment(for: itemId, owner: s.item.ownerId, appointment))
            let doctor = raw(appointment.doctorId)
            if doctor.id != s.item.ownerId && !s.item.accessList.contains(where: { $0.granteeId == doctor.id }) {
                s.item.accessList.append(grant(for: doctor))       // booking grants the doctor access
            }
        }
    }

    func visitAction(_ itemId: String, appointmentId: String, visitDate: String, action: String, newDate: String?, newTime: String?) async throws -> DataItem {
        try await change(itemId) { s in
            guard let i = s.item.appointments.firstIndex(where: { $0.id == appointmentId }) else { throw RepoError("Appointment not found") }
            switch action {
            case "CANCELLED", "MOVED":
                s.item.appointments[i].exceptions.removeAll { $0.date == visitDate }
                s.item.appointments[i].exceptions.append(VisitException(date: visitDate, action: action, newDate: newDate, newTime: newTime))
            default:
                s.marks[appointmentId, default: [:]][visitDate] = action
            }
        }
    }

    func cancelAppointment(_ itemId: String, appointmentId: String) async throws -> DataItem {
        try await change(itemId) { s in
            for i in s.item.appointments.indices where s.item.appointments[i].id == appointmentId { s.item.appointments[i].status = "CANCELLED" }
        }
    }

    func addAlert(_ itemId: String, _ alert: NewAlert) async throws -> DataItem {
        try await change(itemId) { s in
            s.item.alerts.append(self.alert(itemId, forUser: alert.forUser ?? meId, alert.type, alert.text, date: alert.date, time: alert.time, alert.recurrence))
        }
    }

    func deleteAlert(_ itemId: String, alertId: String) async throws -> DataItem {
        try await change(itemId) { s in s.item.alerts.removeAll { $0.id == alertId } }
    }

    func closeItem(_ itemId: String, feedback: String?, rating: Int?) async throws -> DataItem {
        try await change(itemId) { s in
            guard rating == nil || s.item.ownerId == meId else { throw RepoError("Only the patient can rate") }
            let text = feedback?.trimmingCharacters(in: .whitespacesAndNewlines)
            s.item.status = .closed
            s.item.closure = Closure(closedAt: iso(), closedBy: meId, feedback: text?.isEmpty == false ? text : nil, rating: rating)
            for i in s.item.appointments.indices { s.item.appointments[i].status = "CANCELLED" }   // future visits end
            for i in s.item.alerts.indices { s.item.alerts[i].active = false }
        }
    }

    func reopenItem(_ itemId: String) async throws -> DataItem {
        try await change(itemId) { s in s.item.status = .open; s.item.closure = nil }
    }

    // MARK: Sharing
    func share(_ itemId: String, with granteeId: String) async throws -> DataItem {
        try await change(itemId) { s in
            // The owner shares with anyone; a doctor, lab or hospital may pass it on to a doctor.
            guard s.item.ownerId == meId || (raw(meId).canRefer && raw(granteeId).primaryRole == .doctor) else {
                throw RepoError("Only the owner can share this with \(raw(granteeId).displayName); you can pass it on to doctors")
            }
            if !s.item.accessList.contains(where: { $0.granteeId == granteeId }) { s.item.accessList.append(grant(for: raw(granteeId))) }
        }
    }

    func revoke(_ itemId: String, grantId: String) async throws -> DataItem {
        try await change(itemId) { s in s.item.accessList.removeAll { $0.grantId == grantId } }
    }

    // MARK: Administration (rule 10: create users and doctors, deactivate doctors only)
    private func myHospital() throws -> String {
        guard let h = adminHospital[meId] else { throw RepoError("Only hospital administrators can manage accounts") }
        return h
    }

    private func adminView(_ u: UserProfile) -> AdminAccount {
        AdminAccount(userId: u.id, publicId: u.publicId, displayName: u.displayName, roles: u.roles, headline: u.headline,
                     hospital: u.hospital, officialNumber: u.officialNumber, email: emails[u.id], active: !deactivated.contains(u.id))
    }

    func adminDoctors() async throws -> [AdminAccount] {
        await latency()
        let h = try myHospital()
        return users.filter { $0.primaryRole == .doctor && (affiliation[$0.id] == h || (deactivated.contains($0.id) && homeHospital[$0.id] == h)) }
            .sorted { (deactivated.contains($0.id) ? 1 : 0, $0.displayName) < (deactivated.contains($1.id) ? 1 : 0, $1.displayName) }
            .map(adminView)
    }

    private func newAccount(_ a: NewAccount, role: Role) throws -> AdminAccount {
        let h = try myHospital()
        let email = a.email.trimmingCharacters(in: .whitespaces).lowercased()
        let name = a.displayName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { throw RepoError("Enter a name") }
        guard email.range(of: #"^[^@\s]+@[^@\s]+\.[^@\s]+$"#, options: .regularExpression) != nil else { throw RepoError("Enter a valid email address") }
        guard !emails.values.contains(email) else { throw RepoError("An account with that email already exists") }
        let doctor = role == .doctor
        let alphabet = Array("23456789ABCDEFGHJKLMNPQRSTUVWXYZ")
        var publicId = ""
        repeat { publicId = "HL-" + String((0..<5).map { _ in alphabet.randomElement()! }) } while users.contains { $0.publicId == publicId }
        let designation = a.designation?.trimmingCharacters(in: .whitespaces) ?? ""
        let u = UserProfile(userId: "n-" + String(newId().prefix(8)), publicId: publicId, displayName: name, roles: [role],
                            headline: doctor ? (designation.isEmpty ? "Doctor" : "Doctor · \(designation)") : "Patient",
                            hospital: doctor ? raw(h).displayName : nil,
                            officialNumber: doctor ? a.officialNumber.flatMap { $0.isEmpty ? nil : $0 } : nil)
        users.append(u)
        emails[u.id] = email
        if doctor { affiliation[u.id] = h; homeHospital[u.id] = h; links.insert([u.id, h]) }
        return adminView(u)
    }

    func adminCreateUser(_ account: NewAccount) async throws -> AdminAccount { await latency(); return try newAccount(account, role: .patient) }
    func adminCreateDoctor(_ account: NewAccount) async throws -> AdminAccount { await latency(); return try newAccount(account, role: .doctor) }

    func adminDeactivateDoctor(_ doctorId: String) async throws {
        await latency()
        let h = try myHospital()
        guard raw(doctorId).primaryRole == .doctor else { throw RepoError("Users can't be deleted; administrators can delete (deactivate) doctors only") }
        guard affiliation[doctorId] == h || homeHospital[doctorId] == h else { throw RepoError("You can delete only doctors of your own hospital") }
        affiliation[doctorId] = nil
        deactivated.insert(doctorId)
    }

    func adminReactivateDoctor(_ doctorId: String) async throws -> AdminAccount {
        await latency()
        let h = try myHospital()
        guard homeHospital[doctorId] == h else { throw RepoError("You can reactivate only doctors of your own hospital") }
        deactivated.remove(doctorId)
        affiliation[doctorId] = h
        return adminView(raw(doctorId))
    }

    // MARK: Account
    func updateProfile(_ update: ProfileUpdate) async throws -> UserProfile {
        await latency()
        let i = users.firstIndex { $0.id == meId }!
        users[i].displayName = update.displayName.trimmingCharacters(in: .whitespaces)
        users[i].location = update.location?.isEmpty == true ? nil : update.location
        return view(users[i])
    }

    // Demo mode: the photo stays on the phone (a file in the caches folder).
    func setProfilePhoto(_ photo: PendingAttachment) async throws -> UserProfile {
        await latency()
        let i = users.firstIndex { $0.id == meId }!
        users[i].photoUri = photo.localURL.absoluteString
        return view(users[i])
    }

    func removeProfilePhoto() async throws -> UserProfile {
        await latency()
        let i = users.firstIndex { $0.id == meId }!
        users[i].photoUri = nil
        return view(users[i])
    }

    func activeShares() async throws -> [ShareGroup] {
        await latency()
        let grants = store.filter { $0.item.ownerId == meId }.flatMap { s in
            s.item.accessList.map { OwnedGrant(grantId: $0.grantId, granteeType: $0.granteeType, granteeId: $0.granteeId,
                                               granteeName: $0.granteeName, itemId: s.item.id, itemTitle: s.item.title, primaryKind: s.item.primaryKind) }
        }
        return Dictionary(grouping: grants, by: \.granteeId).map { id, list in
            ShareGroup(granteeId: id, granteeName: list[0].granteeName, granteeType: list[0].granteeType, grants: list)
        }.sorted { $0.granteeName < $1.granteeName }
    }

    func notificationPrefs() async throws -> NotificationPrefs { await latency(); return prefs[meId] ?? NotificationPrefs() }
    func saveNotificationPrefs(_ p: NotificationPrefs) async throws -> NotificationPrefs { await latency(); prefs[meId] = p; return p }
    func registerDevice(token: String) async throws {}
    func unregisterDevice(token: String) async throws {}
}

// MARK: - Remote (design doc 4.3 and DataItem_Design.md section 4)

final class RemoteRepository: HealooRepository {
    private let base: URL
    private let auth: AuthManager
    private let session = URLSession(configuration: .default)
    private let decoder: JSONDecoder = { let d = JSONDecoder(); d.keyDecodingStrategy = .convertFromSnakeCase; return d }()
    private let encoder: JSONEncoder = { let e = JSONEncoder(); e.keyEncodingStrategy = .convertToSnakeCase; return e }()
    private lazy var realtime = RealtimeClient(baseURL: base, decoder: decoder, refresh: { [auth = self.auth] in await auth.freshToken() })

    init(baseURL: URL, auth: AuthManager) { base = baseURL; self.auth = auth }

    var events: AnyPublisher<RealtimeEvent, Never> { realtime.events }
    func startRealtime() { realtime.start() }
    func stopRealtime() { realtime.stop() }

    /// Server error body: {"error": {"code", "message", "request_id"}}
    private struct ServerError: Decodable { struct Detail: Decodable { let code: String?; let message: String? }; let error: Detail? }

    private func request<T: Decodable>(_ method: String, _ path: String, query: [String: String?] = [:],
                                       body: (any Encodable)? = nil, retried: Bool = false) async throws -> T {
        var comps = URLComponents(url: base.appendingPathComponent(path), resolvingAgainstBaseURL: false)!
        let items = query.compactMap { k, v in v.map { URLQueryItem(name: k, value: $0) } }
        if !items.isEmpty { comps.queryItems = items }
        var req = URLRequest(url: comps.url!)
        req.httpMethod = method
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if let t = TokenStore.accessToken { req.setValue("Bearer \(t)", forHTTPHeaderField: "Authorization") }
        if let body { req.httpBody = try encoder.encode(body); req.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        let (data, resp) = try await session.data(for: req)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        // Expired access token -> refresh once through Auth0, then retry (doc 5.4).
        if code == 401, !retried, await auth.freshToken() != nil {
            return try await request(method, path, query: query, body: body, retried: true)
        }
        guard (200..<300).contains(code) else {
            // The server explains 400s (for example an invalid repeat/period); show that text.
            let detail = (try? decoder.decode(ServerError.self, from: data))?.error?.message
            throw RepoError(detail ?? "Server returned \(code)")
        }
        if T.self == Empty.self { return Empty() as! T }
        return try decoder.decode(T.self, from: data)
    }

    struct Empty: Codable {}
    struct ConnectBody: Encodable { let userId: String }
    struct GrantBody: Encodable { let itemIds: [String]; let granteeId: String }
    struct DeviceBody: Encodable { let platform: String; let token: String }
    struct CancelBody: Encodable { let status = "CANCELLED" }
    struct PresignBody: Encodable { struct File: Encodable { let name: String; let mime: String; let size: Int64 }; let files: [File] }
    struct PresignResponse: Decodable { struct Upload: Decodable { let uploadUrl: String; let uri: String }; let uploads: [Upload] }

    func me() async throws -> UserProfile { try await request("GET", "v1/me") }
    func dashboard() async throws -> Dashboard { try await request("GET", "v1/dashboard") }
    func items(status: ItemStatus, limit: Int, kind: String?) async throws -> [DataItem] {
        let p: PageResult<DataItem> = try await request("GET", "v1/items", query: ["status": status.rawValue, "limit": String(limit), "kind": kind]); return p.data
    }
    func item(_ id: String) async throws -> DataItem { try await request("GET", "v1/items/\(id)") }
    func user(_ id: String) async throws -> UserProfile { try await request("GET", "v1/users/\(id)") }
    func sharedItems(with userId: String) async throws -> [DataItem] {
        let p: PageResult<DataItem> = try await request("GET", "v1/users/\(userId)/shared-items", query: ["status": "OPEN"]); return p.data
    }
    func search(_ query: String, role: Role?) async throws -> [UserProfile] {
        let p: PageResult<UserProfile> = try await request("GET", "v1/search", query: ["q": query, "type": role?.rawValue]); return p.data
    }
    func connections() async throws -> [UserProfile] { let p: PageResult<UserProfile> = try await request("GET", "v1/connections"); return p.data }
    func hospitalDoctors(_ hospitalId: String) async throws -> [UserProfile] {
        let p: PageResult<UserProfile> = try await request("GET", "v1/hospitals/\(hospitalId)/doctors"); return p.data
    }
    func connect(_ userId: String) async throws -> UserProfile { try await request("POST", "v1/connections", body: ConnectBody(userId: userId)) }
    func share(_ itemId: String, with granteeId: String) async throws -> DataItem {
        let _: Empty = try await request("POST", "v1/grants", body: GrantBody(itemIds: [itemId], granteeId: granteeId)); return try await item(itemId)
    }
    func revoke(_ itemId: String, grantId: String) async throws -> DataItem {
        let _: Empty = try await request("DELETE", "v1/grants/\(grantId)"); return try await item(itemId)
    }

    // Create
    func createReport(_ draft: ReportDraft, files: [PendingAttachment]) async throws -> DataItem {
        let attachments = try await uploadFiles(files)
        return try await request("POST", "v1/items", body: NewItemRequest(ownerId: draft.ownerId, keywords: draft.keywords, shareWith: draft.shareWith,
                                                                         report: NewReport(title: draft.title, attachments: attachments, links: draft.links)))
    }
    func createAppointment(ownerId: String?, _ appointment: NewAppointment, shareWith: [String]) async throws -> DataItem {
        try await request("POST", "v1/items", body: NewItemRequest(ownerId: ownerId, shareWith: shareWith, appointment: appointment))
    }
    func createAlert(title: String, _ alert: NewAlert, shareWith: [String]) async throws -> DataItem {
        try await request("POST", "v1/items", body: NewItemRequest(title: title, shareWith: shareWith, alert: alert))
    }
    func startConversation(with userId: String, body: String, alsoWith: [String]) async throws -> DataItem {
        let me = try await self.me(), other = try await user(userId)
        // The patient in the pair owns the discussion; a clinician starts it on the patient's behalf.
        let clinicianToPatient = other.primaryRole == .patient && me.isClinical
        return try await request("POST", "v1/items", body: NewItemRequest(
            ownerId: clinicianToPatient ? other.id : me.id, shareWith: clinicianToPatient ? alsoWith : [userId] + alsoWith,
            message: NewMessage(body: body, clientMsgId: UUID().uuidString)))
    }

    // Add to an item
    func itemMessages(_ itemId: String) async throws -> [Message] {
        let p: PageResult<Message> = try await request("GET", "v1/items/\(itemId)/messages"); return p.data
    }
    func sendItemMessage(_ itemId: String, body: String) async throws -> Message {
        try await request("POST", "v1/items/\(itemId)/messages", body: NewMessage(body: body, clientMsgId: UUID().uuidString))
    }
    func addAttachments(_ itemId: String, files: [PendingAttachment], isReport: Bool) async throws -> DataItem {
        let attachments = try await uploadFiles(files)
        let _: PageResult<Attachment> = try await request("POST", "v1/items/\(itemId)/attachments",
                                                          body: AddAttachmentsRequest(attachments: attachments, isReport: isReport))
        return try await item(itemId)
    }
    func bookAppointment(_ itemId: String, _ appointment: NewAppointment) async throws -> DataItem {
        let _: Appointment = try await request("POST", "v1/items/\(itemId)/appointments", body: appointment); return try await item(itemId)
    }
    func visitAction(_ itemId: String, appointmentId: String, visitDate: String, action: String, newDate: String?, newTime: String?) async throws -> DataItem {
        let _: Appointment = try await request("POST", "v1/items/\(itemId)/appointments/\(appointmentId)/visits/\(visitDate)",
                                               body: VisitActionRequest(action: action, newDate: newDate, newTime: newTime))
        return try await item(itemId)
    }
    func cancelAppointment(_ itemId: String, appointmentId: String) async throws -> DataItem {
        let _: Appointment = try await request("PATCH", "v1/items/\(itemId)/appointments/\(appointmentId)", body: CancelBody()); return try await item(itemId)
    }
    func addAlert(_ itemId: String, _ alert: NewAlert) async throws -> DataItem {
        let _: Alert = try await request("POST", "v1/items/\(itemId)/alerts", body: alert); return try await item(itemId)
    }
    func deleteAlert(_ itemId: String, alertId: String) async throws -> DataItem {
        let _: Empty = try await request("DELETE", "v1/items/\(itemId)/alerts/\(alertId)"); return try await item(itemId)
    }
    func closeItem(_ itemId: String, feedback: String?, rating: Int?) async throws -> DataItem {
        try await request("POST", "v1/items/\(itemId)/close", body: CloseRequest(feedback: feedback, rating: rating))
    }
    func reopenItem(_ itemId: String) async throws -> DataItem { try await request("POST", "v1/items/\(itemId)/reopen") }

    func conversations(with withUser: String?) async throws -> [Conversation] {
        let p: PageResult<Conversation> = try await request("GET", "v1/conversations", query: ["with": withUser]); return p.data
    }
    func calendar(from: String, to: String) async throws -> [CalendarVisit] {
        let p: PageResult<CalendarVisit> = try await request("GET", "v1/appointments", query: ["from": from, "to": to]); return p.data
    }

    // Administration (hospital administrators; users can't be deleted)
    /// The administrator's hospital's doctors, deactivated ones included.
    func adminDoctors() async throws -> [AdminAccount]
    func adminCreateUser(_ account: NewAccount) async throws -> AdminAccount
    func adminCreateDoctor(_ account: NewAccount) async throws -> AdminAccount
    /// "Delete" a doctor: deactivates the account and ends the hospital affiliation.
    func adminDeactivateDoctor(_ doctorId: String) async throws
    func adminReactivateDoctor(_ doctorId: String) async throws -> AdminAccount

    // Account
    // Administration
    func adminDoctors() async throws -> [AdminAccount] {
        let p: PageResult<AdminAccount> = try await request("GET", "v1/admin/doctors"); return p.data
    }
    func adminCreateUser(_ account: NewAccount) async throws -> AdminAccount { try await request("POST", "v1/admin/users", body: account) }
    func adminCreateDoctor(_ account: NewAccount) async throws -> AdminAccount { try await request("POST", "v1/admin/doctors", body: account) }
    func adminDeactivateDoctor(_ doctorId: String) async throws { let _: Empty = try await request("DELETE", "v1/admin/doctors/\(doctorId)") }
    func adminReactivateDoctor(_ doctorId: String) async throws -> AdminAccount { try await request("POST", "v1/admin/doctors/\(doctorId)/reactivate") }

    func updateProfile(_ update: ProfileUpdate) async throws -> UserProfile { try await request("PATCH", "v1/me", body: update) }
    func setProfilePhoto(_ photo: PendingAttachment) async throws -> UserProfile {
        let uploaded = try await uploadFiles([photo])
        return try await request("PUT", "v1/me/photo", body: PhotoUpdate(uri: uploaded[0].uri))
    }
    func removeProfilePhoto() async throws -> UserProfile {
        let _: Empty = try await request("DELETE", "v1/me/photo")
        return try await me()
    }
    func activeShares() async throws -> [ShareGroup] {
        let p: PageResult<OwnedGrant> = try await request("GET", "v1/grants", query: ["owner": "me"])
        return Dictionary(grouping: p.data, by: \.granteeId).map { id, list in
            ShareGroup(granteeId: id, granteeName: list[0].granteeName, granteeType: list[0].granteeType, grants: list.sorted { $0.itemTitle < $1.itemTitle })
        }.sorted { $0.granteeName < $1.granteeName }
    }
    func notificationPrefs() async throws -> NotificationPrefs { try await request("GET", "v1/me/notification-prefs") }
    func saveNotificationPrefs(_ prefs: NotificationPrefs) async throws -> NotificationPrefs { try await request("PUT", "v1/me/notification-prefs", body: prefs) }
    func registerDevice(token: String) async throws { let _: Empty = try await request("POST", "v1/devices", body: DeviceBody(platform: "ios", token: token)) }
    func unregisterDevice(token: String) async throws { let _: Empty = try await request("DELETE", "v1/devices/\(token)") }

    /// Presign all files in one call and PUT each to storage; returns them ready to attach.
    private func uploadFiles(_ files: [PendingAttachment]) async throws -> [NewAttachment] {
        guard !files.isEmpty else { return [] }
        let presigned: PresignResponse = try await request("POST", "v1/uploads/presign",
            body: PresignBody(files: files.map { .init(name: $0.name, mime: $0.mime, size: $0.size) }))
        for (file, target) in zip(files, presigned.uploads) {
            var put = URLRequest(url: URL(string: target.uploadUrl)!)
            put.httpMethod = "PUT"
            put.setValue(file.mime, forHTTPHeaderField: "Content-Type")
            let (_, resp) = try await session.upload(for: put, fromFile: file.localURL)
            guard (resp as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) == true else {
                throw RepoError("Upload of \(file.name) failed")
            }
        }
        return zip(files, presigned.uploads).map { f, p in NewAttachment(kind: f.kind, uri: p.uri, mime: f.mime, size: f.size, name: f.name) }
    }
}

// MARK: - Attachment files

/// Resolves attachment URIs (bundle samples, local files, presigned https URLs) to local files.
///
/// Two size-limited caches, about 100 MB together (same limits as Android's AttachmentFiles):
/// full images and PDFs keep the 15 most recently used files and at most 90 MB; thumbnails at
/// most 10 MB. Each download evicts the least recently used files.
enum AttachmentStore {
    struct Limits { let folder: String; let maxFiles: Int; let maxBytes: Int64 }
    static let full = Limits(folder: "attachments", maxFiles: 15, maxBytes: 90 * 1024 * 1024)
    static let thumbs = Limits(folder: "thumbnails", maxFiles: .max, maxBytes: 10 * 1024 * 1024)
    /// The newest files are never evicted, even over the byte limit (they may be on screen).
    static let keepNewest = 3

    /// The full image or PDF. Downloaded only when it is shown in the viewer (or when no thumbnail exists).
    static func localURL(for a: Attachment) async throws -> URL {
        try await localFile(a.uri, name: a.name, cache: full)
    }

    /// The 320-px thumbnail the media worker made (images, and the first page of PDFs), for cards
    /// and the viewer's strip. Nil when the server has no thumbnail (yet).
    static func thumbnailURL(for a: Attachment) async throws -> URL? {
        guard let t = a.thumbUri, !t.isEmpty else { return nil }
        return try await localFile(t, name: "thumb-\((a.name as NSString).deletingPathExtension).jpg", cache: thumbs)
    }

    /// A small image such as a profile picture, cached with the thumbnails.
    static func imageURL(_ uri: String, name: String) async throws -> URL {
        try await localFile(uri, name: name, cache: thumbs)
    }

    /// Deletes the least recently used files beyond the cache's file or byte limit.
    static func trim(_ dir: URL, _ limits: Limits) {
        let fm = FileManager.default
        let keys: [URLResourceKey] = [.contentModificationDateKey, .fileSizeKey, .isRegularFileKey]
        guard let urls = try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: keys) else { return }
        let files = urls.compactMap { u -> (URL, Date, Int64)? in
            guard let v = try? u.resourceValues(forKeys: Set(keys)), v.isRegularFile == true else { return nil }
            return (u, v.contentModificationDate ?? .distantPast, Int64(v.fileSize ?? 0))
        }.sorted { $0.1 > $1.1 }
        var bytes: Int64 = 0
        for (i, f) in files.enumerated() {
            bytes += f.2
            if i >= keepNewest && (i >= limits.maxFiles || bytes > limits.maxBytes) { try? fm.removeItem(at: f.0) }
        }
    }

    private static func touch(_ url: URL) {
        try? FileManager.default.setAttributes([.modificationDate: Date()], ofItemAtPath: url.path)
    }

    private static func localFile(_ uri: String, name: String, cache: Limits) async throws -> URL {
        if uri.hasPrefix("bundle://") {
            let name = String(uri.dropFirst("bundle://".count))
            let ext = (name as NSString).pathExtension, stem = (name as NSString).deletingPathExtension
            guard let url = Bundle.main.url(forResource: stem, withExtension: ext) else { throw URLError(.fileDoesNotExist) }
            return url
        }
        guard let remote = URL(string: uri) else { throw URLError(.badURL) }
        if remote.isFileURL { return remote }

        // Presigned URLs change on every fetch; key the cache on the path without the query string.
        let dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent(cache.folder, isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let key = SHA256.hash(data: Data(remote.path.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
        let target = dir.appendingPathComponent("\(key)-\(name)")
        if FileManager.default.fileExists(atPath: target.path) { touch(target); return target }   // recently used: evicted last
        let (tmp, resp) = try await URLSession.shared.download(from: remote)
        guard (resp as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) == true else { throw URLError(.badServerResponse) }
        try? FileManager.default.removeItem(at: target)
        try FileManager.default.moveItem(at: tmp, to: target)
        touch(target)
        trim(dir, cache)
        return target
    }
}
