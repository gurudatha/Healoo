import SwiftUI

private struct ScrollOffsetKey: PreferenceKey {
    static var defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
}

// MARK: - Person page (user, doctor or hospital; header collapses 25% → 10%, then stays pinned)

private enum PageSheet: String, Identifiable { case message, share, bookAtHospital; var id: String { rawValue } }

struct UserPageView: View {
    let userId: String
    let startOnMessages: Bool

    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @Environment(\.dismiss) private var dismiss
    @State private var me: UserProfile?
    @State private var user: UserProfile?
    @State private var shared: [DataItem] = []
    @State private var conversations: [Conversation] = []
    @State private var contacts: [UserProfile] = []
    @State private var doctors: [UserProfile] = []          // hospital page
    @State private var tab = 0
    @State private var error: String?
    @State private var notice: String?
    @State private var offset: CGFloat = 0
    @State private var sheet: PageSheet?
    @FocusState private var doctorSearchFocused: Bool

    private var page: PageKind { user.map(PageOperations.page(for:)) ?? .user }
    private var operations: ResolvedOperations { PageOperations.resolve(page, facts: OpFact.of(viewer: me, subject: user)) }

    var body: some View {
        GeometryReader { geo in
            let maxH = geo.size.height * HeaderRatio.userExpanded
            let minH = geo.size.height * HeaderRatio.userCollapsed
            let current = max(minH, maxH + min(0, offset))
            let progress = (maxH - current) / max(maxH - minH, 1)

            ZStack(alignment: .top) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        Color.clear.frame(height: maxH)
                            .background(GeometryReader { g in
                                Color.clear.preference(key: ScrollOffsetKey.self, value: g.frame(in: .named("userScroll")).minY)
                            })
                        content.padding(.horizontal, 20)
                    }
                    .padding(.bottom, 24)
                }
                .coordinateSpace(name: "userScroll")
                .onPreferenceChange(ScrollOffsetKey.self) { offset = $0 }
                if let user { ProfileHeader(user: user, progress: progress) { dismiss() }.frame(height: current) }
            }
        }
        .background(Sage.background)
        .safeAreaInset(edge: .bottom, spacing: 0) { if user != nil { OperationBar(ops: operations, perform: perform) } }
        .onAppear { router.personPages += 1 }
        .onDisappear { router.personPages -= 1 }
        .task { if user == nil { await load() } }
        // Live messages from the WebSocket (doc 4.4): refresh the conversation list.
        .onReceive(env.repo.events) { event in
            if case .newMessage = event, user?.connected == true {
                Task { conversations = (try? await env.repo.conversations(with: userId)) ?? conversations }
            }
        }
        .sheet(item: $sheet) { which in
            if let user {
                NavigationStack {
                    switch which {
                    case .message:
                        MessageSheet(user: user, me: me, contacts: contacts) { id in
                            sheet = nil
                            Task { conversations = (try? await env.repo.conversations(with: userId)) ?? conversations }
                            router.push(.discussion(id))
                        }
                    case .share:
                        ShareSheet(user: user, me: me, contacts: contacts) { text in
                            sheet = nil; notice = text; tab = 0
                            Task { shared = (try? await env.repo.sharedItems(with: userId)) ?? shared }
                        }
                    case .bookAtHospital:
                        BookAtHospitalSheet(hospital: user, doctors: doctors) { d in
                            sheet = nil; router.upload(UploadRequest(to: nil, mode: .appointment, doctorId: d.id, hospitalId: user.id))
                        }
                    }
                }
                .environment(env)
                .presentationDetents([.large])
            }
        }
    }

    private func perform(_ op: String) {
        guard let user else { return }
        switch op {
        case OpId.connect: Task { _ = try? await env.repo.connect(user.id); await load() }
        case OpId.message: sheet = .message
        case OpId.history: tab = 0
        case OpId.shareDocument: sheet = .share
        case OpId.uploadFor: router.upload(UploadRequest(to: user.id))
        case OpId.bookAppointment:
            if page == .hospital { sheet = .bookAtHospital }
            else { router.upload(UploadRequest(to: user.id, mode: .appointment, doctorId: user.id)) }
        case OpId.searchDoctors: doctorSearchFocused = true
        default: break
        }
    }

    @ViewBuilder
    private var content: some View {
        if let error { ErrorView(message: error) { Task { await load() } } }
        else if let user {
            if let notice {
                HStack {
                    Text(notice).font(HFont.caption).foregroundStyle(Sage.primary)
                    Spacer()
                    Button("OK") { self.notice = nil }.font(HFont.captionStrong).foregroundStyle(Sage.primary)
                }
                .padding(12).background(Sage.sageTint, in: RoundedRectangle(cornerRadius: 12)).padding(.top, 12)
            }
            if page == .hospital {
                SectionHeader(title: "Doctors · \(doctors.count)").padding(.top, 16)
                FilteredSearchBar(
                    placeholder: "Search doctors by name or Healoo ID", candidates: doctors,
                    matches: { $0.matches($1) || $0.headline.lowercased().contains($1) },
                    onPick: { router.push(.user($0.id, messages: false)) },
                    showAllWhenBlank: true, maxResults: 100, focused: $doctorSearchFocused,
                    emptyText: doctors.isEmpty ? "No doctors are listed for this hospital yet." : "No doctor matches."
                ) { UserResultRow(user: $0) }
            } else if !user.connected {
                Text("You can see \(user.displayName)'s profile. Add them to your contacts (below) to message them; records appear only after the owner shares them.")
                    .font(HFont.body).foregroundStyle(Sage.inkSoft).padding(.top, 16)
            } else {
                // History with this person: what is shared between you, and your discussions.
                Segmented(options: ["Shared items · \(shared.count)", "Messages · \(conversations.count)"], selection: $tab).padding(.top, 16)
                if tab == 0 {
                    if shared.isEmpty { Text("Nothing shared between you yet.").font(HFont.body).foregroundStyle(Sage.muted) }
                    ForEach(shared) { item in
                        NavigationLink(value: Route.item(item.id)) { DataItemRow(item: item) }.buttonStyle(.plain)
                    }
                } else {
                    // Every message belongs to an item (design D5): one row per discussion with this person.
                    if conversations.isEmpty {
                        Text("No discussions with \(user.displayName) yet. Tap Message below to start one.")
                            .font(HFont.body).foregroundStyle(Sage.muted)
                    }
                    ForEach(conversations) { c in
                        NavigationLink(value: Route.discussion(c.itemId)) { ConversationRow(conversation: c, showPerson: false) }.buttonStyle(.plain)
                    }
                }
            }
        } else { LoadingView().padding(.top, 40) }
    }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            let u = try await env.repo.user(userId)
            user = u
            if PageOperations.page(for: u) == .hospital { doctors = try await env.repo.hospitalDoctors(u.id) }
            if u.connected {
                shared = try await env.repo.sharedItems(with: userId)
                conversations = try await env.repo.conversations(with: userId)
                if startOnMessages { tab = 1 }
            }
            contacts = (try? await env.repo.connections()) ?? []
        } catch { self.error = "Couldn't load this profile. Try again." }
    }
}

