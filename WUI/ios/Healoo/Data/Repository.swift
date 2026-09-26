import Foundation
import Combine
import CryptoKit

/// Everything the WUI needs. Mock and remote implementations are interchangeable.
protocol HealooRepository: AnyObject {
    func me() async throws -> UserProfile
    func dashboard() async throws -> Dashboard
    func openItems(limit: Int) async throws -> [DataItem]
    func item(_ id: String) async throws -> DataItem
    func user(_ id: String) async throws -> UserProfile
    /// Items shared between the caller and `userId`. Empty unless connected (doc 2.3 rule 8).
    func sharedItems(with userId: String) async throws -> [DataItem]
    func messages(with userId: String) async throws -> [Message]
    func send(to userId: String, body: String) async throws -> Message
    func threads() async throws -> [ThreadSummary]
    /// Global search by Healoo ID or name. Profiles only, never data.
    func search(_ query: String, role: Role?) async throws -> [UserProfile]
    func connections() async throws -> [UserProfile]
    func connect(_ userId: String) async throws -> UserProfile
    func setStatus(_ itemId: String, _ status: ItemStatus) async throws -> DataItem
    func share(_ itemId: String, with granteeId: String) async throws -> DataItem
    func revoke(_ itemId: String, grantId: String) async throws -> DataItem
    func upload(_ draft: UploadDraft, files: [PendingAttachment]) async throws -> DataItem

    // Added in 0.2
    func updateProfile(_ update: ProfileUpdate) async throws -> UserProfile
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
        await PushManager.shared.unregister(repo: repo)
        repo.stopRealtime()
        if !useFakeData { await auth.logout() } else { auth.signedOut() }
    }
}

/// Current Auth0 access token (design doc 5.4); kept fresh by AuthManager.
enum TokenStore { nonisolated(unsafe) static var accessToken: String? }

// MARK: - Mock (multi-user demo backend)

