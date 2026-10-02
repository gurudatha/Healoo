import Foundation

// App settings from WUI/app-config.json (shared with Android, bundled as a resource by
// project.yml), and the offline cache that uses them.

/// Every setting has a default, so a missing value or a missing file changes nothing.
struct AppConfig: Decodable {
    struct MessageCache: Decodable {
        var maxMb = 40
        var trimMb = 2
        init() {}
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            maxMb = try c.decodeIfPresent(Int.self, forKey: .maxMb) ?? 40
            trimMb = try c.decodeIfPresent(Int.self, forKey: .trimMb) ?? 2
        }
        enum CodingKeys: String, CodingKey { case maxMb, trimMb }
    }
    struct FileCache: Decodable {
        var maxFiles = 15
        var maxMb = 90
        init() {}
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            maxFiles = try c.decodeIfPresent(Int.self, forKey: .maxFiles) ?? 15
            maxMb = try c.decodeIfPresent(Int.self, forKey: .maxMb) ?? 90
        }
        enum CodingKeys: String, CodingKey { case maxFiles, maxMb }
    }

    /// Share of the screen height each header takes (design doc 3.2).
    struct Layout: Decodable {
        var homeHeader: CGFloat = 0.20
        var personHeaderExpanded: CGFloat = 0.25
        var personHeaderCollapsed: CGFloat = 0.10
        var pinnedHeader: CGFloat = 0.10
        init() {}
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            var read = Layout()
            read.homeHeader = try c.decodeIfPresent(CGFloat.self, forKey: .homeHeader) ?? 0.20
            read.personHeaderExpanded = try c.decodeIfPresent(CGFloat.self, forKey: .personHeaderExpanded) ?? 0.25
            read.personHeaderCollapsed = try c.decodeIfPresent(CGFloat.self, forKey: .personHeaderCollapsed) ?? 0.10
            read.pinnedHeader = try c.decodeIfPresent(CGFloat.self, forKey: .pinnedHeader) ?? 0.10
            // Values that would break the screens fall back to the defaults.
            let all = [read.homeHeader, read.personHeaderExpanded, read.personHeaderCollapsed, read.pinnedHeader]
            self = all.allSatisfy { (0.05...0.6).contains($0) } && read.personHeaderExpanded >= read.personHeaderCollapsed ? read : Layout()
        }
        enum CodingKeys: String, CodingKey { case homeHeader, personHeaderExpanded, personHeaderCollapsed, pinnedHeader }
    }
    struct Lists: Decodable {
        var homePreview = 20
        var homeAll = 100
        init() {}
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            homePreview = min(max(try c.decodeIfPresent(Int.self, forKey: .homePreview) ?? 20, 1), 100)
            homeAll = min(max(try c.decodeIfPresent(Int.self, forKey: .homeAll) ?? 100, homePreview), 100)
        }
        enum CodingKeys: String, CodingKey { case homePreview, homeAll }
    }
    struct ProfilePhoto: Decodable {
        var maxPx = 512
        init() {}
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            maxPx = min(max(try c.decodeIfPresent(Int.self, forKey: .maxPx) ?? 512, 64), 4096)
        }
        enum CodingKeys: String, CodingKey { case maxPx }
    }

    var layout = Layout()
    var lists = Lists()
    var profilePhoto = ProfilePhoto()
    /// Choices offered when an administrator adds a doctor; any other can be typed.
    var designations = ["Cardiologist", "Urologist", "Paediatrician", "General Medicine"]
    var messageCache = MessageCache()
    var fileCache = FileCache()

    init() {}
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        layout = try c.decodeIfPresent(Layout.self, forKey: .layout) ?? Layout()
        lists = try c.decodeIfPresent(Lists.self, forKey: .lists) ?? Lists()
        profilePhoto = try c.decodeIfPresent(ProfilePhoto.self, forKey: .profilePhoto) ?? ProfilePhoto()
        designations = try c.decodeIfPresent([String].self, forKey: .designations) ?? designations
        messageCache = try c.decodeIfPresent(MessageCache.self, forKey: .messageCache) ?? MessageCache()
        fileCache = try c.decodeIfPresent(FileCache.self, forKey: .fileCache) ?? FileCache()
    }
    enum CodingKeys: String, CodingKey { case layout, lists, profilePhoto, designations, messageCache, fileCache }

    static let current: AppConfig = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        guard let url = Bundle.main.url(forResource: "app-config", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let cfg = try? decoder.decode(AppConfig.self, from: data) else { return AppConfig() }
        return cfg
    }()
}