// MARK: - Operation sheets

/// Message: a new discussion item; the page's person is always a recipient.
private struct MessageSheet: View {
    let user: UserProfile
    let me: UserProfile?
    let contacts: [UserProfile]
    let onStarted: (String) -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var extras: [UserProfile] = []
    @State private var draft = ""
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // A clinician writing to a patient starts it on the patient's behalf; only doctors can be added.
                RecipientField(fixed: user, extras: $extras, contacts: contacts,
                               doctorsOnly: me?.isClinical == true && user.primaryRole == .patient, exclude: Set([me?.id].compactMap { $0 }))
                VStack(alignment: .leading, spacing: 6) {
                    FieldLabel("Message")
                    TextField("Write to \(user.displayName)", text: $draft, axis: .vertical)
                        .lineLimit(3...8).font(HFont.body).padding(12)
                        .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
                    Text("This starts a new discussion item that everyone above can read and add to.").font(HFont.small).foregroundStyle(Sage.muted)
                }
                if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Sending…" : "Send", action: send).buttonStyle(PrimaryButtonStyle()).disabled(busy)
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle("New message").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
    }

    private func send() {
        let body = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { error = "Write a message first."; return }
        busy = true
        Task {
            do { onStarted(try await env.repo.startConversation(with: user.id, body: body, alsoWith: extras.map(\.id)).id) }
            catch { self.error = "That didn't send: \(error.localizedDescription). Try again." }
            busy = false
        }
    }
}

/// Share a document: one of the viewer's existing items, to the page's person (and anyone added).
private struct ShareSheet: View {
    let user: UserProfile
    let me: UserProfile?
    let contacts: [UserProfile]
    let onShared: (String) -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var items: [DataItem] = []
    @State private var picked: DataItem?
    @State private var extras: [UserProfile] = []
    @State private var busy = false
    @State private var error: String?

