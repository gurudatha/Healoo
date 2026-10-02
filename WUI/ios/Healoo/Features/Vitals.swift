import SwiftUI

// Vitals screens (Documentation/Vitals_Design.md): the Home tiles and entry sheet for a user, the
// red / yellow / green alert row and list for a doctor, and a person's full vitals page.

// MARK: - Icons and colours

enum VitalStyle {
    /// Icon keys from "icons" in Server/config/vitals.json → SF Symbols. Unknown keys get "other".
    static func symbol(_ key: String) -> String {
        switch key {
        case "water": "waterbottle"
        case "bp": "heart"
        case "sugar": "hexagon"
        case "heart": "waveform.path.ecg"
        case "urine": "drop"
        case "weight": "scalemass"
        case "temperature": "thermometer.medium"
        case "oxygen": "lungs"
        case "sleep": "moon"
        case "steps": "figure.walk"
        case "pain": "bolt"
        case "breath": "wind"
        case "pill": "pills"
        case "lab": "flask"
        case "mood": "face.smiling"
        case "more": "ellipsis"
        default: "plus.circle"
        }
    }

    /// One shape per colour, so the three aren't told apart by colour alone.
    static func alertSymbol(_ s: Severity) -> String {
        switch s { case .red: "exclamationmark.triangle"; case .yellow: "exclamationmark.circle"; case .green: "checkmark.circle" }
    }

    /// Red, yellow and green as used for readings and the doctor dashboard (readable on white).
    static func color(_ s: Severity?) -> Color {
        switch s {
        case .red: Color(hex: 0xC62828)
        case .yellow: Color(hex: 0xB26A00)
        case .green: Color(hex: 0x2E7D32)
        case nil: Sage.muted                // nothing recorded yet
        }
    }

    static func tint(_ s: Severity?) -> Color {
        switch s {
        case .red: Color(hex: 0xFCE8E6)
        case .yellow: Color(hex: 0xFFF4D6)
        case .green: Color(hex: 0xE4F3E5)
        case nil: Sage.sunken
        }
    }
}

// MARK: - Home: a user's tiles

/// A block of vitals (two rows of five by default; the server's configuration decides). Each tile
/// shows the icon in the colour of the latest reading (grey when nothing is recorded) and that
/// value; tapping it opens the entry sheet. The last tile opens the full list.
struct VitalTileGrid: View {
    let types: [VitalType]
    let latest: [String: VitalReading]
    let settings: VitalSettings
    let onTile: (VitalType) -> Void
    let onMore: () -> Void

    var body: some View {
        let columns = min(max(settings.homeColumns, 2), 6)
        let shown = Array(types.prefix(columns * min(max(settings.homeRows, 1), 4) - 1))
        let cells: [VitalType?] = shown.map { Optional($0) } + [nil]
        let rows = stride(from: 0, to: cells.count, by: columns).map { Array(cells[$0..<min($0 + columns, cells.count)]) }
        VStack(spacing: 0) {
            ForEach(rows.indices, id: \.self) { i in
                HStack(spacing: 0) {
                    ForEach(rows[i].indices, id: \.self) { j in
                        if let t = rows[i][j] {
                            let r = latest[t.code]
                            tile(t.label, VitalStyle.symbol(t.icon), r?.severity, r.map { VitalText.format($0.values, unit: "", order: t.keys) } ?? "–", more: false) { onTile(t) }
                        } else {
                            tile("More", VitalStyle.symbol("more"), nil, types.count > shown.count ? "+\(types.count - shown.count)" : "All", more: true, action: onMore)
                        }
                    }
                    ForEach(0..<(columns - rows[i].count), id: \.self) { _ in Color.clear.frame(maxWidth: .infinity, maxHeight: 1) }
                }
            }
        }
        .padding(.vertical, 4)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
    }

