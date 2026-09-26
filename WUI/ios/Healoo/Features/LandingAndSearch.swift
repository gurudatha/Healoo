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

    var body: some View {
        VStack(spacing: 0) {
            header
            ScrollView {
                LazyVStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 10) {
                        FieldLabel("Find doctors, patients, hospitals or labs")
                        SearchField(text: $query, placeholder: "Search by name or Healoo ID", onSubmit: { openSearch(role) })
                        RoleFilterRow(selected: role, includeAll: false) { role = $0; openSearch($0) }
                    }
                    .padding(.bottom, 6)

                    SectionHeader(title: "Open items", action: "See all") { openSearch(nil) }

                    if let error {
                        ErrorView(message: error) { Task { await load() } }
                    } else if let items {
                        if items.isEmpty {
                            Text("Nothing open right now. New reports, messages and bookings will appear here.")
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
                        .font(HFont.greeting).foregroundStyle(.white).lineLimit(1)
                }
                Spacer()
                Button { router.openTab(.settings) } label: { Avatar(initials: me?.initials ?? "", ring: Sage.onPrimaryLine) }
                    .accessibilityLabel("Open profile and settings")
            }
            Spacer(minLength: 8)
            HStack(spacing: 8) {
                counter(dashboard?.openReports, "Open reports")
                counter(dashboard?.unreadMessages, "Unread messages")
                counter(dashboard?.upcomingAppointments, "Appointments")
            }
        }
        .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 16)
        .heightFraction(HeaderRatio.landing)
        .sageHeaderBackground()
    }

    private var greeting: String {
        switch Calendar.current.component(.hour, from: Date()) { case 0..<12: "Good morning"; case 12..<17: "Good afternoon"; default: "Good evening" }
    }

    private func counter(_ value: Int?, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value.map(String.init) ?? "–").font(HFont.counter).foregroundStyle(.white)
            Text(label).font(HFont.small).foregroundStyle(Sage.onPrimarySoft).lineLimit(1).minimumScaleFactor(0.85)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12).padding(.vertical, 10)
        .background(Sage.primaryRaised, in: RoundedRectangle(cornerRadius: 14))
        .accessibilityElement(children: .combine)
    }

    private func openSearch(_ role: Role?) { router.home.append(Route.search(query: query, role: role)) }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            dashboard = try await env.repo.dashboard()
            items = try await env.repo.openItems(limit: 20)
        } catch { self.error = "Couldn't load your items. Check your connection and try again." }
    }
}

// MARK: - Shared search pieces

struct SearchField<Trailing: View>: View {
    @Binding var text: String
    let placeholder: String
    var onSubmit: () -> Void = {}
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass").foregroundStyle(Sage.muted).accessibilityHidden(true)
            TextField(placeholder, text: $text, prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
                .font(HFont.body).foregroundStyle(Sage.ink)
                .submitLabel(.search).onSubmit(onSubmit)
                .autocorrectionDisabled().textInputAutocapitalization(.never)
            trailing()
        }
        .padding(.leading, 14).padding(.trailing, 8).frame(minHeight: 48)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.field))
        .overlay(RoundedRectangle(cornerRadius: Radius.field).stroke(Sage.border, lineWidth: 1))
    }
}

extension SearchField where Trailing == EmptyView {
    init(text: Binding<String>, placeholder: String, onSubmit: @escaping () -> Void = {}) {
        self.init(text: text, placeholder: placeholder, onSubmit: onSubmit) { EmptyView() }
    }
}

struct RoleFilterRow: View {
    let selected: Role?
    var includeAll = true
    var onHeader = false
    let onSelect: (Role?) -> Void
    private let filters: [(Role?, String)] = [(nil, "All"), (.doctor, "Doctors"), (.patient, "Users"), (.hospital, "Hospitals"), (.lab, "Labs")]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(filters.filter { includeAll || $0.0 != nil }, id: \.1) { filter in
                    SageChip(label: filter.1, selected: selected == filter.0, onHeader: onHeader) { onSelect(filter.0) }
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
                Text("Search").font(HFont.screenTitle).foregroundStyle(.white)
                SearchField(text: $query, placeholder: "Healoo ID or name") {
                    Button { showScanner = true } label: {
                        Image(systemName: "qrcode.viewfinder").foregroundStyle(Sage.primary)
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
                    Avatar(initials: user.initials, background: tint(for: user.primaryRole))
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
                Label("Connected", systemImage: "checkmark").font(HFont.smallStrong).foregroundStyle(Sage.primary)
                    .padding(.horizontal, 10).padding(.vertical, 6).background(Sage.sageTint, in: Capsule())
            } else {
                Button { Task { await connect(user) } } label: {
                    Label(adding.contains(user.id) ? "Adding…" : "Add", systemImage: "plus").font(HFont.captionStrong)
                        .foregroundStyle(Sage.primary).padding(.horizontal, 14).frame(minHeight: 40)
                        .overlay(Capsule().stroke(Sage.primary, lineWidth: 1))
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