    /// Passing on an item the viewer doesn't own is allowed to doctors only (server rule 9).
    private var referral: Bool { picked.map { $0.ownerId != me?.id } ?? false }
    private let kinds = PrimaryKind.allCases.map { k in SearchFilter<DataItem>(label: k.label + "s") { $0.primaryKind == k } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    FieldLabel("Document")
                    if let picked { PickedRow(content: { ItemResultRow(item: picked) }, onChange: { self.picked = nil }) }
                    else {
                        FilteredSearchBar(
                            placeholder: "Search your items", candidates: items,
                            matches: { i, q in i.title.lowercased().contains(q) || i.keywords.contains { $0.lowercased().contains(q) } },
                            onPick: { picked = $0; error = nil }, filters: kinds, showAllWhenBlank: true, maxResults: 6,
                            emptyText: items.isEmpty ? "You have no open items to share yet. New documents come in through Upload." : "No item matches."
                        ) { ItemResultRow(item: $0) }
                    }
                    if referral {
                        Text("You don't own this item, so it can be passed on to doctors only. The owner sees who has it.")
                            .font(HFont.small).foregroundStyle(Sage.sandInk)
                    }
                }
                RecipientField(fixed: user, extras: $extras, contacts: contacts, doctorsOnly: referral,
                               exclude: Set([me?.id].compactMap { $0 }), label: "Share with")
                if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Sharing…" : "Share", action: share).buttonStyle(PrimaryButtonStyle()).disabled(busy || picked == nil)
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle("Share a document").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .task { items = ((try? await env.repo.items(status: .open, limit: 100)) ?? []).filter { $0.can("share") && $0.ownerId != user.id } }
    }

    private func share() {
        guard let item = picked else { error = "Choose a document to share."; return }
        let people = [user] + extras
        if referral && people.contains(where: { $0.primaryRole != .doctor }) {
            error = "You don't own \"\(item.title)\", so you can pass it on to doctors only."; return
        }
        busy = true
        Task {
            do {
                for p in people { _ = try await env.repo.share(item.id, with: p.id) }
                onShared("Shared \"\(item.title)\" with \(people.map(\.displayName).joined(separator: ", ")).")
            } catch { self.error = "That didn't share: \(error.localizedDescription)." }
            busy = false
        }
    }
}

/// Book on a hospital's page: choose one of its doctors, then the appointment form.
private struct BookAtHospitalSheet: View {
    let hospital: UserProfile
    let doctors: [UserProfile]
    let onPick: (UserProfile) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Choose a doctor; you pick the date and time next.").font(HFont.caption).foregroundStyle(Sage.muted)
                FilteredSearchBar(
                    placeholder: "Search doctors by name or Healoo ID", candidates: doctors,
                    matches: { $0.matches($1) || $0.headline.lowercased().contains($1) },
                    onPick: onPick, showAllWhenBlank: true, maxResults: 100,
                    emptyText: doctors.isEmpty ? "No doctors are listed for this hospital yet." : "No doctor matches."
                ) { UserResultRow(user: $0, trailing: "Book") }
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle("Book at \(hospital.displayName)").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
    }
}

private struct ProfileHeader: View {
    let user: UserProfile
    let progress: CGFloat          // 0 expanded … 1 collapsed
    let onBack: () -> Void

