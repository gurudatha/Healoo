import SwiftUI

// Bottom-bar operations per page, read from WUI/page-operations.json (shared with Android and
// bundled as a resource by project.yml). Change operations there, not here.

/// Operation ids the app knows how to perform. Ids in the config that aren't listed are ignored.
enum OpId {
    static let home = "home", search = "search", upload = "upload", messages = "messages", settings = "settings"
    static let connect = "connect", message = "message", history = "history", shareDocument = "share_document"
    static let uploadFor = "upload_for", bookAppointment = "book_appointment", searchDoctors = "search_doctors"
    static let known: Set<String> = [home, search, upload, messages, settings, connect, message, history,
                                     shareDocument, uploadFor, bookAppointment, searchDoctors]
}

/// Which operation list a page uses.
enum PageKind: String { case global, user, doctor, hospital }

struct OperationSpec: Decodable { let label: String; let icon: String; var requires: [String]? }

struct OperationsConfig: Decodable {
    var version = 1
    var maxVisible = 4
    var pageForRole: [String: String] = [:]
    var operations: [String: OperationSpec] = [:]
    var pages: [String: [String]] = [:]
}

struct PageOperation: Identifiable, Hashable { let id: String; let label: String; let icon: String }
struct ResolvedOperations { var visible: [PageOperation] = []; var overflow: [PageOperation] = [] }

/// What `requires` conditions are checked against.
enum OpFact {
    static func of(viewer: UserProfile?, subject: UserProfile?) -> Set<String> {
        var f = Set<String>()
        if let subject { f.insert(subject.connected ? "connected" : "not_connected") }
        if let viewer { f.insert(viewer.isClinical ? "viewer_clinical" : "viewer_patient") }
        return f
    }
}

enum PageOperations {
    static let config: OperationsConfig = {
        guard let url = Bundle.main.url(forResource: "page-operations", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let cfg = try? JSONDecoder().decode(OperationsConfig.self, from: data)
        else { assertionFailure("page-operations.json is missing from the app bundle"); return OperationsConfig() }
        return cfg
    }()

    static func page(for user: UserProfile) -> PageKind {
        PageKind(rawValue: config.pageForRole[user.primaryRole.rawValue] ?? config.pageForRole["*"] ?? "user") ?? .user
    }

    /// The page's operations whose conditions hold, split at maxVisible for the three-dot menu.
    static func resolve(_ page: PageKind, facts: Set<String>) -> ResolvedOperations {
        let ops: [PageOperation] = (config.pages[page.rawValue] ?? []).compactMap { id in
            guard OpId.known.contains(id), let spec = config.operations[id], facts.isSuperset(of: spec.requires ?? []) else { return nil }
            return PageOperation(id: id, label: spec.label, icon: spec.icon)
        }
        let max = Swift.max(config.maxVisible, 1)
        return ops.count <= max ? ResolvedOperations(visible: ops) : ResolvedOperations(visible: Array(ops.prefix(max)), overflow: Array(ops.dropFirst(max)))
    }

    /// Icon keys used in page-operations.json → SF Symbols.
    static func symbol(_ key: String) -> String {
        switch key {
        case "home": "house"
        case "search": "magnifyingglass"
        case "upload": "square.and.arrow.up"
        case "chat": "bubble.left"
        case "settings": "gearshape"
        case "person_add": "person.badge.plus"
        case "history": "clock.arrow.circlepath"
        case "share": "arrowshape.turn.up.right"
        case "event": "calendar.badge.plus"
        case "doctor": "stethoscope"
        default: "circle"
        }
    }
}

/// The bottom bar for a page: at most maxVisible operations, then a three-dot menu with the rest.
struct OperationBar: View {
    let ops: ResolvedOperations
    var selected: String? = nil
    let perform: (String) -> Void

    var body: some View {
        VStack(spacing: 0) {
            Rectangle().fill(Sage.divider).frame(height: 1)
            HStack(spacing: 0) {
                ForEach(ops.visible) { op in
                    Button { perform(op.id) } label: { label(op.label, PageOperations.symbol(op.icon), on: op.id == selected) }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(op.id == selected ? .isSelected : [])
                }
                if !ops.overflow.isEmpty {
                    Menu {
                        ForEach(ops.overflow) { op in
                            Button { perform(op.id) } label: { Label(op.label, systemImage: PageOperations.symbol(op.icon)) }
                        }
                    } label: { label("More", "ellipsis", on: ops.overflow.contains { $0.id == selected }) }
                    .accessibilityLabel("More")
                }
            }
            .frame(height: 56)
        }
        .background(Sage.surface.ignoresSafeArea(edges: .bottom))
    }

