import Foundation

// Vitals (Documentation/Vitals_Design.md). Not DataItems: small, frequent, per-person
// measurements with their own tables and API (/v1/vitals/...).
//
// Compatibility: unknown fields from a newer server are ignored, every field an older server
// might not send has a default (decodeIfPresent), and the colour is read from plain text so an
// unknown colour can't fail decoding. The catalogue, default ranges and colour percentages come
// from the server (Server/config/vitals.json), never from the app.

/// Colour of a reading against the person's range. Unknown text reads as `.green`.
enum Severity: String, CaseIterable, Identifiable {
    case red = "RED", yellow = "YELLOW", green = "GREEN"
    var id: String { rawValue }
    var label: String { rawValue.capitalized }
    static func of(_ text: String?) -> Severity { text.flatMap(Severity.init(rawValue:)) ?? .green }
}

/// One number the person enters; most vitals have a single component called "value".
struct VitalComponent: Codable, Hashable {
    var key: String
    var label: String = ""
    var low: Double?                    // sent to the person's doctor only
    var high: Double?

    init(key: String, label: String = "", low: Double? = nil, high: Double? = nil) { self.key = key; self.label = label; self.low = low; self.high = high }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        key = try c.decode(String.self, forKey: .key)
        label = try c.decodeIfPresent(String.self, forKey: .label) ?? ""
        low = try c.decodeIfPresent(Double.self, forKey: .low)
        high = try c.decodeIfPresent(Double.self, forKey: .high)
    }
}

/// One value as a percentage of another (urine output against water intake). Thresholds go to doctors only.
struct VitalRatio: Codable, Hashable {
    var numerator: String
    var denominator: String
    var greenMinPct: Double?
    var yellowMinPct: Double?

    /// The numerator as a percentage of the denominator; nil when it can't be worked out.
    func percent(_ values: [String: Double]) -> Double? {
        guard let n = values[numerator], let d = values[denominator], d > 0 else { return nil }
        return n / d * 100
    }
}

/// A vital as one person has it: built-in, or added for them by a doctor.
struct VitalType: Codable, Hashable, Identifiable {
    var code: String
    var label = ""
    var icon = "other"
    var unit = ""
    var components = [VitalComponent(key: "value")]
    var marginPct: Int?
    var custom = false
    var position = 0
    /// Set for vitals coloured by one value as a percentage of another (urine).
    var ratio: VitalRatio?
    var id: String { code }
    var keys: [String] { components.map(\.key) }

    init(code: String, label: String, icon: String, unit: String, components: [VitalComponent], marginPct: Int? = nil, custom: Bool = false, position: Int = 0, ratio: VitalRatio? = nil) {
        self.code = code; self.label = label; self.icon = icon; self.unit = unit; self.components = components
        self.marginPct = marginPct; self.custom = custom; self.position = position; self.ratio = ratio
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        code = try c.decode(String.self, forKey: .code)
        label = try c.decodeIfPresent(String.self, forKey: .label) ?? ""
        icon = try c.decodeIfPresent(String.self, forKey: .icon) ?? "other"
        unit = try c.decodeIfPresent(String.self, forKey: .unit) ?? ""
        components = try c.decodeIfPresent([VitalComponent].self, forKey: .components) ?? [VitalComponent(key: "value")]
        marginPct = try c.decodeIfPresent(Int.self, forKey: .marginPct)
        custom = try c.decodeIfPresent(Bool.self, forKey: .custom) ?? false
        position = try c.decodeIfPresent(Int.self, forKey: .position) ?? 0
        ratio = try c.decodeIfPresent(VitalRatio.self, forKey: .ratio)
    }
}

struct VitalReading: Codable, Hashable, Identifiable {
    var readingId: String
    var userId = ""
    var code: String
    var values: [String: Double] = [:]
    var takenAt = ""
    var note: String?
    var severityText = "GREEN"
    var id: String { readingId }
    var severity: Severity { .of(severityText) }

