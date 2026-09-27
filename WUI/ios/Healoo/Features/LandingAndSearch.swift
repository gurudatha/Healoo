import SwiftUI

// MARK: - Landing (20% dashboard header, search, open items)

struct LandingView: View {
    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @State private var me: UserProfile?
    @State private var dashboard: Dashboard?
    @State private var items: [DataItem]?
    @State private var error: String?
    @State private var query = ""
    @State private var role: Role? = .doctor
    /// Dashboard filter: a PartKind (REPORT, MESSAGE or APPOINTMENT), or nil for all open items.
    @State private var filter: String?
    /// "See all": every open item instead of the latest `preview`.
    @State private var showAll = false
    private let preview = 20, all = 100

    var body: some View {
        VStack(spacing: 0) {
            header
            ScrollView {
                LazyVStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 10) {
                        FieldLabel("Find doctors, patients, hospitals or labs")
                        SearchField(text: $query, placeholder: "Search by name or Healoo ID", onSubmit: { openSearch(role) }, compact: true)
                        RoleFilterRow(selected: role, includeAll: true, compact: true) { role = $0; openSearch($0) }
                    }
                    .padding(.bottom, 6)
                    // A fine line between Find and Open items, drawn inside the gap that was already there.
                    .overlay(alignment: .bottom) { Rectangle().fill(Sage.border).frame(height: 1).offset(y: 3) }

                    // "See all" expands the list in place (it used to open Search, which lists people, not items).
                    let more = showAll || (items?.count ?? 0) >= preview
                    SectionHeader(title: filterTitle, action: more ? (showAll ? "Show fewer" : "See all") : nil) {
                        showAll.toggle(); Task { await load() }
                    }
                    if let f = filter {
                        Button("Clear filter") { toggleFilter(f) }
                            .font(HFont.small).foregroundStyle(Sage.accent)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    if let error {
                        ErrorView(message: error) { Task { await load() } }
                    } else if let items {
                        if items.isEmpty {
                            Text(filter == nil ? "Nothing open right now. New reports, messages and bookings will appear here." : "No open items with \(filterNoun).")
                                .font(HFont.body).foregroundStyle(Sage.muted).padding(.vertical, 16)
                        }
                        ForEach(items) { item in
                            NavigationLink(value: Route.item(item.id)) { DataItemRow(item: item) }.buttonStyle(.plain)
                        }
                    } else {
                        LoadingView()
                    }
                }
                .padding(.horizontal, 20).padding(.vertical, 16)
            }
            .refreshable { await load() }
        }
        .background(Sage.background)
        .task { if items == nil { await load() } }
        // Keep counters and the list current when items or messages arrive live.
        .onReceive(env.repo.events) { event in
            if case .connection = event { return }
            Task { await load() }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(Date(), format: .dateTime.weekday(.wide).day().month(.abbreviated))
                        .font(HFont.caption).foregroundStyle(Sage.onPrimarySoft)
                    Text("\(greeting), \(me?.displayName.split(separator: " ").first.map(String.init) ?? "")")
                        .font(HFont.greeting).foregroundStyle(Sage.onPrimary).lineLimit(1)
                }
                Spacer()
                Button { router.openTab(.settings) } label: { Avatar(initials: me?.initials ?? "", ring: Sage.onPrimaryLine, photoUrl: me?.photoUri) }
                    .accessibilityLabel("Open profile and settings")
            }
            Spacer(minLength: 8)
            HStack(spacing: 8) {
                counter(dashboard?.openReports, "Open reports", kind: PartKind.report)
                counter(dashboard?.unreadMessages, "Unread messages", kind: PartKind.message)
                counter(dashboard?.upcomingAppointments, "Appointments", kind: PartKind.appointment)
            }
        }
        .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 16)
        .heightFraction(HeaderRatio.landing)
        .sageHeaderBackground()
    }

    private var greeting: String {
        switch Calendar.current.component(.hour, from: Date()) { case 0..<12: "Good morning"; case 12..<17: "Good afternoon"; default: "Good evening" }
    }

    /// A dashboard tile; tapping it filters the open items. The selected tile turns light, and
    /// tapping it again clears the filter.
    private func counter(_ value: Int?, _ label: String, kind: String) -> some View {
        let selected = filter == kind
        return Button { toggleFilter(kind) } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(value.map(String.init) ?? "–").font(HFont.counter).foregroundStyle(selected ? Sage.accent : Sage.onPrimary)
                Text(label).font(HFont.small).foregroundStyle(selected ? Sage.ink : Sage.onPrimarySoft).lineLimit(1).minimumScaleFactor(0.85)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 12).padding(.vertical, 10)
            .background(selected ? Color.white : Sage.primaryRaised, in: RoundedRectangle(cornerRadius: 14))
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }

    private func toggleFilter(_ kind: String) {
        filter = filter == kind ? nil : kind
        items = nil
        Task { await load() }
    }

    private var filterTitle: String {
        switch filter {
        case PartKind.report: "Open items · Reports"
        case PartKind.message: "Open items · Messages"
        case PartKind.appointment: "Open items · Appointments"
        default: "Open items"
        }
    }

    private var filterNoun: String {
        switch filter { case PartKind.report: "reports"; case PartKind.message: "messages"; default: "appointments" }
    }

    private func openSearch(_ role: Role?) { router.home.append(Route.search(query: query, role: role)) }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            dashboard = try await env.repo.dashboard()
            items = try await env.repo.openItems(limit: showAll ? all : preview, kind: filter)
        } catch { self.error = "Couldn't load your items. Check your connection and try again." }
    }
}