/// Seeded demo data evaluated from the signed-in user's point of view, so the patient, doctor and
/// lab flows can all be tried. Access follows doc 2.3: owners see their items; others only when
/// granted directly or through a hospital they are affiliated with.
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
        UserProfile(userId: "u-priya", publicId: "HL-4K7Q2", displayName: "Priya Rao", roles: [.patient], headline: "User"),
    ]

    var demoAccounts: [UserProfile] { users.filter { ["u-lakshmi", "u-rao", "l-city"].contains($0.id) } }
    private var meId = "u-lakshmi"
    func signIn(as userId: String) { meId = userId }

    private let affiliation = ["u-rao": "h-a"]
    private var links: Set<Set<String>> = [
        ["u-lakshmi", "u-rao"], ["u-lakshmi", "h-a"], ["u-lakshmi", "l-city"], ["u-rao", "h-a"], ["u-rao", "l-city"], ["u-priya", "l-city"],
    ]

    private func raw(_ id: String) -> UserProfile { users.first { $0.id == id }! }
    private func isConnected(_ a: String, _ b: String) -> Bool { links.contains([a, b]) }
    private func view(_ u: UserProfile) -> UserProfile { var v = u; v.connected = isConnected(meId, u.id); return v }
    private func grant(for u: UserProfile) -> Grant {
        Grant(grantId: UUID().uuidString, granteeType: u.primaryRole == .hospital ? .hospital : .user, granteeId: u.id, granteeName: u.displayName)
    }
    private func canSee(_ item: DataItem, _ userId: String) -> Bool {
        item.ownerId == userId || item.accessList.contains { $0.granteeId == userId || ($0.granteeType == .hospital && affiliation[userId] == $0.granteeId) }
    }

    private lazy var items: [DataItem] = [
        DataItem(itemId: "i-cbc", date: "2026-09-20", coreItemType: .report, title: "CBC – Complete blood count",
                 subtitle: "City Diagnostics · Lab report", keywords: ["CBC", "Haemoglobin", "Routine check"],
                 coreItemData: [
                    Attachment(kind: .image, uri: sample("cbc_scan_1.jpg"), mime: "image/jpeg", size: 91_000, position: 0, name: "cbc_scan_1.jpg"),
                    Attachment(kind: .image, uri: sample("cbc_scan_2.jpg"), mime: "image/jpeg", size: 92_000, position: 1, name: "cbc_scan_2.jpg"),
                    Attachment(kind: .pdf, uri: sample("cbc_report.pdf"), mime: "application/pdf", size: 3_400, position: 2, name: "CBC_20Sep2026.pdf", pageCount: 3),
                    Attachment(kind: .pdf, uri: sample("reference_ranges.pdf"), mime: "application/pdf", size: 2_500, position: 3, name: "Reference_ranges.pdf", pageCount: 2),
                 ],
                 ownerId: "u-lakshmi", createdByName: "City Diagnostics",
                 accessList: [Grant(grantId: "g1", granteeType: .hospital, granteeId: "h-a", granteeName: "Test Hospital A"),
                              Grant(grantId: "g2", granteeType: .user, granteeId: "u-rao", granteeName: "Dr. Anitha Rao"),
                              Grant(grantId: "g3", granteeType: .user, granteeId: "l-city", granteeName: "City Diagnostics")],
                 pointerItemId: "i-followup", allowedActions: ["read", "share", "revoke", "status"]),
        DataItem(itemId: "i-msg", date: "2026-09-22", coreItemType: .message, title: "Dr. Anitha Rao", subtitle: "Please share your latest BP readings",
                 ownerId: "u-lakshmi", createdByName: "Dr. Anitha Rao", accessList: [Grant(grantId: "g4", granteeType: .user, granteeId: "u-rao", granteeName: "Dr. Anitha Rao")]),
        DataItem(itemId: "i-followup", date: "2026-09-24", coreItemType: .booking, title: "Follow-up consultation", subtitle: "Test Hospital A · 24 Sep, 11:30",
                 ownerId: "u-lakshmi", createdByName: "Dr. Anitha Rao", accessList: [Grant(grantId: "g5", granteeType: .hospital, granteeId: "h-a", granteeName: "Test Hospital A")],
                 pointerItemId: "i-cbc"),
        DataItem(itemId: "i-med", date: "2026-09-22", coreItemType: .alert, title: "Medication reminder", subtitle: "Evening dose · 8:00 PM",
                 ownerId: "u-lakshmi", createdByName: "Dr. Anitha Rao", accessList: [Grant(grantId: "g6", granteeType: .user, granteeId: "u-rao", granteeName: "Dr. Anitha Rao")]),
        DataItem(itemId: "i-fee", date: "2026-09-18", coreItemType: .payment, title: "Consultation fee", subtitle: "₹600 · Payment pending",
                 ownerId: "u-lakshmi", createdByName: "Test Hospital A", accessList: [Grant(grantId: "g7", granteeType: .hospital, granteeId: "h-a", granteeName: "Test Hospital A")]),
        DataItem(itemId: "i-feedback", date: "2026-09-10", coreItemType: .feedback, title: "Visit feedback", subtitle: "Rate your 10 Sep consultation",
                 ownerId: "u-lakshmi", createdByName: "Dr. Anitha Rao", accessList: [Grant(grantId: "g8", granteeType: .user, granteeId: "u-rao", granteeName: "Dr. Anitha Rao")]),
        DataItem(itemId: "i-priya-lipid", date: "2026-09-19", coreItemType: .report, title: "Lipid profile", subtitle: "City Diagnostics · Lab report",
                 ownerId: "u-priya", createdByName: "City Diagnostics", accessList: [Grant(grantId: "g9", granteeType: .user, granteeId: "l-city", granteeName: "City Diagnostics")]),
    ]

    private func key(_ a: String, _ b: String) -> String { [a, b].sorted().joined(separator: "|") }
    private lazy var chats: [String: [Message]] = [
        key("u-lakshmi", "u-rao"): [
            Message(messageId: "m1", threadId: "t-u-lakshmi|u-rao", senderId: "u-rao", body: "Please share your latest BP readings before Thursday.", sentAt: "10:42"),
            Message(messageId: "m2", threadId: "t-u-lakshmi|u-rao", senderId: "u-lakshmi", body: "Sure, I will upload them tonight.", sentAt: "10:50"),
        ],
    ]
    private var prefs: [String: NotificationPrefs] = [:]

    private let subject = PassthroughSubject<RealtimeEvent, Never>()
    var events: AnyPublisher<RealtimeEvent, Never> { subject.receive(on: DispatchQueue.main).eraseToAnyPublisher() }
    func startRealtime() { subject.send(.connection(true)) }
    func stopRealtime() {}

    private func latency(_ ms: UInt64 = 150) async { try? await Task.sleep(nanoseconds: ms * 1_000_000) }
    private func index(of id: String) -> Int { items.firstIndex { $0.id == id }! }
    private func now() -> String { let f = DateFormatter(); f.dateFormat = "HH:mm"; return f.string(from: Date()) }

    func me() async throws -> UserProfile { view(raw(meId)) }

    func dashboard() async throws -> Dashboard {
        await latency()
        let mine = items.filter { canSee($0, meId) && $0.status == .open }
        return Dashboard(openReports: mine.filter { $0.type == .report }.count, unreadMessages: meId == "u-lakshmi" ? 2 : 1,
                         upcomingAppointments: mine.filter { $0.type == .booking }.count)
    }

    func openItems(limit: Int) async throws -> [DataItem] {
        await latency()
        return Array(items.filter { canSee($0, meId) && $0.status == .open }.sorted { $0.date > $1.date }.prefix(limit))
    }

    func item(_ id: String) async throws -> DataItem {
        await latency()
        var it = items[index(of: id)]
        guard canSee(it, meId) else { throw URLError(.userAuthenticationRequired) }
        if it.ownerId != meId { it.allowedActions = ["read"] }
        return it
    }

    func user(_ id: String) async throws -> UserProfile { await latency(); return view(raw(id)) }

    func sharedItems(with userId: String) async throws -> [DataItem] {
        await latency()
        guard isConnected(meId, userId) else { return [] }
        return items.filter { $0.status == .open && (($0.ownerId == meId && canSee($0, userId)) || ($0.ownerId == userId && canSee($0, meId))) }
    }

    func messages(with userId: String) async throws -> [Message] { await latency(); return chats[key(meId, userId)] ?? [] }

    func send(to userId: String, body: String) async throws -> Message {
        await latency()
        let k = key(meId, userId)
        let m = Message(messageId: UUID().uuidString, threadId: "t-\(k)", senderId: meId, body: body, sentAt: now())
        chats[k, default: []].append(m)
        // Simulate the other person replying over the WebSocket, so live delivery can be tested.
        let other = raw(userId)
        if other.primaryRole != .hospital {
            Task {
                try? await Task.sleep(nanoseconds: 1_500_000_000)
                let reply = Message(messageId: UUID().uuidString, threadId: "t-\(k)", senderId: userId,
                                    body: other.primaryRole == .patient ? "Thank you, doctor." : "Noted — I'll review it today.", sentAt: now())
                chats[k, default: []].append(reply)
                subject.send(.newMessage(reply))
            }
        }
        return m
    }

    func threads() async throws -> [ThreadSummary] {
        await latency()
        return chats.compactMap { k, list in
            let ids = k.split(separator: "|").map(String.init)
            guard ids.contains(meId), let last = list.last, let other = ids.first(where: { $0 != meId }) else { return nil }
            return ThreadSummary(threadId: "t-\(k)", otherUser: view(raw(other)), lastMessage: last.body, lastMessageAt: last.sentAt,
                                 unread: last.senderId == meId ? 0 : 1)
        }
    }

    func search(_ query: String, role: Role?) async throws -> [UserProfile] {
        await latency()
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        return users.filter { $0.id != meId }
            .filter { role == nil || $0.primaryRole == role }
            .filter { q.isEmpty || $0.displayName.lowercased().contains(q) || $0.publicId.lowercased().contains(q) }
            .map(view)
    }

    func connections() async throws -> [UserProfile] { await latency(); return users.filter { $0.id != meId && isConnected(meId, $0.id) }.map(view) }

    func connect(_ userId: String) async throws -> UserProfile { await latency(); links.insert([meId, userId]); return view(raw(userId)) }

    func setStatus(_ itemId: String, _ status: ItemStatus) async throws -> DataItem {
        await latency(); let i = index(of: itemId); items[i].status = status; return items[i]
    }

    func share(_ itemId: String, with granteeId: String) async throws -> DataItem {
        await latency()
        let i = index(of: itemId)
        if !items[i].accessList.contains(where: { $0.granteeId == granteeId }) { items[i].accessList.append(grant(for: raw(granteeId))) }
        return items[i]
    }

    func revoke(_ itemId: String, grantId: String) async throws -> DataItem {
        await latency(); let i = index(of: itemId); items[i].accessList.removeAll { $0.grantId == grantId }; return items[i]
    }

    func upload(_ draft: UploadDraft, files: [PendingAttachment]) async throws -> DataItem {
        await latency(600)
        let me = raw(meId)
        var grants = draft.shareWith.map { grant(for: raw($0)) }
        // Uploading for a patient: the patient owns it; the uploader keeps access (doc 2.3).
        if draft.ownerId != meId && !grants.contains(where: { $0.granteeId == meId }) { grants.append(grant(for: me)) }
        let n = files.count
        let item = DataItem(
            itemId: UUID().uuidString, date: draft.date, coreItemType: draft.type,
            title: draft.title.isEmpty ? draft.type.label : draft.title,
            subtitle: draft.ownerId != meId ? "\(me.displayName) · \(n) file\(n == 1 ? "" : "s")" : "\(n) attachment\(n == 1 ? "" : "s")",
            keywords: draft.keywords,
            coreItemData: files.enumerated().map { i, f in
                Attachment(kind: f.kind, uri: f.localURL.absoluteString, mime: f.mime, size: f.size, position: i, name: f.name)
            },
            links: draft.links, ownerId: draft.ownerId, createdByName: me.displayName, accessList: grants,
            pointerToMessage: draft.pointerToMessage, status: draft.status, allowedActions: ["read", "share", "revoke", "status"])
        items.insert(item, at: 0)
        subject.send(.itemChanged(item.id))
        return item
    }

    func updateProfile(_ update: ProfileUpdate) async throws -> UserProfile {
        await latency()
        let i = users.firstIndex { $0.id == meId }!
        users[i].displayName = update.displayName.trimmingCharacters(in: .whitespaces)
        users[i].location = update.location?.isEmpty == true ? nil : update.location
        return view(users[i])
    }

    func activeShares() async throws -> [ShareGroup] {
        await latency()
        let grants = items.filter { $0.ownerId == meId }.flatMap { item in
            item.accessList.map { OwnedGrant(grantId: $0.grantId, granteeType: $0.granteeType, granteeId: $0.granteeId,
                                             granteeName: $0.granteeName, itemId: item.id, itemTitle: item.title, coreItemType: item.type) }
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

// MARK: - Remote (design doc 4.3)

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
            throw URLError(.badServerResponse, userInfo: [NSLocalizedDescriptionKey: "Server returned \(code)"])
        }
        if T.self == Empty.self { return Empty() as! T }
        return try decoder.decode(T.self, from: data)
    }

    struct Empty: Codable {}
    struct StatusPatch: Encodable { let status: ItemStatus }
    struct SendBody: Encodable { let body: String; let clientMsgId: String }
    struct ConnectBody: Encodable { let userId: String }
    struct GrantBody: Encodable { let itemIds: [String]; let granteeId: String }
    struct DeviceBody: Encodable { let platform: String; let token: String }
    struct PresignBody: Encodable { struct File: Encodable { let name: String; let mime: String; let size: Int64 }; let files: [File] }
    struct PresignResponse: Decodable { struct Upload: Decodable { let uploadUrl: String; let uri: String }; let uploads: [Upload] }
    struct NewItem: Encodable {
        let ownerId: String; let coreItemType: CoreItemType; let title: String; let date: String; let keywords: [String]
        let coreItemData: [Attachment]; let links: [String]; let status: ItemStatus; let shareWith: [String]; let pointerToMessage: String?
    }

    func me() async throws -> UserProfile { try await request("GET", "v1/me") }
    func dashboard() async throws -> Dashboard { try await request("GET", "v1/dashboard") }
    func openItems(limit: Int) async throws -> [DataItem] {
        let page: PageResult<DataItem> = try await request("GET", "v1/items", query: ["status": "OPEN", "limit": String(limit)]); return page.data
    }
    func item(_ id: String) async throws -> DataItem {
        var item: DataItem = try await request("GET", "v1/items/\(id)")
        if let files: PageResult<Attachment> = try? await request("GET", "v1/items/\(id)/attachments") { item.coreItemData = files.data }
        return item
    }
    func user(_ id: String) async throws -> UserProfile { try await request("GET", "v1/users/\(id)") }
    func sharedItems(with userId: String) async throws -> [DataItem] {
        let p: PageResult<DataItem> = try await request("GET", "v1/users/\(userId)/shared-items", query: ["status": "OPEN"]); return p.data
    }
    private func thread(with userId: String) async throws -> ThreadSummary? {
        let p: PageResult<ThreadSummary> = try await request("GET", "v1/threads", query: ["with": userId]); return p.data.first
    }
    func messages(with userId: String) async throws -> [Message] {
        guard let t = try await thread(with: userId) else { return [] }
        let p: PageResult<Message> = try await request("GET", "v1/threads/\(t.id)/messages"); return p.data
    }
    func send(to userId: String, body: String) async throws -> Message {
        guard let t = try await thread(with: userId) else { throw URLError(.fileDoesNotExist) }
        return try await request("POST", "v1/threads/\(t.id)/messages", body: SendBody(body: body, clientMsgId: UUID().uuidString))
    }
    func threads() async throws -> [ThreadSummary] { let p: PageResult<ThreadSummary> = try await request("GET", "v1/threads"); return p.data }
    func search(_ query: String, role: Role?) async throws -> [UserProfile] {
        let p: PageResult<UserProfile> = try await request("GET", "v1/search", query: ["q": query, "type": role?.rawValue]); return p.data
    }
    func connections() async throws -> [UserProfile] { let p: PageResult<UserProfile> = try await request("GET", "v1/connections"); return p.data }
    func connect(_ userId: String) async throws -> UserProfile { try await request("POST", "v1/connections", body: ConnectBody(userId: userId)) }
    func setStatus(_ itemId: String, _ status: ItemStatus) async throws -> DataItem { try await request("PATCH", "v1/items/\(itemId)", body: StatusPatch(status: status)) }
    func share(_ itemId: String, with granteeId: String) async throws -> DataItem {
        let _: Empty = try await request("POST", "v1/grants", body: GrantBody(itemIds: [itemId], granteeId: granteeId)); return try await item(itemId)
    }
    func revoke(_ itemId: String, grantId: String) async throws -> DataItem {
        let _: Empty = try await request("DELETE", "v1/grants/\(grantId)"); return try await item(itemId)
    }
    func updateProfile(_ update: ProfileUpdate) async throws -> UserProfile { try await request("PATCH", "v1/me", body: update) }
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

    /// Presign all files in one call, PUT each to storage, then create the DataItem.
    func upload(_ draft: UploadDraft, files: [PendingAttachment]) async throws -> DataItem {
        let presigned: PresignResponse = try await request("POST", "v1/uploads/presign",
            body: PresignBody(files: files.map { .init(name: $0.name, mime: $0.mime, size: $0.size) }))
        for (file, target) in zip(files, presigned.uploads) {
            var put = URLRequest(url: URL(string: target.uploadUrl)!)
            put.httpMethod = "PUT"
            put.setValue(file.mime, forHTTPHeaderField: "Content-Type")
            let (_, resp) = try await session.upload(for: put, fromFile: file.localURL)
            guard (resp as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) == true else {
                throw URLError(.cannotWriteToFile, userInfo: [NSLocalizedDescriptionKey: "Upload of \(file.name) failed"])
            }
        }
        let attachments = zip(files, presigned.uploads).enumerated().map { i, pair in
            Attachment(kind: pair.0.kind, uri: pair.1.uri, mime: pair.0.mime, size: pair.0.size, position: i, name: pair.0.name)
        }
        return try await request("POST", "v1/items", body: NewItem(
            ownerId: draft.ownerId, coreItemType: draft.type, title: draft.title, date: draft.date, keywords: draft.keywords,
            coreItemData: attachments, links: draft.links, status: draft.status, shareWith: draft.shareWith, pointerToMessage: draft.pointerToMessage))
    }
}

// MARK: - Attachment files

/// Resolves attachment URIs (bundle samples, local files, presigned https URLs) to local files.
enum AttachmentStore {
    static func localURL(for a: Attachment) async throws -> URL {
        if a.uri.hasPrefix("bundle://") {
            let name = String(a.uri.dropFirst("bundle://".count))
            let ext = (name as NSString).pathExtension, stem = (name as NSString).deletingPathExtension
            guard let url = Bundle.main.url(forResource: stem, withExtension: ext) else { throw URLError(.fileDoesNotExist) }
            return url
        }
        guard let remote = URL(string: a.uri) else { throw URLError(.badURL) }
        if remote.isFileURL { return remote }

        let dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("attachments", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let key = SHA256.hash(data: Data(remote.path.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
        let target = dir.appendingPathComponent("\(key)-\(a.name)")
        if FileManager.default.fileExists(atPath: target.path) { return target }
        let (tmp, resp) = try await URLSession.shared.download(from: remote)
        guard (resp as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) == true else { throw URLError(.badServerResponse) }
        try? FileManager.default.removeItem(at: target)
        try FileManager.default.moveItem(at: tmp, to: target)
        return target
    }
}
