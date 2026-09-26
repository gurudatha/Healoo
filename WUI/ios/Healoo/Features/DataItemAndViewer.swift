import SwiftUI
import PDFKit

// MARK: - Data view (10% pinned header)

struct DataItemView: View {
    let itemId: String
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL
    @State private var item: DataItem?
    @State private var linked: DataItem?
    @State private var me: UserProfile?
    @State private var contacts: [UserProfile] = []
    @State private var error: String?
    @State private var viewerIndex: ViewerStart?
    @State private var confirmRevoke: Grant?
    @State private var showShare = false
    @State private var messageUser: String?

    struct ViewerStart: Identifiable { let index: Int; var id: Int { index } }

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: item?.title ?? "", subtitle: item.map { "\(me?.displayName ?? "") · added by \($0.createdByName)" } ?? "",
                         onBack: { dismiss() }) {
                if let me { Avatar(initials: me.initials, size: 36) }
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let error { ErrorView(message: error) { Task { await load() } } }
                    else if let item { details(item) }
                    else { LoadingView() }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .bottomActionBar {
            if let item {
                if item.ownerId == me?.id {
                    let doctor = item.accessList.first { $0.granteeType == .user }
                    Button("Message doctor") { messageUser = doctor?.granteeId }.buttonStyle(SecondaryButtonStyle()).disabled(doctor == nil)
                } else {
                    // Doctor or lab viewing a patient's record: talk to the owner.
                    Button("Message patient") { messageUser = item.ownerId }.buttonStyle(SecondaryButtonStyle())
                }
                if item.ownerId == me?.id { Button("Share with…") { showShare = true }.buttonStyle(PrimaryButtonStyle()) }
            }
        }
        .navigationDestination(item: $messageUser) { UserPageView(userId: $0, startOnMessages: true).toolbar(.hidden, for: .navigationBar) }
        .fullScreenCover(item: $viewerIndex) { start in
            if let item { AttachmentViewer(title: item.title, attachments: item.attachments, startIndex: start.index) }
        }
        .confirmationDialog("Stop sharing with \(confirmRevoke?.granteeName ?? "")?", isPresented: Binding(
            get: { confirmRevoke != nil }, set: { if !$0 { confirmRevoke = nil } }), titleVisibility: .visible) {
            Button("Stop sharing", role: .destructive) {
                if let g = confirmRevoke { Task { if let u = try? await env.repo.revoke(itemId, grantId: g.grantId) { item = u } } }
            }
            Button("Keep sharing", role: .cancel) {}
        } message: { Text("They will lose access to this item straight away. You can share it again later.") }
        .sheet(isPresented: $showShare) { shareSheet.presentationDetents([.medium]) }
        .task { if item == nil { await load() } }
    }

    @ViewBuilder
    private func details(_ item: DataItem) -> some View {
        // Type · date · status
        HStack(spacing: 8) {
            pill(item.type.label, item.type.style.tint, item.type.style.fg)
            pill(DateText.long(item.date), Sage.sandTint, Sage.sand)
            Spacer()
            let open = item.status == .open
            Button { Task { if let u = try? await env.repo.setStatus(itemId, open ? .closed : .open) { self.item = u } } } label: {
                HStack(spacing: 6) {
                    Circle().fill(open ? Sage.primary : Sage.muted).frame(width: 8, height: 8)
                    Text(open ? "Open" : "Closed").font(HFont.smallStrong)
                }
                .foregroundStyle(open ? Sage.primary : Sage.muted).padding(.horizontal, 12).frame(minHeight: 36)
                .overlay(Capsule().stroke(open ? Sage.primary : Sage.border))
            }
            .accessibilityLabel(open ? "Status open. Double-tap to close" : "Status closed. Double-tap to reopen")
        }

        if !item.attachments.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                FieldLabel("Attachments · \(item.attachments.count)")
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 10) {
                        ForEach(Array(item.attachments.enumerated()), id: \.element.id) { i, a in
                            Button { viewerIndex = ViewerStart(index: i) } label: { attachmentCard(a) }
                                .buttonStyle(.plain)
                                .accessibilityLabel("Open \(a.name), \(i + 1) of \(item.attachments.count)")
                        }
                    }
                }
            }
        }

        if !item.links.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                FieldLabel("Links")
                GroupCard {
                    ForEach(Array(item.links.enumerated()), id: \.offset) { i, link in
                        if i > 0 { RowDivider() }
                        Button { if let u = URL(string: link) { openURL(u) } } label: {
                            Label(link, systemImage: "link").font(HFont.caption).foregroundStyle(Sage.primary).lineLimit(1)
                                .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading).padding(.horizontal, 14)
                        }
                    }
                }
            }
        }

        if !item.keywords.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                FieldLabel("Keywords")
                FlowLayout(spacing: 8) {
                    ForEach(item.keywords, id: \.self) {
                        Text($0).font(HFont.caption).foregroundStyle(Sage.ink).padding(.horizontal, 12).padding(.vertical, 7)
                            .background(Sage.surface, in: Capsule()).overlay(Capsule().stroke(Sage.border))
                    }
                }
            }
        }

        let isOwner = item.ownerId == me?.id
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Who can see this")
            GroupCard {
                accessRow("person", isOwner ? "You (owner)" : "Owner", "Full control", nil)
                ForEach(item.accessList) { g in
                    RowDivider()
                    let hospital = g.granteeType == .hospital
                    let revoke: (() -> Void)? = isOwner ? { confirmRevoke = g } : nil
                    accessRow(hospital ? "cross.case" : "person", g.granteeName,
                              hospital ? "All affiliated doctors" : (g.viaHospitalId != nil ? "Via hospital" : "Shared directly"),
                              revoke)
                }
            }
            if item.type == .report {
                Text("Report files open only for doctors you share with.").font(HFont.small).foregroundStyle(Sage.muted)
            }
        }

        if let linked {
            VStack(alignment: .leading, spacing: 8) {
                FieldLabel("Linked items")
                NavigationLink(value: Route.item(linked.id)) { DataItemRow(item: linked) }.buttonStyle(.plain)
            }
        }
    }

    private func pill(_ text: String, _ bg: Color, _ fg: Color) -> some View {
        Text(text).font(HFont.smallStrong).foregroundStyle(fg).padding(.horizontal, 12).padding(.vertical, 6).background(bg, in: Capsule())
    }

    private func attachmentCard(_ a: Attachment) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack {
                Sage.preview
                if a.kind == .image { AttachmentImage(attachment: a, contentMode: .fill) }
                else { Image(systemName: "doc.richtext").font(.system(size: 30)).foregroundStyle(Sage.muted) }
            }
            .frame(width: 132, height: 110).clipped()
            VStack(alignment: .leading, spacing: 1) {
                Text(a.name).font(HFont.smallStrong).foregroundStyle(Sage.ink).lineLimit(1)
                Text(a.kind == .pdf ? "PDF · \(a.pageCount.map(String.init) ?? "?") pages" : "Image · \(a.size / 1024) KB")
                    .font(.custom(FontName.figtreeRegular, size: 11)).foregroundStyle(Sage.muted)
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
        }
        .frame(width: 132)
        .background(Sage.surface)
        .clipShape(RoundedRectangle(cornerRadius: Radius.card))
    }

    private func accessRow(_ symbol: String, _ name: String, _ note: String, _ onRevoke: (() -> Void)?) -> some View {
        HStack(spacing: 12) {
            Image(systemName: symbol).font(.system(size: 15)).foregroundStyle(Sage.primary)
                .frame(width: 32, height: 32).background(Sage.sageTint, in: RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 1) {
                Text(name).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                Text(note).font(HFont.small).foregroundStyle(Sage.muted)
            }
            Spacer()
            if let onRevoke { Button("Revoke", action: onRevoke).font(HFont.captionStrong).foregroundStyle(Sage.clay).frame(minHeight: 44) }
        }
        .padding(.horizontal, 12).frame(minHeight: 56)
    }

    private var shareSheet: some View {
        let available = contacts.filter { c in !(item?.accessList.contains { $0.granteeId == c.id } ?? false) }
        return NavigationStack {
            List {
                if available.isEmpty {
                    Text("Everyone in your contacts already has access. Add a doctor or hospital from Search first.").font(HFont.body)
                }
                ForEach(available) { c in
                    Button {
                        Task { if let u = try? await env.repo.share(itemId, with: c.id) { item = u }; showShare = false }
                    } label: {
                        HStack(spacing: 12) {
                            Avatar(initials: c.initials, size: 36)
                            VStack(alignment: .leading) {
                                Text(c.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                                Text(c.primaryRole == .hospital ? "All its doctors" : c.headline).font(HFont.small).foregroundStyle(Sage.muted)
                            }
                        }
                    }
                }
            }
            .navigationTitle("Share this item").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { showShare = false } } }
        }
    }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            let it = try await env.repo.item(itemId)
            item = it
            if let p = it.pointerItemId { linked = try? await env.repo.item(p) }
            contacts = (try? await env.repo.connections())?.filter { $0.primaryRole == .doctor || $0.primaryRole == .hospital } ?? []
        } catch { self.error = "Couldn't open this item. You may no longer have access to it." }
    }
}