// MARK: - Shared search pieces

struct SearchField<Trailing: View>: View {
    @Binding var text: String
    let placeholder: String
    var onSubmit: () -> Void = {}
    /// 34 pt high instead of 48 (30% smaller), for the landing page.
    var compact = false
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(spacing: compact ? 8 : 10) {
            Image(systemName: "magnifyingglass").font(compact ? .footnote : .body).foregroundStyle(Sage.muted).accessibilityHidden(true)
            TextField(placeholder, text: $text, prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
                .font(compact ? HFont.caption : HFont.body).foregroundStyle(Sage.ink)
                .submitLabel(.search).onSubmit(onSubmit)
                .autocorrectionDisabled().textInputAutocapitalization(.never)
            trailing()
        }
        .padding(.leading, compact ? 12 : 14).padding(.trailing, 8).frame(minHeight: compact ? 34 : 48)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.field))
        .overlay(RoundedRectangle(cornerRadius: Radius.field).stroke(Sage.border, lineWidth: 1))
    }
}

extension SearchField where Trailing == EmptyView {
    init(text: Binding<String>, placeholder: String, onSubmit: @escaping () -> Void = {}, compact: Bool = false) {
        self.init(text: text, placeholder: placeholder, onSubmit: onSubmit, compact: compact) { EmptyView() }
    }
}

struct RoleFilterRow: View {
    let selected: Role?
    var includeAll = true
    var onHeader = false
    var compact = false
    let onSelect: (Role?) -> Void
    private let filters: [(Role?, String)] = [(nil, "All"), (.doctor, "Doctors"), (.patient, "Users"), (.hospital, "Hospitals"), (.lab, "Labs")]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: compact ? 6 : 8) {
                ForEach(filters.filter { includeAll || $0.0 != nil }, id: \.1) { filter in
                    SageChip(label: filter.1, selected: selected == filter.0, onHeader: onHeader, compact: compact) { onSelect(filter.0) }
                }
            }
        }
    }
}

// MARK: - Search (profiles only; Add to contacts)

struct SearchView: View {
    @Environment(AppEnvironment.self) private var env
    @State private var query: String
    @State private var role: Role?
    @State private var results: [UserProfile]?
    @State private var adding: Set<String> = []
    @State private var error: String?
    @State private var showScanner = false
    @State private var scanNote: String?