    private func tile(_ label: String, _ symbol: String, _ severity: Severity?, _ value: String, more: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 1) {
                Image(systemName: symbol).font(.system(size: 20)).foregroundStyle(more ? Sage.accent : VitalStyle.color(severity)).frame(height: 24)
                Text(label).font(HFont.tiny).foregroundStyle(Sage.ink).lineLimit(1)
                Text(value).font(.custom(FontName.figtreeRegular, size: 11)).foregroundStyle(VitalStyle.color(severity)).lineLimit(1)
            }
            .frame(maxWidth: .infinity).padding(.vertical, 5).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(label) \(value)" + (severity.map { ", \($0.label.lowercased())" } ?? ""))
    }
}

// MARK: - Home: a doctor's alert row

/// Unread alerts from patients as red, yellow and green — one row, the size of the bottom bar's
/// icons. Tapping a colour lists the patients behind it.
struct AlertDashboardRow: View {
    let counts: VitalAlertCounts?
    var body: some View {
        HStack(spacing: 0) {
            ForEach(Severity.allCases) { s in
                let n = counts?[s]
                // Grey when there is nothing to look at, so colour means "needs attention".
                let color = (n ?? 0) > 0 ? VitalStyle.color(s) : Sage.muted
                NavigationLink(value: Route.vitalAlerts(s.rawValue)) {
                    VStack(spacing: 1) {
                        Image(systemName: VitalStyle.alertSymbol(s)).font(.system(size: 20))
                        Text("\(s.label) · \(n.map(String.init) ?? "–")").font(HFont.tiny).lineLimit(1)
                    }
                    .foregroundStyle(color).frame(maxWidth: .infinity, minHeight: 44).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("\(n ?? 0) \(s.label.lowercased()) alerts")
            }
        }
        .frame(height: 46)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
    }
}

// MARK: - Shared rows

/// A coloured dot, value and time: one reading in a list.
struct ReadingRow: View {
    let type: VitalType?
    let reading: VitalReading
    var showLabel = false

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(VitalStyle.color(reading.severity)).frame(width: 10, height: 10).accessibilityLabel(reading.severity.label)
            if showLabel { Image(systemName: VitalStyle.symbol(type?.icon ?? "other")).foregroundStyle(Sage.muted).frame(width: 20) }
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                if let note = reading.note { Text(note).font(HFont.small).foregroundStyle(Sage.muted).lineLimit(1) }
            }
            Spacer(minLength: 0)
            Text(VitalText.time(reading.takenAt)).font(HFont.small).foregroundStyle(Sage.muted)
        }
        .padding(.horizontal, 12).padding(.vertical, 6).frame(minHeight: 40)
    }

    private var title: String {
        var text = showLabel ? "\(type?.label ?? reading.code) · " : ""
        text += VitalText.format(reading.values, unit: type?.unit ?? "", order: type?.keys ?? [])
        if let pct = type?.ratio?.percent(reading.values) { text += " · \(Int(pct.rounded()))%" }
        return text
    }
}

private func numberField(_ label: String, _ text: Binding<String>) -> some View {
    VStack(alignment: .leading, spacing: 6) {
        FieldLabel(label)
        TextField("", text: text).keyboardType(.decimalPad)
            .font(HFont.body).foregroundStyle(Sage.ink).padding(.horizontal, 14).frame(minHeight: 48)
            .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
    }
}

private func parse(_ s: String?) -> Double? { Double((s ?? "").trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: ".")) }

// MARK: - Entry sheet

/// Enter a value of one vital, for now or for a time up to `backdateHours` ago (in case the person
/// couldn't enter it straight away). Once saved, a reading and its time can't be changed.
struct VitalEntrySheet: View {
    let type: VitalType
    let backdateHours: Int
    /// Called with the saved reading.
    let onSaved: (VitalReading) -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var inputs: [String: String] = [:]
    @State private var note = ""
    @State private var earlier = false
    @State private var at = Date()
    @State private var recent: [VitalReading] = []
    @State private var busy = false
    @State private var error: String?