    private func label(_ text: String, _ symbol: String, on: Bool) -> some View {
        VStack(spacing: 4) {
            Image(systemName: symbol).font(.system(size: 20))
            Text(text).font(HFont.tiny).lineLimit(1)
        }
        .foregroundStyle(on ? Sage.accent : Sage.muted)
        .frame(maxWidth: .infinity, minHeight: 52)
        .contentShape(Rectangle())
    }
}

// MARK: - Filtered Search Bar

/// A chip above the Filtered Search Bar that narrows the candidates.
struct SearchFilter<T>: Identifiable { let id = UUID(); let label: String; let test: (T) -> Bool }

/// Filtered Search Bar: a text field with optional filter chips and a result list underneath.
/// Candidates are filtered on the device; `remote` (when given) adds server matches once two or
/// more characters are typed. Picking a result calls `onPick` and clears the text.
struct FilteredSearchBar<T: Identifiable, Row: View>: View where T.ID == String {
    let placeholder: String
    let candidates: [T]
    let matches: (T, String) -> Bool
    let onPick: (T) -> Void
    var filters: [SearchFilter<T>] = []
    var exclude: Set<String> = []
    var remote: ((String) async -> [T])? = nil
    var showAllWhenBlank = false
    var maxResults = 8
    var focused: FocusState<Bool>.Binding? = nil
    var emptyText = "No matches."
    @ViewBuilder let row: (T) -> Row

    @State private var query = ""
    @State private var filter: UUID?
    @State private var fetched: [T] = []

    private var results: [T] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        var seen = Set<String>()
        return (candidates + fetched).filter { seen.insert($0.id).inserted }
            .filter { !exclude.contains($0.id) }
            .filter { t in filter.flatMap { id in filters.first { $0.id == id } }?.test(t) ?? true }
            .filter { q.isEmpty || matches($0, q) }
            .prefix(maxResults).map { $0 }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass").foregroundStyle(Sage.muted)
                field
                if !query.isEmpty {
                    Button { query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(Sage.muted) }
                        .accessibilityLabel("Clear search")
                }
            }
            .padding(.horizontal, 14).frame(minHeight: 48)
            .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.field))
            .overlay(RoundedRectangle(cornerRadius: Radius.field).stroke(Sage.border))

            if !filters.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        SageChip(label: "All", selected: filter == nil) { filter = nil }
                        ForEach(filters) { f in SageChip(label: f.label, selected: filter == f.id) { filter = f.id } }
                    }
                }
            }
            if !query.isEmpty || showAllWhenBlank {
                let list = results
                if list.isEmpty { Text(emptyText).font(HFont.caption).foregroundStyle(Sage.muted).padding(.vertical, 4) }
                else {
                    GroupCard {
                        ForEach(Array(list.enumerated()), id: \.element.id) { i, r in
                            if i > 0 { RowDivider() }
                            Button { onPick(r); query = "" } label: { row(r) }.buttonStyle(.plain)
                        }
                    }
                }
            }
        }
        .task(id: query) {
            fetched = []
            let q = query.trimmingCharacters(in: .whitespaces)
            guard let remote, q.count >= 2 else { return }
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            fetched = await remote(q)
        }
    }

    @ViewBuilder private var field: some View {
        let tf = TextField(placeholder, text: $query, prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
            .font(HFont.body).foregroundStyle(Sage.ink).textInputAutocapitalization(.never).autocorrectionDisabled()
        if let focused { tf.focused(focused) } else { tf }
    }
}