/// Simple wrapping layout for keyword chips.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(proposal.width ?? .infinity, subviews).size
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (i, p) in arrange(bounds.width, subviews).points.enumerated() {
            subviews[i].place(at: CGPoint(x: bounds.minX + p.x, y: bounds.minY + p.y), proposal: .unspecified)
        }
    }
    private func arrange(_ width: CGFloat, _ subviews: Subviews) -> (points: [CGPoint], size: CGSize) {
        var points: [CGPoint] = [], x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, maxX: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x + size.width > width, x > 0 { x = 0; y += row + spacing; row = 0 }
            points.append(CGPoint(x: x, y: y))
            x += size.width + spacing; row = max(row, size.height); maxX = max(maxX, x - spacing)
        }
        return (points, CGSize(width: maxX, height: y + row))
    }
}

/// Loads an image attachment from the bundle, a local file or a presigned URL.
struct AttachmentImage: View {
    let attachment: Attachment
    var contentMode: ContentMode = .fit
    @State private var image: UIImage?
    @State private var failed = false

    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().aspectRatio(contentMode: contentMode) }
            else if failed { Image(systemName: "photo").foregroundStyle(Sage.muted) }
            else { ProgressView() }
        }
        .task(id: attachment.uri) {
            do {
                let url = try await AttachmentStore.localURL(for: attachment)
                image = UIImage(contentsOfFile: url.path)
                failed = image == nil
            } catch { failed = true }
        }
    }
}