    private var values: [String: Double] { Dictionary(uniqueKeysWithValues: type.components.compactMap { c in parse(inputs[c.key]).map { (c.key, $0) } }) }
    private var allEntered: Bool { values.count == type.components.count && values.values.allSatisfy { $0 >= 0 } }
    /// Urine: the percentage can't be worked out without some water intake.
    private var ratioMissing: Bool { type.ratio != nil && allEntered && type.ratio?.percent(values) == nil }
    private func name(_ key: String) -> String { type.components.first { $0.key == key }?.label ?? key }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 12) {
                    ForEach(type.components, id: \.key) { c in
                        numberField(type.components.count == 1 ? "Value" + (type.unit.isEmpty ? "" : " (\(type.unit))") : c.label + (type.unit.isEmpty ? "" : " (\(type.unit))"),
                                    Binding(get: { inputs[c.key] ?? "" }, set: { inputs[c.key] = String($0.filter { $0.isNumber || $0 == "." || $0 == "," }.prefix(8)) }))
                    }
                }
                if let r = type.ratio {
                    Group {
                        if let pct = r.percent(values) { Text("\(name(r.numerator)) is \(Int(pct.rounded()))% of \(name(r.denominator).lowercased()).") }
                        else if ratioMissing { Text("\(name(r.denominator)) must be more than 0.").foregroundStyle(Sage.clay) }
                        else { Text("Enter both to compare \(name(r.numerator).lowercased()) with \(name(r.denominator).lowercased()).") }
                    }
                    .font(HFont.caption).foregroundStyle(Sage.muted)
                }
                VStack(alignment: .leading, spacing: 6) {
                    FieldLabel("Time")
                    HStack(spacing: 8) {
                        SageChip(label: "Now", selected: !earlier) { earlier = false }
                        SageChip(label: "Earlier…", selected: earlier) { earlier = true; at = Date() }
                    }
                    if earlier {
                        // The picker itself can't go further back than the limit, or into the future.
                        DatePicker("Measured at", selection: $at, in: Date().addingTimeInterval(-Double(backdateHours) * 3600)...Date(),
                                   displayedComponents: [.date, .hourAndMinute])
                            .font(HFont.body)
                    }
                    Text("Up to \(backdateHours) hours back. The time can't be changed after saving.").font(HFont.small).foregroundStyle(Sage.muted)
                }
                VStack(alignment: .leading, spacing: 6) {
                    FieldLabel("Note (optional)")
                    TextField("", text: $note).font(HFont.body).padding(.horizontal, 14).frame(minHeight: 48)
                        .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
                }
                if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Saving…" : "Save", action: save).buttonStyle(PrimaryButtonStyle()).disabled(busy || !allEntered || ratioMissing)

                if !recent.isEmpty {
                    FieldLabel("Recent entries")
                    GroupCard {
                        ForEach(Array(recent.prefix(5).enumerated()), id: \.element.id) { i, r in
                            if i > 0 { RowDivider() }
                            ReadingRow(type: type, reading: r)
                        }
                    }
                }
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle(type.label).navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .task { recent = (try? await env.repo.vitalReadings(nil, code: type.code, limit: 5)) ?? [] }
    }

    private func save() {
        let text = note.trimmingCharacters(in: .whitespaces)
        let reading = NewVitalReading(code: type.code, values: values, note: text.isEmpty ? nil : String(text.prefix(200)),
                                      takenAt: earlier ? VitalText.iso(min(at, Date())) : nil)
        busy = true; error = nil
        Task {
            do {
                let saved = try await env.repo.recordVital(reading)
                onSaved(saved)
            } catch { self.error = "That didn't save: \(error.localizedDescription). Try again." }
            busy = false
        }
    }
}

// MARK: - A person's vitals

