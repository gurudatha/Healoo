import SwiftUI

private struct ScrollOffsetKey: PreferenceKey {
    static var defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
}

// MARK: - User page (header collapses 25% → 10%, then stays pinned)

struct UserPageView: View {
    let userId: String
    let startOnMessages: Bool

    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @Environment(\.dismiss) private var dismiss
    @State private var me: UserProfile?
    @State private var user: UserProfile?
    @State private var shared: [DataItem] = []
    @State private var messages: [Message] = []
    @State private var tab = 0
    @State private var draft = ""
    @State private var error: String?
    @State private var offset: CGFloat = 0

    var body: some View {
        GeometryReader { geo in
            let maxH = geo.size.height * HeaderRatio.userExpanded
            let minH = geo.size.height * HeaderRatio.userCollapsed
            let current = max(minH, maxH + min(0, offset))
            let progress = (maxH - current) / max(maxH - minH, 1)

            ZStack(alignment: .top) {
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 12) {
                            Color.clear.frame(height: maxH)
                                .background(GeometryReader { g in
                                    Color.clear.preference(key: ScrollOffsetKey.self, value: g.frame(in: .named("userScroll")).minY)
                                })
                            content(proxy: proxy).padding(.horizontal, 20)
                        }
                        .padding(.bottom, 24)
                    }
                    .coordinateSpace(name: "userScroll")
                    .onPreferenceChange(ScrollOffsetKey.self) { offset = $0 }
                }
                if let user { ProfileHeader(user: user, progress: progress) { dismiss() }.frame(height: current) }
            }
        }
        .background(Sage.background)
        .safeAreaInset(edge: .bottom, spacing: 0) { if tab == 1 && user?.connected == true { composer } }
        .task { if user == nil { await load() } }
        // Live messages from the WebSocket (doc 4.4).
        .onReceive(env.repo.events) { event in
            if case .newMessage(let m) = event, m.senderId == userId, !messages.contains(where: { $0.id == m.id }) {
                messages.append(m)
            }
        }
    }

    @ViewBuilder
    private func content(proxy: ScrollViewProxy) -> some View {
        if let error { ErrorView(message: error) { Task { await load() } } }
        else if let user {
            if !user.connected {
                Text("You can see \(user.displayName)'s profile. Add them to your contacts to message them; records appear only after the owner shares them.")
                    .font(HFont.body).foregroundStyle(Sage.inkSoft).padding(.top, 16)
                Button("Add to contacts") { Task { _ = try? await env.repo.connect(user.id); await load() } }
                    .buttonStyle(PrimaryButtonStyle())
            } else {
                // Doctors, assistants and labs can upload a record the patient will own (doc 2.3).
                if me?.isClinical == true && user.primaryRole == .patient {
                    Button("Upload for \(user.displayName.split(separator: " ").first.map(String.init) ?? user.displayName)") {
                        router.upload(for: user.id)
                    }
                    .buttonStyle(PrimaryButtonStyle()).padding(.top, 16)
                }
                Segmented(options: ["Shared items · \(shared.count)", "Messages"], selection: $tab).padding(.top, 16)
                if tab == 0 {
                    if shared.isEmpty { Text("Nothing shared between you yet.").font(HFont.body).foregroundStyle(Sage.muted) }
                    ForEach(shared) { item in
                        NavigationLink(value: Route.item(item.id)) { DataItemRow(item: item) }.buttonStyle(.plain)
                    }
                } else {
                    ForEach(messages) { bubble($0).id($0.id) }
                        .onChange(of: messages.count) { if let last = messages.last { withAnimation { proxy.scrollTo(last.id, anchor: .bottom) } } }
                }
            }
        } else { LoadingView().padding(.top, 40) }
    }

    private func bubble(_ m: Message) -> some View {
        let mine = m.senderId == me?.id
        return HStack {
            if mine { Spacer(minLength: 60) }
            VStack(alignment: .leading, spacing: 4) {
                Text(m.body).font(.custom(FontName.figtreeRegular, size: 14)).foregroundStyle(mine ? .white : Sage.ink)
                Text(mine ? "You · \(m.sentAt)" : m.sentAt).font(.custom(FontName.figtreeRegular, size: 11))
                    .foregroundStyle(mine ? Sage.onPrimaryLine : Sage.muted)
            }
            .padding(.horizontal, 14).padding(.vertical, 10)
            .background(mine ? Sage.primary : Sage.surface,
                        in: UnevenRoundedRectangle(topLeadingRadius: 16, bottomLeadingRadius: mine ? 16 : 4, bottomTrailingRadius: mine ? 4 : 16, topTrailingRadius: 16))
            if !mine { Spacer(minLength: 60) }
        }
    }

    private var composer: some View {
        HStack(spacing: 8) {
            TextField("Write a message", text: $draft, axis: .vertical).lineLimit(1...4)
                .font(HFont.body).padding(.horizontal, 14).padding(.vertical, 10)
                .overlay(RoundedRectangle(cornerRadius: 22).stroke(Sage.border))
                .submitLabel(.send).onSubmit(send)
            Button(action: send) {
                Image(systemName: "paperplane.fill").foregroundStyle(.white).frame(width: 48, height: 48)
                    .background(draft.trimmingCharacters(in: .whitespaces).isEmpty ? Sage.muted : Sage.primary, in: Circle())
            }
            .disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(Sage.surface.ignoresSafeArea(edges: .bottom))
    }

    private func send() {
        let body = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { return }
        draft = ""
        Task {
            if let m = try? await env.repo.send(to: userId, body: body) {
                if !messages.contains(where: { $0.id == m.id }) { messages.append(m) }
            } else { draft = body }        // keep the text so it can be resent
        }
    }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            let u = try await env.repo.user(userId)
            user = u
            if u.connected {
                shared = try await env.repo.sharedItems(with: userId)
                messages = try await env.repo.messages(with: userId)
                if startOnMessages { tab = 1 }
            }
        } catch { self.error = "Couldn't load this profile. Try again." }
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