// MARK: - Attachment viewer (design doc 3.6)

/// Swipe left/right through every image and PDF of the item without closing.
/// PDF pages scroll vertically; images zoom with pinch or double-tap.
struct AttachmentViewer: View {
    let title: String
    let attachments: [Attachment]
    @State var index: Int
    @State private var pdfPage: [Int: (Int, Int)] = [:]
    @Environment(\.dismiss) private var dismiss

    init(title: String, attachments: [Attachment], startIndex: Int) {
        self.title = title
        self.attachments = attachments
        _index = State(initialValue: min(max(startIndex, 0), max(attachments.count - 1, 0)))
    }

    var body: some View {
        VStack(spacing: 0) {
            topBar
            TabView(selection: $index) {
                ForEach(Array(attachments.enumerated()), id: \.element.id) { i, a in
                    Group {
                        switch a.kind {
                        case .image: ZoomableImage(attachment: a)
                        case .pdf: PDFAttachment(attachment: a) { page, total in pdfPage[i] = (page, total) }
                        }
                    }
                    .tag(i)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            thumbnails
        }
        .background(Sage.viewerBackground.ignoresSafeArea())
        .statusBarHidden(false)
        .preferredColorScheme(.dark)
    }

    private var topBar: some View {
        HStack(spacing: 8) {
            Button { dismiss() } label: {
                Image(systemName: "xmark").font(.system(size: 17, weight: .semibold)).foregroundStyle(.white).frame(width: 44, height: 44)
            }
            .accessibilityLabel("Close viewer")
            VStack(alignment: .leading, spacing: 1) {
                let a = attachments[index]
                Text(a.name).font(HFont.bodyStrong).foregroundStyle(.white).lineLimit(1)
                Text(pdfPage[index].map { "Page \($0.0) of \($0.1)" } ?? (a.kind == .image ? "Image" : "PDF"))
                    .font(HFont.small).foregroundStyle(Sage.onPrimaryLine)
            }
            Spacer()
            Text("\(index + 1) / \(attachments.count)").font(HFont.bodyStrong).foregroundStyle(.white).padding(.trailing, 12)
                .accessibilityLabel("Attachment \(index + 1) of \(attachments.count)")
        }
        .padding(.horizontal, 8).padding(.vertical, 4)
    }

    private var thumbnails: some View {
        ScrollViewReader { proxy in
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(Array(attachments.enumerated()), id: \.element.id) { i, a in
                        Button { withAnimation { index = i } } label: {
                            ZStack {
                                Color.white.opacity(0.1)
                                if a.kind == .image { AttachmentImage(attachment: a, contentMode: .fill) }
                                else { Image(systemName: "doc.richtext").foregroundStyle(.white) }
                            }
                            .frame(width: 56, height: 56).clipShape(RoundedRectangle(cornerRadius: 10))
                            .overlay(RoundedRectangle(cornerRadius: 10).stroke(i == index ? .white : .clear, lineWidth: 2))
                        }
                        .id(i)
                        .accessibilityLabel("Go to \(a.name)")
                    }
                }
                .padding(.horizontal, 16).padding(.vertical, 10)
            }
            .onChange(of: index) { withAnimation { proxy.scrollTo(index, anchor: .center) } }
            .onAppear { proxy.scrollTo(index, anchor: .center) }
        }
    }
}