/// Every vital with its latest value and colour, then the history, newest entry first. The person
/// taps a vital to enter a value. Their doctor (`userId` set) instead sees and edits the range of
/// each vital and can add a vital for this patient; ranges are never shown to the patient.
struct VitalsView: View {
    let userId: String?
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var patient: UserProfile?
    @State private var types: [VitalType]?
    @State private var latest: [String: VitalReading] = [:]
    @State private var readings: [VitalReading] = []
    @State private var settings = VitalSettings()
    @State private var filter: String?
    @State private var error: String?
    @State private var entering: VitalType?
    @State private var editing: VitalType?
    @State private var adding = false

    private var asDoctor: Bool { userId != nil }

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: "Vitals", subtitle: asDoctor ? (patient?.displayName ?? "…") : "Your readings", onBack: { dismiss() }) { EmptyView() }
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if let error { ErrorView(message: error) { Task { await load() } } }
                    else if let types {
                        if asDoctor {
                            Text("Ranges are set per patient and are not shown to them. A reading inside the range is green, outside by up to the margin yellow, and beyond that red. Changes apply to new readings.")
                                .font(HFont.caption).foregroundStyle(Sage.sandInk).padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(Sage.sandTint, in: RoundedRectangle(cornerRadius: 12))
                        }
                        GroupCard {
                            ForEach(Array(types.enumerated()), id: \.element.id) { i, t in
                                if i > 0 { RowDivider() }
                                Button { if asDoctor { editing = t } else { entering = t } } label: { vitalRow(t) }.buttonStyle(.plain)
                            }
                        }
                        if asDoctor { Button("Add a vital for this patient") { adding = true }.buttonStyle(SecondaryButtonStyle()) }
                        SectionHeader(title: "History").padding(.top, 6)
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 8) {
                                SageChip(label: "All", selected: filter == nil) { filterBy(nil) }
                                ForEach(types) { t in SageChip(label: t.label, selected: filter == t.code) { filterBy(t.code) } }
                            }
                        }
                        if readings.isEmpty { Text("No readings yet.").font(HFont.body).foregroundStyle(Sage.muted) }
                        else {
                            GroupCard {
                                ForEach(Array(readings.enumerated()), id: \.element.id) { i, r in
                                    if i > 0 { RowDivider() }
                                    ReadingRow(type: types.first { $0.code == r.code }, reading: r, showLabel: filter == nil)
                                }
                            }
                        }
                    } else { LoadingView() }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .toolbar(.hidden, for: .navigationBar)
        .task { await load() }
        .sheet(item: $entering) { t in
            NavigationStack { VitalEntrySheet(type: t, backdateHours: settings.backdateHours) { _ in entering = nil; Task { await load() } } }.environment(env)
        }
        .sheet(item: $editing) { t in
            NavigationStack { RangeSheet(userId: userId ?? "", type: t, settings: settings) { editing = nil; Task { await load() } } }.environment(env)
        }
        .sheet(isPresented: $adding) {
            NavigationStack { AddVitalSheet(userId: userId ?? "", settings: settings) { adding = false; Task { await load() } } }.environment(env)
        }
    }

    private func vitalRow(_ t: VitalType) -> some View {
        let r = latest[t.code]
        return HStack(spacing: 12) {
            Image(systemName: VitalStyle.symbol(t.icon)).foregroundStyle(VitalStyle.color(r?.severity))
                .frame(width: 36, height: 36).background(VitalStyle.tint(r?.severity), in: RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 1) {
                Text(t.label + (t.custom ? " · added by doctor" : "")).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                Text(asDoctor ? rangeText(t) : (r.map { "Last entered \(VitalText.time($0.takenAt))" } ?? "Tap to enter a value"))
                    .font(HFont.small).foregroundStyle(Sage.muted).lineLimit(2).multilineTextAlignment(.leading)
            }
            Spacer(minLength: 0)
            VStack(alignment: .trailing, spacing: 1) {
                Text(r.map { VitalText.format($0.values, unit: t.unit, order: t.keys) } ?? "–").font(HFont.bodyStrong).foregroundStyle(VitalStyle.color(r?.severity))
                Text(asDoctor ? "Set range" : (r?.severity.label ?? "")).font(HFont.small).foregroundStyle(asDoctor ? Sage.accent : VitalStyle.color(r?.severity))
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 8).frame(minHeight: 56).contentShape(Rectangle())
    }

    /// "90–120 / 60–80 mmHg · yellow up to 30% outside", the urine percentage rule, or "No range set".
    private func rangeText(_ t: VitalType) -> String {
        let parts: [String?] = t.components.map { c in
            switch (c.low, c.high) {
            case let (l?, h?): "\(VitalText.number(l))–\(VitalText.number(h))"
            case let (l?, nil): "from \(VitalText.number(l))"
            case let (nil, h?): "up to \(VitalText.number(h))"
            default: nil
            }
        }
        var out: [String] = []
        if let r = t.ratio, let g = r.greenMinPct, let y = r.yellowMinPct {
            let name = { (key: String) in t.components.first { $0.key == key }?.label ?? key }
            out.append("\(name(r.numerator)) \(VitalText.number(g))% or more of \(name(r.denominator).lowercased()) is green, from \(VitalText.number(y))% yellow, less red")
        }
        if parts.contains(where: { $0 != nil }) {
            out.append(parts.map { $0 ?? "any" }.joined(separator: " / ") + " \(t.unit) · yellow up to \(t.marginPct ?? 30)% outside")
        }
        return out.isEmpty ? "No range set: every value is green" : out.joined(separator: " · ")
    }

    private func filterBy(_ code: String?) {
        filter = code
        Task { readings = (try? await env.repo.vitalReadings(userId, code: code, limit: 50)) ?? readings }
    }

    private func load() async {
        error = nil
        do {
            if let userId { patient = try await env.repo.user(userId) }
            settings = (try? await env.repo.vitalSettings()) ?? settings
            types = try await env.repo.vitalTypes(userId)
            latest = Dictionary(try await env.repo.vitalLatest(userId).map { ($0.code, $0) }, uniquingKeysWith: { first, _ in first })
            readings = try await env.repo.vitalReadings(userId, code: filter, limit: 50)
        } catch { self.error = "Couldn't load vitals: \(error.localizedDescription)" }
    }
}