    var body: some View {
        let details = max(0, 1 - progress * 1.6)           // details fade out early
        ZStack(alignment: .top) {
            HStack(spacing: 12) {
                HeaderIconButton(symbol: "chevron.left", label: "Back", action: onBack)
                Text(user.displayName).font(HFont.headerTitle).foregroundStyle(.white).lineLimit(1).opacity(1 - details)
                Spacer(minLength: 0)
                if user.connected {
                    Label("Connected", systemImage: "checkmark").font(HFont.smallStrong).foregroundStyle(Sage.onPrimarySoft)
                        .padding(.horizontal, 12).padding(.vertical, 6).background(Sage.primaryRaised, in: Capsule())
                }
            }
            if details > 0.02 {
                VStack(alignment: .leading, spacing: 12) {
                    Spacer(minLength: 0)
                    HStack(spacing: 14) {
                        Avatar(initials: user.initials, size: 64, ring: Sage.onPrimaryLine)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(user.displayName).font(HFont.profileName).foregroundStyle(.white)
                            Text(user.headline).font(HFont.caption).foregroundStyle(Sage.onPrimarySoft)
                        }
                    }
                    HStack(alignment: .top, spacing: 8) {
                        ForEach(facts, id: \.0) { fact in
                            VStack(alignment: .leading, spacing: 1) {
                                Text(fact.0).font(.custom(FontName.figtreeRegular, size: 11)).foregroundStyle(Sage.onPrimaryLine)
                                Text(fact.1).font(HFont.captionStrong).foregroundStyle(.white).lineLimit(1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
                .opacity(details)
                .accessibilityElement(children: .combine)
            }
        }
        .padding(.horizontal, 20).padding(.top, 4).padding(.bottom, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(
            UnevenRoundedRectangle(bottomLeadingRadius: Radius.header * (1 - progress), bottomTrailingRadius: Radius.header * (1 - progress))
                .fill(Sage.primary).ignoresSafeArea(edges: .top)
        )
        .clipped()
    }

    private var facts: [(String, String)] {
        [user.hospital.map { ("Hospital", $0) }, user.officialNumber.map { ("Reg. number", $0) }, ("Healoo ID", user.publicId)].compactMap { $0 }
    }
}

// MARK: - Messages tab

/// Every item with a discussion, per person (replaces the v0.1 chat threads).
struct ConversationsView: View {
    @Environment(AppEnvironment.self) private var env
    @State private var list: [Conversation]?

    var body: some View {
        VStack(spacing: 0) {
            Text("Messages").font(HFont.screenTitle).foregroundStyle(.white)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 20)
                .sageHeaderBackground()
            ScrollView {
                LazyVStack(spacing: 8) {
                    if let list {
                        if list.isEmpty {
                            Text("No conversations yet. Open a person's profile from Search to start one, or use Start discussion on any item.")
                                .font(HFont.body).foregroundStyle(Sage.muted)
                        }
                        ForEach(list) { c in
                            NavigationLink(value: Route.discussion(c.itemId)) { ConversationRow(conversation: c, showPerson: true) }.buttonStyle(.plain)
                        }
                    } else { LoadingView() }
                }
                .padding(20)
            }
            .refreshable { await load() }
        }
        .background(Sage.background)
        .task { await load() }
        .onReceive(env.repo.events) { event in
            if case .newMessage = event { Task { await load() } }
        }
    }

    private func load() async { list = (try? await env.repo.conversations()) ?? list ?? [] }
}

struct ConversationRow: View {
    let conversation: Conversation
    let showPerson: Bool

    var body: some View {
        let c = conversation
        HStack(spacing: 12) {
            if showPerson { Avatar(initials: c.otherUser.initials) } else { TypeTile(type: c.primaryKind) }
            VStack(alignment: .leading, spacing: 2) {
                Text(showPerson ? c.otherUser.displayName : c.itemTitle).font(HFont.bodyStrong).foregroundStyle(Sage.ink).lineLimit(1)
                if showPerson { Text(c.itemTitle).font(HFont.smallStrong).foregroundStyle(Sage.primary).lineLimit(1) }
                Text(c.lastMessage).font(HFont.caption).foregroundStyle(Sage.muted).lineLimit(1)
            }
            Spacer(minLength: 0)
            VStack(alignment: .trailing, spacing: 4) {
                Text(MessageTime.short(c.lastMessageAt)).font(HFont.small).foregroundStyle(Sage.muted)
                if c.unread > 0 {
                    Text("\(c.unread)").font(HFont.tiny).foregroundStyle(.white)
                        .frame(width: 20, height: 20).background(Sage.primary, in: Circle())
                        .accessibilityLabel("\(c.unread) unread")
                }
            }
        }
        .padding(12)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
        .accessibilityElement(children: .combine)
    }
}

// MARK: - Discussion (an item's messages)

struct DiscussionView: View {
    let itemId: String
    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @Environment(\.dismiss) private var dismiss
    @State private var item: DataItem?
    @State private var me: UserProfile?
    @State private var messages: [Message] = []
    @State private var draft = ""
    @State private var sending = false
    @State private var error: String?

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: item?.title ?? "Discussion", subtitle: item.map { "\($0.primaryKind.label) · everyone this item is shared with" } ?? "",
                         onBack: { dismiss() }) {
                if item != nil {
                    Button("Details") { router.push(.item(itemId)) }.font(HFont.captionStrong).foregroundStyle(.white).frame(minHeight: 44)
                }
            }
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 10) {
                        if let error { ErrorView(message: error) { Task { await load() } } }
                        else if item == nil { LoadingView() }
                        else if messages.isEmpty {
                            Text("No messages yet. Write the first one below.").font(HFont.body).foregroundStyle(Sage.muted)
                        }
                        ForEach(messages) { m in
                            MessageBubble(message: m, mine: m.senderId == me?.id, name: name(m.senderId)).id(m.id)
                        }
                    }
                    .padding(20)
                }
                .onChange(of: messages.count) { if let last = messages.last { withAnimation { proxy.scrollTo(last.id, anchor: .bottom) } } }
            }
        }
        .background(Sage.background)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if let item {
                if item.can("message") { Composer(draft: $draft, placeholder: "Write a message", busy: sending, send: send) }
                else {
                    Text(item.status == .closed ? "This item is closed. Reopen it to continue the discussion." : "You can read this discussion but not reply.")
                        .font(HFont.small).foregroundStyle(Sage.muted).frame(maxWidth: .infinity).padding(14)
                        .background(Sage.surface.ignoresSafeArea(edges: .bottom))
                }
            }
        }
        .task { if item == nil { await load() } }
        .onReceive(env.repo.events) { event in
            if case .newMessage(let m) = event, m.itemId == itemId, !messages.contains(where: { $0.id == m.id }) { messages.append(m) }
        }
    }

    private func name(_ senderId: String) -> String {
        guard let item else { return "" }
        if senderId == me?.id { return "You" }
        if let g = item.accessList.first(where: { $0.granteeId == senderId }) { return g.granteeName }
        if let a = item.appointments.first(where: { $0.doctorId == senderId }), !a.doctorName.isEmpty { return a.doctorName }
        if senderId == item.ownerId { return "Patient" }
        return item.createdByName
    }

    private func send() {
        let body = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { return }
        draft = ""; sending = true
        Task {
            do {
                let m = try await env.repo.sendItemMessage(itemId, body: body)
                if !messages.contains(where: { $0.id == m.id }) { messages.append(m) }
            } catch { draft = body }           // keep the text so it can be resent
            sending = false
        }
    }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            item = try await env.repo.item(itemId)
            messages = try await env.repo.itemMessages(itemId)
        } catch { self.error = "Couldn't open this discussion. You may no longer have access to it." }
    }
}