    enum CodingKeys: String, CodingKey { case readingId, userId, code, values, takenAt, note, severityText = "severity" }

    init(readingId: String, userId: String, code: String, values: [String: Double], takenAt: String, note: String?, severity: Severity) {
        self.readingId = readingId; self.userId = userId; self.code = code; self.values = values
        self.takenAt = takenAt; self.note = note; self.severityText = severity.rawValue
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        readingId = try c.decode(String.self, forKey: .readingId)
        userId = try c.decodeIfPresent(String.self, forKey: .userId) ?? ""
        code = try c.decode(String.self, forKey: .code)
        values = try c.decodeIfPresent([String: Double].self, forKey: .values) ?? [:]
        takenAt = try c.decodeIfPresent(String.self, forKey: .takenAt) ?? ""
        note = try c.decodeIfPresent(String.self, forKey: .note)
        severityText = try c.decodeIfPresent(String.self, forKey: .severityText) ?? "GREEN"
    }
}

/// One entry in a doctor's inbox: a patient's reading, with the doctor's own read state.
struct VitalAlert: Codable, Hashable, Identifiable {
    var alertId: String
    var patientId: String
    var patientName = ""
    var code = ""
    var label = ""
    var icon = "other"
    var unit = ""
    var values: [String: Double] = [:]
    var severityText = "GREEN"
    var takenAt = ""
    var id: String { alertId }
    var severity: Severity { .of(severityText) }

    enum CodingKeys: String, CodingKey { case alertId, patientId, patientName, code, label, icon, unit, values, severityText = "severity", takenAt }

    init(alertId: String, patientId: String, patientName: String, type: VitalType, reading: VitalReading) {
        self.alertId = alertId; self.patientId = patientId; self.patientName = patientName
        code = type.code; label = type.label; icon = type.icon; unit = type.unit
        values = reading.values; severityText = reading.severityText; takenAt = reading.takenAt
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        alertId = try c.decode(String.self, forKey: .alertId)
        patientId = try c.decode(String.self, forKey: .patientId)
        patientName = try c.decodeIfPresent(String.self, forKey: .patientName) ?? ""
        code = try c.decodeIfPresent(String.self, forKey: .code) ?? ""
        label = try c.decodeIfPresent(String.self, forKey: .label) ?? ""
        icon = try c.decodeIfPresent(String.self, forKey: .icon) ?? "other"
        unit = try c.decodeIfPresent(String.self, forKey: .unit) ?? ""
        values = try c.decodeIfPresent([String: Double].self, forKey: .values) ?? [:]
        severityText = try c.decodeIfPresent(String.self, forKey: .severityText) ?? "GREEN"
        takenAt = try c.decodeIfPresent(String.self, forKey: .takenAt) ?? ""
    }
}

struct VitalAlertCounts: Codable, Hashable {
    var red = 0, yellow = 0, green = 0
    subscript(s: Severity) -> Int { switch s { case .red: red; case .yellow: yellow; case .green: green } }

    init(red: Int = 0, yellow: Int = 0, green: Int = 0) { self.red = red; self.yellow = yellow; self.green = green }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        red = try c.decodeIfPresent(Int.self, forKey: .red) ?? 0
        yellow = try c.decodeIfPresent(Int.self, forKey: .yellow) ?? 0
        green = try c.decodeIfPresent(Int.self, forKey: .green) ?? 0
    }
}

struct VitalAlertPage: Codable {
    var counts = VitalAlertCounts()
    var data: [VitalAlert] = []

    init(counts: VitalAlertCounts = VitalAlertCounts(), data: [VitalAlert] = []) { self.counts = counts; self.data = data }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        counts = try c.decodeIfPresent(VitalAlertCounts.self, forKey: .counts) ?? VitalAlertCounts()
        data = try c.decodeIfPresent([VitalAlert].self, forKey: .data) ?? []
    }
}