/// Doctor: this patient's range for one vital, and how far outside still counts as yellow.
private struct RangeSheet: View {
    let userId: String
    let type: VitalType
    let settings: VitalSettings
    let onSaved: () -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var low: [String: String] = [:]
    @State private var high: [String: String] = [:]
    @State private var margin = ""
    @State private var busy = false
    @State private var error: String?

    private var bounds: [String: VitalBounds] {
        Dictionary(uniqueKeysWithValues: type.components.map { ($0.key, VitalBounds(low: parse(low[$0.key]), high: parse(high[$0.key]))) })
    }
    private var problem: String? {
        if bounds.values.contains(where: { b in if let l = b.low, let h = b.high { return l > h }; return false }) { return "The low end can't be above the high end." }
        guard let m = Int(margin), (settings.marginPctMin...settings.marginPctMax).contains(m) else { return "The margin must be \(settings.marginPctMin)–\(settings.marginPctMax)%." }
        return nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("Leave an end empty for no limit on that side.").font(HFont.caption).foregroundStyle(Sage.muted)
                ForEach(type.components, id: \.key) { c in
                    let name = type.components.count > 1 ? "\(c.label) " : ""
                    HStack(spacing: 12) {
                        numberField("\(name)from" + (type.unit.isEmpty ? "" : " (\(type.unit))"), Binding(get: { low[c.key] ?? "" }, set: { low[c.key] = $0 }))
                        numberField("\(name)to", Binding(get: { high[c.key] ?? "" }, set: { high[c.key] = $0 }))
                    }
                }
                numberField("Yellow up to this % outside the range (red beyond)", $margin)
                if let message = problem ?? error { Text(message).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Saving…" : "Save range", action: save).buttonStyle(PrimaryButtonStyle()).disabled(busy || problem != nil)
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle("\(type.label) range").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .onAppear {
            for c in type.components { low[c.key] = VitalText.number(c.low); high[c.key] = VitalText.number(c.high) }
            margin = String(type.marginPct ?? settings.yellowMarginPct)
        }
    }

    private func save() {
        busy = true; error = nil
        let update = VitalRangeUpdate(components: bounds, marginPct: Int(margin))
        Task {
            do { _ = try await env.repo.setVitalRange(userId, code: type.code, update); onSaved() }
            catch { self.error = error.localizedDescription }
            busy = false
        }
    }
}

/// Doctor: a new vital for this patient, with an icon from the predefined set and its range.
private struct AddVitalSheet: View {
    let userId: String
    let settings: VitalSettings
    let onAdded: () -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var label = ""
    @State private var icon = "other"
    @State private var unit = ""
    @State private var low = ""
    @State private var high = ""
    @State private var margin = ""
    @State private var busy = false
    @State private var error: String?