private struct ZoomableImage: View {
    let attachment: Attachment
    @State private var scale: CGFloat = 1
    @State private var lastScale: CGFloat = 1
    @State private var offset: CGSize = .zero
    @State private var lastOffset: CGSize = .zero

    var body: some View {
        AttachmentImage(attachment: attachment)
            .scaleEffect(scale)
            .offset(offset)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(
                MagnifyGesture()
                    .onChanged { scale = min(max(lastScale * $0.magnification, 1), 5) }
                    .onEnded { _ in lastScale = scale; if scale == 1 { reset() } }
            )
            // Drag only pans while zoomed; at 1× it isn't attached, so the swipe goes to the pager.
            .highPriorityGesture(
                DragGesture()
                    .onChanged { offset = CGSize(width: lastOffset.width + $0.translation.width, height: lastOffset.height + $0.translation.height) }
                    .onEnded { _ in lastOffset = offset },
                including: scale > 1 ? .all : .none
            )
            .onTapGesture(count: 2) { withAnimation(.spring(duration: 0.25)) { if scale > 1 { reset() } else { scale = 2.5; lastScale = 2.5 } } }
            .accessibilityLabel(attachment.name)
            .accessibilityHint("Double-tap to zoom")
    }

    private func reset() { scale = 1; lastScale = 1; offset = .zero; lastOffset = .zero }
}

private struct PDFAttachment: View {
    let attachment: Attachment
    let onPage: (Int, Int) -> Void
    @State private var url: URL?
    @State private var failed = false

    var body: some View {
        Group {
            if let url { PDFKitView(url: url, onPage: onPage) }
            else if failed {
                Text("Couldn't open \(attachment.name). Swipe to the next file or try again later.")
                    .font(HFont.body).foregroundStyle(.white).multilineTextAlignment(.center).padding(24)
            } else { ProgressView().tint(.white) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task(id: attachment.uri) {
            do { url = try await AttachmentStore.localURL(for: attachment) } catch { failed = true }
        }
    }
}

/// PDFKit in continuous vertical mode; reports the visible page.
private struct PDFKitView: UIViewRepresentable {
    let url: URL
    let onPage: (Int, Int) -> Void

    func makeUIView(context: Context) -> PDFView {
        let view = PDFView()
        view.displayMode = .singlePageContinuous
        view.displayDirection = .vertical
        view.autoScales = true
        view.pageShadowsEnabled = false
        view.backgroundColor = UIColor(Sage.viewerBackground)
        view.document = PDFDocument(url: url)
        context.coordinator.attach(view)
        return view
    }

    func updateUIView(_ view: PDFView, context: Context) {
        if view.document?.documentURL != url { view.document = PDFDocument(url: url) }
    }

    func makeCoordinator() -> Coordinator { Coordinator(onPage: onPage) }

    final class Coordinator: NSObject {
        let onPage: (Int, Int) -> Void
        private weak var view: PDFView?
        init(onPage: @escaping (Int, Int) -> Void) { self.onPage = onPage }

        func attach(_ view: PDFView) {
            self.view = view
            NotificationCenter.default.addObserver(self, selector: #selector(changed), name: .PDFViewPageChanged, object: view)
            DispatchQueue.main.async { self.changed() }
        }

        @objc func changed() {
            guard let view, let doc = view.document, let page = view.currentPage else { return }
            onPage(doc.index(for: page) + 1, doc.pageCount)
        }
    }
}