/// A person as a Filtered Search Bar result.
struct UserResultRow: View {
    let user: UserProfile
    var trailing: String? = nil
    var body: some View {
        HStack(spacing: 12) {
            Avatar(initials: user.initials, size: 36, photoUrl: user.photoUri)
            VStack(alignment: .leading, spacing: 1) {
                Text(user.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                Text([user.headline, user.publicId].filter { !$0.isEmpty }.joined(separator: " · "))
                    .font(HFont.small).foregroundStyle(Sage.muted).lineLimit(1)
            }
            Spacer(minLength: 0)
            if let trailing { Text(trailing).font(HFont.small).foregroundStyle(Sage.accent) }
        }
        .padding(.horizontal, 12).padding(.vertical, 8).frame(minHeight: 56)
        .contentShape(Rectangle())
    }
}

/// An item as a Filtered Search Bar result (Share a document, Add to an existing item).
struct ItemResultRow: View {
    let item: DataItem
    var body: some View {
        HStack(spacing: 12) {
            TypeTile(type: item.primaryKind, size: 36)
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                Text(item.primaryKind.label + (item.date.isEmpty ? "" : " · \(DateText.short(item.date))"))
                    .font(HFont.small).foregroundStyle(Sage.muted).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .contentShape(Rectangle())
    }
}

/// A selected value shown in place of a Filtered Search Bar, with a way to change it.
struct PickedRow<Content: View>: View {
    @ViewBuilder let content: () -> Content
    let onChange: () -> Void
    var body: some View {
        HStack(spacing: 0) {
            content()
            Button("Change", action: onChange).font(HFont.captionStrong).foregroundStyle(Sage.accent).padding(.horizontal, 12)
        }
        .background(Sage.sageTint, in: RoundedRectangle(cornerRadius: Radius.card))
    }
}

private let recipientFilters: [SearchFilter<UserProfile>] = [
    SearchFilter(label: "Doctors") { $0.primaryRole == .doctor },
    SearchFilter(label: "Hospitals") { $0.primaryRole == .hospital },
    SearchFilter(label: "Labs") { $0.primaryRole == .lab },
    SearchFilter(label: "Users") { $0.primaryRole == .patient },
]

/// Who something goes to: `fixed` is the person whose page this started from (always included,
/// can't be removed); more people are added with the Filtered Search Bar. `doctorsOnly` limits
/// the extra people to doctors (when passing on something the viewer doesn't own).
struct RecipientField: View {
    let fixed: UserProfile?
    @Binding var extras: [UserProfile]
    let contacts: [UserProfile]
    var doctorsOnly = false
    var exclude: Set<String> = []
    var label = "To"
    @Environment(AppEnvironment.self) private var env

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel(label)
            if fixed != nil || !extras.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        if let fixed { chip(fixed, removable: false) }
                        ForEach(extras) { u in chip(u, removable: true) }
                    }
                }
            }
            let allowed: (UserProfile) -> Bool = { !doctorsOnly || $0.primaryRole == .doctor }
            let repo = env.repo
            let role: Role? = doctorsOnly ? .doctor : nil
            FilteredSearchBar(
                placeholder: doctorsOnly ? "Add a doctor by name or Healoo ID" : "Add someone by name or Healoo ID",
                candidates: contacts.filter(allowed),
                matches: { $0.matches($1) },
                onPick: { extras.append($0) },
                filters: doctorsOnly ? [] : recipientFilters,
                exclude: exclude.union(extras.map(\.id)).union([fixed?.id].compactMap { $0 }),
                remote: { q in ((try? await repo.search(q, role: role)) ?? []).filter(allowed) },
                emptyText: doctorsOnly ? "No doctor matches." : "No one matches."
            ) { UserResultRow(user: $0, trailing: "Add") }
        }
    }

    private func chip(_ u: UserProfile, removable: Bool) -> some View {
        HStack(spacing: 6) {
            Avatar(initials: u.initials, size: 26, photoUrl: u.photoUri)
            Text(u.displayName).font(HFont.captionStrong).foregroundStyle(Sage.ink).lineLimit(1)
            if removable {
                Button { extras.removeAll { $0.id == u.id } } label: { Image(systemName: "xmark").font(.system(size: 12, weight: .semibold)).foregroundStyle(Sage.muted) }
                    .frame(width: 28, height: 28).accessibilityLabel("Remove \(u.displayName)")
            } else {
                Image(systemName: "lock.fill").font(.system(size: 11)).foregroundStyle(Sage.accent).accessibilityLabel("Always included")
            }
        }
        .padding(.leading, 6).padding(.trailing, removable ? 4 : 12).frame(minHeight: 36)
        .background(removable ? Sage.surface : Sage.sageTint, in: Capsule())
        .overlay(Capsule().stroke(removable ? Sage.border : Sage.sageTint))
    }
}