    private var problem: String? {
        if let l = parse(low), let h = parse(high), l > h { return "The low end can't be above the high end." }
        guard let m = Int(margin), (settings.marginPctMin...settings.marginPctMax).contains(m) else { return "The margin must be \(settings.marginPctMin)–\(settings.marginPctMax)%." }
        return nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                textField("Name", $label, "e.g. Peak flow")
                FieldLabel("Icon")
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 6), spacing: 8) {
                    ForEach(settings.icons, id: \.self) { k in
                        let on = k == icon
                        Button { icon = k } label: {
                            Image(systemName: VitalStyle.symbol(k)).foregroundStyle(on ? Sage.accent : Sage.muted)
                                .frame(maxWidth: .infinity, minHeight: 44)
                                .background(on ? Sage.sageTint : Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                                .overlay(RoundedRectangle(cornerRadius: 12).stroke(on ? Sage.accent : Sage.border))
                        }
                        .buttonStyle(.plain).accessibilityLabel(k).accessibilityAddTraits(on ? .isSelected : [])
                    }
                }
                textField("Unit (optional)", $unit, "e.g. L/min")
                HStack(spacing: 12) { numberField("Range from", $low); numberField("to", $high) }
                numberField("Yellow up to this % outside the range (red beyond)", $margin)
                if let message = problem ?? error { Text(message).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Adding…" : "Add vital", action: add).buttonStyle(PrimaryButtonStyle())
                    .disabled(busy || problem != nil || !label.contains(where: { $0.isLetter || $0.isNumber }))
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle("Add a vital").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .onAppear { margin = String(settings.yellowMarginPct); icon = settings.icons.last ?? "other" }
    }

    private func textField(_ title: String, _ text: Binding<String>, _ placeholder: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel(title)
            TextField(placeholder, text: text, prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
                .font(HFont.body).foregroundStyle(Sage.ink).padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
        }
    }

    private func add() {
        busy = true; error = nil
        let type = NewVitalType(userId: userId, label: String(label.trimmingCharacters(in: .whitespaces).prefix(30)), icon: icon,
                                unit: String(unit.trimmingCharacters(in: .whitespaces).prefix(12)), low: parse(low), high: parse(high), marginPct: Int(margin))
        Task {
            do { _ = try await env.repo.addVitalType(type); onAdded() }
            catch { self.error = error.localizedDescription }
            busy = false
        }
    }
}

// MARK: - A doctor's alerts

/// A doctor's unread alerts of one colour. Swipe an alert left to remove it from the active list,
/// right to open the patient's profile; tapping the row also opens the profile.
struct VitalAlertsView: View {
    let initial: Severity
    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @Environment(\.dismiss) private var dismiss
    @State private var severity: Severity = .red
    @State private var sort: AlertSort = .time
    @State private var counts = VitalAlertCounts()
    @State private var alerts: [VitalAlert]?
    @State private var error: String?
    @State private var started = false

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: "Patient alerts", subtitle: "\(severity.label) · unread", onBack: { dismiss() }) { EmptyView() }
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    ForEach(Severity.allCases) { s in SageChip(label: "\(s.label) · \(counts[s])", selected: severity == s) { severity = s; alerts = nil; Task { await load() } } }
                }
                HStack(spacing: 8) {
                    Text("Sort").font(HFont.small).foregroundStyle(Sage.muted)
                    ForEach(AlertSort.allCases) { s in SageChip(label: s.label, selected: sort == s) { sort = s; Task { await load() } } }
                }
                Text("Swipe left to remove from the list · swipe right for the patient's profile").font(HFont.small).foregroundStyle(Sage.muted)
            }
            .padding(.horizontal, 20).padding(.top, 12)

            if let error { ErrorView(message: error) { Task { await load() } }; Spacer() }
            else if let alerts {
                if alerts.isEmpty {
                    Text("No unread \(severity.label.lowercased()) alerts.").font(HFont.body).foregroundStyle(Sage.muted).padding(20)
                    Spacer()
                } else {
                    List {
                        ForEach(alerts) { a in
                            Button { router.push(.user(a.patientId, messages: false)) } label: { row(a) }
                                .buttonStyle(.plain)
                                .listRowBackground(Color.clear).listRowSeparator(.hidden)
                                .listRowInsets(EdgeInsets(top: 4, leading: 20, bottom: 4, trailing: 20))
                                .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                    Button("Remove") { remove(a) }.tint(Sage.muted)
                                }
                                .swipeActions(edge: .leading, allowsFullSwipe: true) {
                                    Button("Profile") { router.push(.user(a.patientId, messages: false)) }.tint(Sage.accent)
                                }
                        }
                    }
                    .listStyle(.plain).scrollContentBackground(.hidden)
                }
            } else { LoadingView(); Spacer() }
        }
        .background(Sage.background)
        .toolbar(.hidden, for: .navigationBar)
        .task { if !started { started = true; severity = initial }; await load() }
        // A patient just recorded something: the list fills in by itself.
        .onReceive(env.repo.events) { event in if case .vitalAlert = event { Task { await load() } } }
    }

    private func row(_ a: VitalAlert) -> some View {
        HStack(spacing: 12) {
            Image(systemName: VitalStyle.symbol(a.icon)).foregroundStyle(VitalStyle.color(a.severity))
                .frame(width: 36, height: 36).background(VitalStyle.tint(a.severity), in: RoundedRectangle(cornerRadius: 10))
                .accessibilityLabel(a.severity.label)
            VStack(alignment: .leading, spacing: 1) {
                Text(a.patientName).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                Text("\(a.label) · \(VitalText.format(a.values, unit: a.unit))").font(HFont.caption).foregroundStyle(VitalStyle.color(a.severity)).lineLimit(1)
                Text(VitalText.time(a.takenAt)).font(HFont.small).foregroundStyle(Sage.muted)
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
        .contentShape(Rectangle())
    }

    /// Removes the alert from the active list at once; the server is told in the background.
    private func remove(_ a: VitalAlert) {
        alerts?.removeAll { $0.id == a.id }
        switch a.severity {
        case .red: counts.red = max(0, counts.red - 1)
        case .yellow: counts.yellow = max(0, counts.yellow - 1)
        case .green: counts.green = max(0, counts.green - 1)
        }
        Task { do { try await env.repo.readVitalAlert(a.id) } catch { await load() } }
    }

    private func load() async {
        error = nil
        do {
            let page = try await env.repo.vitalAlerts(severity: severity, sort: sort)
            counts = page.counts; alerts = page.data
        } catch { self.error = "Couldn't load alerts: \(error.localizedDescription)" }
    }
}