struct ThreadsView: View {
    @Environment(AppEnvironment.self) private var env
    @State private var threads: [ThreadSummary]?

    var body: some View {
        VStack(spacing: 0) {
            Text("Messages").font(HFont.screenTitle).foregroundStyle(.white)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 20)
                .sageHeaderBackground()
            ScrollView {
                LazyVStack(spacing: 8) {
                    if let threads {
                        if threads.isEmpty {
                            Text("No conversations yet. Open a doctor's profile from Search to send the first message.")
                                .font(HFont.body).foregroundStyle(Sage.muted)
                        }
                        ForEach(threads) { t in
                            NavigationLink(value: Route.user(t.otherUser.id, messages: true)) {
                                HStack(spacing: 12) {
                                    Avatar(initials: t.otherUser.initials)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(t.otherUser.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                                        Text(t.lastMessage).font(HFont.caption).foregroundStyle(Sage.muted).lineLimit(1)
                                    }
                                    Spacer(minLength: 0)
                                    VStack(alignment: .trailing, spacing: 4) {
                                        Text(t.lastMessageAt).font(HFont.small).foregroundStyle(Sage.muted)
                                        if t.unread > 0 {
                                            Text("\(t.unread)").font(HFont.tiny).foregroundStyle(.white)
                                                .frame(width: 20, height: 20).background(Sage.primary, in: Circle())
                                        }
                                    }
                                }
                                .padding(12)
                                .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
                            }
                            .buttonStyle(.plain)
                        }
                    } else { LoadingView() }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .task { threads = (try? await env.repo.threads()) ?? [] }
        .onReceive(env.repo.events) { event in
            if case .newMessage = event { Task { threads = (try? await env.repo.threads()) ?? threads } }
        }
    }
}