    init(initialQuery: String, initialRole: Role?) {
        _query = State(initialValue: initialQuery)
        _role = State(initialValue: initialRole)
    }

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 12) {
                Text("Search").font(HFont.screenTitle).foregroundStyle(Sage.onPrimary)
                SearchField(text: $query, placeholder: "Healoo ID or name") {
                    Button { showScanner = true } label: {
                        Image(systemName: "qrcode.viewfinder").foregroundStyle(Sage.accent)
                            .frame(width: 36, height: 36).background(Sage.sageTint, in: RoundedRectangle(cornerRadius: 10))
                    }
                    .accessibilityLabel("Scan Healoo ID QR code")
                }
                RoleFilterRow(selected: role, onHeader: true) { role = $0 }
                if let scanNote { Text(scanNote).font(HFont.small).foregroundStyle(Sage.onPrimarySoft) }
            }
            .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 16)
            .sageHeaderBackground()

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 8) {
                    HStack(alignment: .top, spacing: 10) {
                        Image(systemName: "shield").foregroundStyle(Sage.sandInk).accessibilityHidden(true)
                        Text("You see profiles only. Health data opens after you connect and the owner shares it.")
                            .font(HFont.caption).foregroundStyle(Sage.sandInk)
                    }
                    .padding(12).frame(maxWidth: .infinity, alignment: .leading)
                    .background(Sage.sandTint, in: RoundedRectangle(cornerRadius: 12))

                    if let error {
                        ErrorView(message: error) { Task { await run(debounce: false) } }
                    } else if let results {
                        if results.isEmpty {
                            Text("No one matches \"\(query)\". Check the Healoo ID, or try part of the name.")
                                .font(HFont.body).foregroundStyle(Sage.muted).padding(.vertical, 12)
                        } else {
                            SectionHeader(title: "Results").padding(.top, 4)
                            ForEach(results) { user in resultRow(user) }
                        }
                    } else { LoadingView() }
                }
                .padding(.horizontal, 20).padding(.vertical, 16)
            }
        }
        .background(Sage.background)
        .task(id: "\(query)|\(role?.rawValue ?? "")") { await run(debounce: true) }
        .fullScreenCover(isPresented: $showScanner) {
            QRScannerSheet { id in
                showScanner = false
                if let id { Task { await openScanned(id) } }
            }
        }
        .navigationDestination(item: $scannedUser) { UserPageView(userId: $0, startOnMessages: false).toolbar(.hidden, for: .navigationBar) }
    }

    private func resultRow(_ user: UserProfile) -> some View {
        HStack(spacing: 12) {
            NavigationLink(value: Route.user(user.id, messages: false)) {
                HStack(spacing: 12) {
                    Avatar(initials: user.initials, background: tint(for: user.primaryRole), photoUrl: user.photoUri)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(user.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                        Text([user.headline, user.primaryRole == .doctor ? user.hospital : nil, user.publicId].compactMap { $0 }.joined(separator: " · "))
                            .font(HFont.small).foregroundStyle(Sage.muted).lineLimit(1)
                    }
                    Spacer(minLength: 0)
                }
            }
            .buttonStyle(.plain)

            if user.connected {
                Label("Connected", systemImage: "checkmark").font(HFont.smallStrong).foregroundStyle(Sage.accent)
                    .padding(.horizontal, 10).padding(.vertical, 6).background(Sage.sageTint, in: Capsule())
            } else {
                Button { Task { await connect(user) } } label: {
                    Label(adding.contains(user.id) ? "Adding…" : "Add", systemImage: "plus").font(HFont.captionStrong)
                        .foregroundStyle(Sage.accent).padding(.horizontal, 14).frame(minHeight: 40)
                        .overlay(Capsule().stroke(Sage.accent, lineWidth: 1))
                }
                .disabled(adding.contains(user.id))
                .accessibilityLabel("Add \(user.displayName) to contacts")
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
    }

    @State private var scannedUser: String?

    /// Scanned a Healoo ID: open that profile (profile only — data still needs a grant).
    private func openScanned(_ publicId: String) async {
        scanNote = nil
        let match = try? await env.repo.search(publicId, role: nil).first { $0.publicId.caseInsensitiveCompare(publicId) == .orderedSame }
        if let match { scannedUser = match.id } else { query = publicId; scanNote = "No one has the Healoo ID \(publicId)." }
    }

    private func tint(for role: Role) -> Color {
        switch role { case .hospital, .patient: Sage.sandTint; case .lab: Sage.clayTint; default: Sage.sageTint }
    }

    private func run(debounce: Bool) async {
        if debounce { try? await Task.sleep(nanoseconds: 300_000_000); if Task.isCancelled { return } }
        error = nil
        do { results = try await env.repo.search(query, role: role) }
        catch { if !Task.isCancelled { self.error = "Search isn't available right now. Try again in a moment." } }
    }

    private func connect(_ user: UserProfile) async {
        adding.insert(user.id)
        if let updated = try? await env.repo.connect(user.id) {
            results = results?.map { $0.id == updated.id ? updated : $0 }
        }
        adding.remove(user.id)
    }
}