/// What the apps need from the server's vitals configuration (GET /v1/vitals/config).
struct VitalSettings: Codable {
    var icons = ["other"]
    var yellowMarginPct = 30
    var marginPctMin = 0
    var marginPctMax = 200
    /// The vitals block on Home: tiles per row and rows (the last tile is More).
    var homeColumns = 5
    var homeRows = 2
    /// How long ago a reading may have been taken when it is entered.
    var backdateHours = 12

    init(icons: [String] = ["other"]) { self.icons = icons }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        icons = try c.decodeIfPresent([String].self, forKey: .icons) ?? ["other"]
        yellowMarginPct = try c.decodeIfPresent(Int.self, forKey: .yellowMarginPct) ?? 30
        marginPctMin = try c.decodeIfPresent(Int.self, forKey: .marginPctMin) ?? 0
        marginPctMax = try c.decodeIfPresent(Int.self, forKey: .marginPctMax) ?? 200
        homeColumns = try c.decodeIfPresent(Int.self, forKey: .homeColumns) ?? 5
        homeRows = try c.decodeIfPresent(Int.self, forKey: .homeRows) ?? 2
        backdateHours = try c.decodeIfPresent(Int.self, forKey: .backdateHours) ?? 12
    }
}

/// `takenAt`: when it was measured (ISO time); now if nil. It can't be changed after saving.
struct NewVitalReading: Encodable { var code: String; var values: [String: Double]; var note: String?; var takenAt: String? }
struct VitalBounds: Encodable { var low: Double?; var high: Double? }
/// PUT /v1/vitals/ranges/{user}/{code}: a doctor's range for one patient.
struct VitalRangeUpdate: Encodable { var components: [String: VitalBounds]; var marginPct: Int? }
/// POST /v1/vitals/types: a doctor adds a vital for one patient.
struct NewVitalType: Encodable {
    var userId: String
    var label: String
    var icon: String
    var unit = ""
    var low: Double?
    var high: Double?
    var marginPct: Int?
}

/// How doctor alerts can be ordered.
enum AlertSort: String, CaseIterable, Identifiable {
    case time, user, vital
    var id: String { rawValue }
    var label: String { switch self { case .time: "Newest"; case .user: "Patient"; case .vital: "Vital" } }
}

enum VitalText {
    /// "120/80 mmHg", "98.6 °F": values in the vital's component order, without a trailing ".0".
    static func format(_ values: [String: Double], unit: String, order: [String] = []) -> String {
        var rest = values.keys.filter { !order.contains($0) }.sorted()
        // Blood pressure reads systolic/diastolic even when the type isn't at hand.
        if rest.contains("sys"), rest.contains("dia") { rest = ["sys", "dia"] + rest.filter { $0 != "sys" && $0 != "dia" } }
        let keys = order.filter { values[$0] != nil } + rest
        let text = keys.compactMap { values[$0] }.map(number).joined(separator: "/")
        return unit.isEmpty ? text : "\(text) \(unit)"
    }

    static func number(_ v: Double?) -> String {
        guard let v else { return "" }
        return v == v.rounded() ? String(Int(v)) : String(format: "%.1f", v)
    }

    /// An ISO time from the server, with or without fractional seconds.
    static func date(_ iso: String) -> Date? {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let d = f.date(from: iso) { return d }
        f.formatOptions = [.withInternetDateTime]
        return f.date(from: iso)
    }

    /// The form the server expects for `taken_at`.
    static func iso(_ date: Date) -> String { ISO8601DateFormatter().string(from: date) }

    /// "2 Oct 08:15" in the phone's time zone.
    static func time(_ iso: String) -> String {
        guard let date = date(iso) else { return String(iso.prefix(16)).replacingOccurrences(of: "T", with: " ") }
        let out = DateFormatter()
        out.dateFormat = "d MMM HH:mm"
        return out.string(from: date)
    }
}