struct MessageBubble: View {
    let message: Message
    let mine: Bool
    let name: String

    var body: some View {
        HStack {
            if mine { Spacer(minLength: 60) }
            VStack(alignment: .leading, spacing: 4) {
                if !mine { Text(name).font(HFont.smallStrong).foregroundStyle(Sage.primary) }
                Text(message.body).font(.custom(FontName.figtreeRegular, size: 14)).foregroundStyle(mine ? .white : Sage.ink)
                Text(MessageTime.short(message.sentAt)).font(.custom(FontName.figtreeRegular, size: 11))
                    .foregroundStyle(mine ? Sage.onPrimaryLine : Sage.muted)
            }
            .padding(.horizontal, 14).padding(.vertical, 10)
            .background(mine ? Sage.primary : Sage.surface,
                        in: UnevenRoundedRectangle(topLeadingRadius: 16, bottomLeadingRadius: mine ? 16 : 4, bottomTrailingRadius: mine ? 4 : 16, topTrailingRadius: 16))
            if !mine { Spacer(minLength: 60) }
        }
        .accessibilityElement(children: .combine)
    }
}

struct Composer: View {
    @Binding var draft: String
    var placeholder = "Write a message"
    var busy = false
    let send: () -> Void

    var body: some View {
        let empty = draft.trimmingCharacters(in: .whitespaces).isEmpty
        HStack(spacing: 8) {
            TextField(placeholder, text: $draft, axis: .vertical).lineLimit(1...4)
                .font(HFont.body).padding(.horizontal, 14).padding(.vertical, 10)
                .overlay(RoundedRectangle(cornerRadius: 22).stroke(Sage.border))
                .submitLabel(.send).onSubmit(send)
            Button(action: send) {
                Group { if busy { ProgressView().tint(.white) } else { Image(systemName: "paperplane.fill").foregroundStyle(.white) } }
                    .frame(width: 48, height: 48)
                    .background(empty ? Sage.muted : Sage.primary, in: Circle())
            }
            .disabled(empty || busy)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(Sage.surface.ignoresSafeArea(edges: .bottom))
    }
}

/// Message times: "10:42" today, otherwise "21 Sep".
enum MessageTime {
    private static let parser: ISO8601DateFormatter = { let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime]; return f }()
    private static let parserFrac: ISO8601DateFormatter = { let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]; return f }()
    private static let clock: DateFormatter = { let f = DateFormatter(); f.dateFormat = "HH:mm"; return f }()
    private static let day: DateFormatter = { let f = DateFormatter(); f.dateFormat = "d MMM"; return f }()

    static func short(_ iso: String) -> String {
        guard let d = parser.date(from: iso) ?? parserFrac.date(from: iso) else { return iso }
        return Calendar.current.isDateInToday(d) ? clock.string(from: d) : day.string(from: d)
    }
}