/// What the Messages screens need, kept on the phone so discussions can be read without the
/// server: the user's profile, the conversation list, and for every opened discussion its item and
/// messages. Each is one JSON file dated with its newest content.
///
/// At most `maxBytes` in total (message_cache.max_mb). When a write goes over, the files with the
/// oldest dates are removed until about `trimBytes` (message_cache.trim_mb) is free again. That
/// is deliberately simple rather than efficient. The file just written is never removed.
///
/// It is only a copy: the server stays the source of truth, and nothing here is sent anywhere.
final class OfflineCache {
    static let me = "me", conversations = "conversations"
    static func item(_ id: String) -> String { "item-\(id)" }
    static func messages(_ itemId: String) -> String { "messages-\(itemId)" }

    private let dir: URL
    private let maxBytes: Int
    private let trimBytes: Int
    private let lock = NSLock()
    private let fm = FileManager.default

    init(dir: URL, maxBytes: Int, trimBytes: Int) { self.dir = dir; self.maxBytes = maxBytes; self.trimBytes = trimBytes }

    convenience init(config: AppConfig.MessageCache = AppConfig.current.messageCache) {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        self.init(dir: base.appendingPathComponent("offline", isDirectory: true), maxBytes: config.maxMb * 1024 * 1024, trimBytes: config.trimMb * 1024 * 1024)
    }

    private func file(_ key: String) -> URL {
        let safe = String(key.map { $0.isLetter || $0.isNumber || "._-".contains($0) ? $0 : "_" })
        return dir.appendingPathComponent(safe + ".json")
    }

    /// Stores `value` under `key`; `date` is what trimming goes by.
    func put<T: Encodable>(_ key: String, _ value: T, date: Date = Date()) {
        lock.lock(); defer { lock.unlock() }
        guard let data = try? JSONEncoder().encode(value) else { return }
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = file(key)
        // Protected until the phone is first unlocked, like the rest of the app's data.
        guard (try? data.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])) != nil else { return }
        try? fm.setAttributes([.modificationDate: date], ofItemAtPath: url.path)
        trim(keep: url)
    }

    /// The stored value, or nil when there is none or it can no longer be read.
    func get<T: Decodable>(_ key: String, as type: T.Type = T.self) -> T? {
        lock.lock(); defer { lock.unlock() }
        guard let data = try? Data(contentsOf: file(key)) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }

    func clear() {
        lock.lock(); defer { lock.unlock() }
        try? fm.removeItem(at: dir)
    }

    private func entries() -> [(url: URL, size: Int, date: Date)] {
        let urls = (try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.fileSizeKey, .contentModificationDateKey])) ?? []
        return urls.compactMap { u in
            guard let v = try? u.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey]) else { return nil }
            return (u, v.fileSize ?? 0, v.contentModificationDate ?? .distantPast)
        }
    }

    /// Over the limit: delete the oldest-dated files until `trimBytes` is free (and it fits).
    private func trim(keep: URL) {
        let all = entries()
        var total = all.reduce(0) { $0 + $1.size }
        guard total > maxBytes else { return }
        let target = max(maxBytes - trimBytes, 0)
        for e in all.filter({ $0.url.lastPathComponent != keep.lastPathComponent }).sorted(by: { $0.date < $1.date }) {
            if total <= target { break }
            total -= e.size
            try? fm.removeItem(at: e.url)
        }
    }

    /// The date of a discussion: when its newest message was sent (now if there are none).
    static func date(of messages: [Message]) -> Date { messages.compactMap { VitalText.date($0.sentAt) }.max() ?? Date() }
}
