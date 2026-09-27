import SwiftUI
import PDFKit

// MARK: - Data view (10% pinned header): one DataItem with its child lists (DataItem_Design.md 8)

struct DataItemView: View {
    let itemId: String
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL
    @State private var item: DataItem?
    @State private var me: UserProfile?
    @State private var contacts: [UserProfile] = []
    @State private var error: String?
    @State private var actionError: String?
    @State private var viewerIndex: ViewerStart?
    @State private var confirmRevoke: Grant?
    @State private var sheet: Sheet?
    @State private var openDiscussion = false

    struct ViewerStart: Identifiable { let index: Int; var id: Int { index } }
    enum Sheet: Identifiable {
        case share, book, alert, close, addFiles, move(Appointment, Visit)
        var id: String {
            switch self {
            case .share: "share"; case .book: "book"; case .alert: "alert"; case .close: "close"; case .addFiles: "files"
            case .move(let a, let v): "move-\(a.id)-\(v.originalDate)"
            }
        }
    }

    private var doctors: [UserProfile] { contacts.filter { $0.primaryRole == .doctor } + (me?.primaryRole == .doctor ? [me!] : []) }

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: item?.title ?? "", subtitle: item.map { "\($0.primaryKind.label) · added by \($0.createdByName)" } ?? "",
                         onBack: { dismiss() }) {
                if let me { Avatar(initials: me.initials, size: 36, photoUrl: me.photoUri) }
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let error { ErrorView(message: error) { Task { await load() } } }
                    else if let item { details(item) }
                    else { LoadingView() }
                }
                .padding(20)
            }
            .refreshable { await load() }
        }
        .background(Sage.background)
        .bottomActionBar {
            if let item {
                if item.can("message") || !item.messages.isEmpty {
                    Button(item.messages.isEmpty ? "Start discussion" : "Discussion") { openDiscussion = true }.buttonStyle(SecondaryButtonStyle())
                }
                if item.can("share") { Button("Share with…") { sheet = .share }.buttonStyle(PrimaryButtonStyle()) }
                else if item.can("reopen") { Button("Reopen") { run { try await env.repo.reopenItem(itemId) } }.buttonStyle(PrimaryButtonStyle()) }
            }
        }
        .navigationDestination(isPresented: $openDiscussion) { DiscussionView(itemId: itemId).toolbar(.hidden, for: .navigationBar) }
        .fullScreenCover(item: $viewerIndex) { start in
            if let item { AttachmentViewer(title: item.title, attachments: item.sortedAttachments, startIndex: start.index) }
        }
        .confirmationDialog("Stop sharing with \(confirmRevoke?.granteeName ?? "")?", isPresented: Binding(
            get: { confirmRevoke != nil }, set: { if !$0 { confirmRevoke = nil } }), titleVisibility: .visible) {
            Button("Stop sharing", role: .destructive) {
                if let g = confirmRevoke { run { try await env.repo.revoke(itemId, grantId: g.grantId) } }
            }
            Button("Keep sharing", role: .cancel) {}
        } message: { Text("They will lose access to the whole item straight away: files, discussion and appointments. You can share it again later.") }
        .sheet(item: $sheet) { s in sheetView(s) }
        .onReceive(env.repo.events) { event in
            switch event {
            case .itemChanged(let id) where id == itemId: Task { await refresh() }
            case .newMessage(let m) where m.itemId == itemId: Task { await refresh() }
            default: break
            }
        }
        .task { if item == nil { await load() } }
    }

    // MARK: Sections

    @ViewBuilder
    private func details(_ item: DataItem) -> some View {
        let isOwner = item.ownerId == me?.id
        statusRow(item)
        if let actionError { Text(actionError).font(HFont.small).foregroundStyle(Sage.clay) }
        if let c = item.closure { closureCard(c) }
        if item.can("meta") {
            Text("You can see this item's details and appointment times. Its files and discussion are shared with doctors only.")
                .font(HFont.small).foregroundStyle(Sage.muted)
                .padding(12).frame(maxWidth: .infinity, alignment: .leading).background(Sage.sandTint, in: RoundedRectangle(cornerRadius: Radius.card))
        }
        if !item.attachments.isEmpty || item.can("attach") { attachmentsSection(item) }
        if item.can("message") || !item.messages.isEmpty { discussionSection(item) }
        if !item.appointments.isEmpty || item.can("book") { appointmentsSection(item) }
        if !item.alerts.isEmpty || item.can("alert") { alertsSection(item, isOwner: isOwner) }
        if !item.links.isEmpty { linksSection(item.links) }
        if !item.keywords.isEmpty { keywordsSection(item.keywords) }
        if isOwner { accessSection(item) }
    }

    private func statusRow(_ item: DataItem) -> some View {
        HStack(spacing: 8) {
            pill(item.primaryKind.label, item.primaryKind.style.tint, item.primaryKind.style.fg)
            ForEach(item.kinds.filter { $0 != item.primaryKind.rawValue && $0 != PartKind.attachment }.prefix(2), id: \.self) { k in
                pill(k.capitalized, Sage.sunken, Sage.inkSoft)
            }
            Spacer()
            let open = item.status == .open
            let canChange = open ? item.can("close") : item.can("reopen")
            Button {
                if open { sheet = .close } else { run { try await env.repo.reopenItem(itemId) } }
            } label: {
                HStack(spacing: 6) {
                    Circle().fill(open ? Sage.accent : Sage.muted).frame(width: 8, height: 8)
                    Text(open ? "Open" : "Closed").font(HFont.smallStrong)
                }
                .foregroundStyle(open ? Sage.accent : Sage.muted).padding(.horizontal, 12).frame(minHeight: 36)
                .overlay(Capsule().stroke(open ? Sage.accent : Sage.border))
            }
            .disabled(!canChange)
            .accessibilityLabel(open ? "Status open. Double-tap to close" : "Status closed. Double-tap to reopen")
        }
    }

    private func closureCard(_ c: Closure) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel("Closed \(DateText.long(c.closedAt))")
            if let r = c.rating {
                HStack(spacing: 2) {
                    ForEach(1...5, id: \.self) { n in Image(systemName: n <= r ? "star.fill" : "star").foregroundStyle(Sage.sand) }
                }
                .accessibilityElement().accessibilityLabel("Rated \(r) of 5")
            }
            if let f = c.feedback { Text(f).font(HFont.body).foregroundStyle(Sage.ink) }
            if c.rating == nil && c.feedback == nil { Text("No feedback was left.").font(HFont.small).foregroundStyle(Sage.muted) }
        }
        .padding(14).frame(maxWidth: .infinity, alignment: .leading)
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
    }

    private func attachmentsSection(_ item: DataItem) -> some View {
        let files = item.sortedAttachments
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                FieldLabel("Attachments · \(files.count)")
                Spacer()
                if item.can("attach") { Button("Add files") { sheet = .addFiles }.font(HFont.captionStrong).foregroundStyle(Sage.accent) }
            }
            if files.isEmpty {
                Text("No files yet.").font(HFont.small).foregroundStyle(Sage.muted)
            } else {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 10) {
                        ForEach(Array(files.enumerated()), id: \.element.id) { i, a in
                            Button { viewerIndex = ViewerStart(index: i) } label: { attachmentCard(a) }
                                .buttonStyle(.plain)
                                .accessibilityLabel("Open \(a.name), \(i + 1) of \(files.count)")
                        }
                    }
                }
            }
            if files.contains(where: \.report) {
                Text("Report files open for the patient who owns this item and for doctors it is shared with.").font(HFont.small).foregroundStyle(Sage.muted)
            }
        }
    }

    private func discussionSection(_ item: DataItem) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Discussion · \(item.messages.count)" + (item.counts.unreadMessages > 0 ? " · \(item.counts.unreadMessages) new" : ""))
            Button { openDiscussion = true } label: {
                VStack(alignment: .leading, spacing: 8) {
                    if item.messages.isEmpty {
                        Text("No messages yet. Everyone this item is shared with can take part.").font(HFont.small).foregroundStyle(Sage.muted)
                    }
                    ForEach(item.messages.suffix(2)) { m in
                        VStack(alignment: .leading, spacing: 1) {
                            Text(senderName(item, m.senderId)).font(HFont.smallStrong).foregroundStyle(Sage.accent)
                            Text(m.body).font(HFont.body).foregroundStyle(Sage.ink).lineLimit(2).multilineTextAlignment(.leading)
                        }
                    }
                    Text(item.messages.isEmpty ? "Start discussion ›" : "Open discussion ›").font(HFont.captionStrong).foregroundStyle(Sage.accent)
                }
                .padding(14).frame(maxWidth: .infinity, alignment: .leading)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
            }
            .buttonStyle(.plain)
        }
    }

    private func appointmentsSection(_ item: DataItem) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                FieldLabel("Appointments · \(item.appointments.count)")
                Spacer()
                if item.can("book") { Button("Book") { sheet = .book }.font(HFont.captionStrong).foregroundStyle(Sage.accent) }
            }
            if item.appointments.isEmpty { Text("No appointments yet.").font(HFont.small).foregroundStyle(Sage.muted) }
            ForEach(item.appointments) { a in appointmentCard(item, a) }
        }
    }

    private func appointmentCard(_ item: DataItem, _ a: Appointment) -> some View {
        let manage = item.can("book") && a.status != "CANCELLED"
        let count = a.recurrence != nil ? (a.visitCount.map { " · \($0) visits" } ?? "") : ""
        return VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(a.doctorName.isEmpty ? "Doctor" : a.doctorName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                    Text((a.recurrence?.label ?? "Single visit") + count + (a.status == "CANCELLED" ? " · cancelled" : ""))
                        .font(HFont.small).foregroundStyle(Sage.muted)
                    if let n = a.notes { Text(n).font(HFont.small).foregroundStyle(Sage.inkSoft) }
                }
                Spacer()
                if manage && a.recurrence != nil {
                    Menu {
                        Button("Cancel whole series", role: .destructive) { run { try await env.repo.cancelAppointment(itemId, appointmentId: a.id) } }
                    } label: { Image(systemName: "ellipsis").foregroundStyle(Sage.muted).frame(width: 44, height: 44) }
                    .accessibilityLabel("Appointment options")
                }
            }
            .padding(.horizontal, 14).padding(.top, 12).padding(.bottom, 6)
            if a.visits.isEmpty {
                Text(a.status == "CANCELLED" ? "No upcoming visits." : "No more upcoming visits.").font(HFont.small).foregroundStyle(Sage.muted)
                    .padding(.horizontal, 14).padding(.bottom, 12)
            }
            ForEach(Array(a.visits.prefix(4))) { v in
                RowDivider()
                HStack {
                    Text(visitLabel(v)).font(HFont.caption).foregroundStyle(v.status == "SCHEDULED" ? Sage.ink : Sage.muted)
                    Spacer()
                    if manage && v.status == "SCHEDULED" {
                        Menu {
                            Button("Move this visit") { sheet = .move(a, v) }
                            Button("Cancel this visit", role: .destructive) { visit(a, v, "CANCELLED") }
                            if me?.id == a.doctorId && v.date <= DateText.today() {
                                Button("Mark attended") { visit(a, v, "COMPLETED") }
                                Button("Mark missed") { visit(a, v, "NO_SHOW") }
                            }
                        } label: { Image(systemName: "ellipsis.circle").foregroundStyle(Sage.accent).frame(width: 44, height: 44) }
                        .accessibilityLabel("Options for visit on \(DateText.long(v.date))")
                    }
                }
                .padding(.leading, 14).frame(minHeight: 44)
            }
            if a.visits.count > 4 {
                RowDivider()
                Text("+ \(a.visits.count - 4) more upcoming").font(HFont.small).foregroundStyle(Sage.muted).padding(14)
            }
        }
        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
    }

    private func visitLabel(_ v: Visit) -> String {
        let state: String = switch v.status {
        case "CANCELLED": " · cancelled"; case "COMPLETED": " · attended"; case "NO_SHOW": " · missed"; default: ""
        }
        return "\(DateText.long(v.date)) · \(v.time)" + (v.originalDate != v.date ? " (moved)" : "") + state
    }

    private func alertsSection(_ item: DataItem, isOwner: Bool) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                FieldLabel("Alerts · \(item.alerts.count)")
                Spacer()
                if item.can("alert") { Button("Add alert") { sheet = .alert }.font(HFont.captionStrong).foregroundStyle(Sage.accent) }
            }
            if item.alerts.isEmpty { Text("No alerts.").font(HFont.small).foregroundStyle(Sage.muted) }
            if !item.alerts.isEmpty {
                GroupCard {
                    ForEach(Array(item.alerts.enumerated()), id: \.element.id) { i, al in
                        if i > 0 { RowDivider() }
                        HStack(spacing: 12) {
                            Image(systemName: "bell").font(.system(size: 15)).foregroundStyle(Sage.clay)
                                .frame(width: 32, height: 32).background(Sage.clayTint, in: RoundedRectangle(cornerRadius: 10))
                            VStack(alignment: .leading, spacing: 1) {
                                Text(al.text).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                                Text("\(DateText.clock(al.firesAt)) · \(al.recurrence?.label ?? "once on \(DateText.long(al.firesAt))")" + (al.active ? "" : " · stopped"))
                                    .font(HFont.small).foregroundStyle(Sage.muted)
                            }
                            Spacer()
                            if item.can("alert") && (isOwner || al.forUser == me?.id) && al.type != "APPOINTMENT_REMINDER" {
                                Button { run { try await env.repo.deleteAlert(itemId, alertId: al.id) } } label: {
                                    Image(systemName: "trash").foregroundStyle(Sage.clay).frame(width: 44, height: 44)
                                }
                                .accessibilityLabel("Delete alert \(al.text)")
                            }
                        }
                        .padding(.horizontal, 12).frame(minHeight: 56)
                    }
                }
            }
        }
    }

    private func linksSection(_ links: [String]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Links")
            GroupCard {
                ForEach(Array(links.enumerated()), id: \.offset) { i, link in
                    if i > 0 { RowDivider() }
                    Button { if let u = URL(string: link) { openURL(u) } } label: {
                        Label(link, systemImage: "link").font(HFont.caption).foregroundStyle(Sage.accent).lineLimit(1)
                            .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading).padding(.horizontal, 14)
                    }
                }
            }
        }
    }

    private func keywordsSection(_ keywords: [String]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Keywords")
            FlowLayout(spacing: 8) {
                ForEach(keywords, id: \.self) {
                    Text($0).font(HFont.caption).foregroundStyle(Sage.ink).padding(.horizontal, 12).padding(.vertical, 7)
                        .background(Sage.surface, in: Capsule()).overlay(Capsule().stroke(Sage.border))
                }
            }
        }
    }

    private func accessSection(_ item: DataItem) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Who can see this")
            GroupCard {
                accessRow("person", "You (owner)", "Full control", nil)
                ForEach(item.accessList) { g in
                    RowDivider()
                    let hospital = g.granteeType == .hospital
                    accessRow(hospital ? "cross.case" : "person", g.granteeName,
                              hospital ? "All affiliated doctors" : (g.viaHospitalId != nil ? "Via hospital" : "Shared directly"),
                              item.can("revoke") ? { confirmRevoke = g } : nil)
                }
            }
            Text("Sharing covers everything in this item: files, discussion, appointments and alerts.").font(HFont.small).foregroundStyle(Sage.muted)
        }
    }

    // MARK: Helpers

    private func senderName(_ item: DataItem, _ senderId: String) -> String {
        if senderId == me?.id { return "You" }
        if let g = item.accessList.first(where: { $0.granteeId == senderId }) { return g.granteeName }
        if let a = item.appointments.first(where: { $0.doctorId == senderId }), !a.doctorName.isEmpty { return a.doctorName }
        if senderId == item.ownerId { return "Patient" }
        return contacts.first { $0.id == senderId }?.displayName ?? "Care team"
    }

    private func pill(_ text: String, _ bg: Color, _ fg: Color) -> some View {
        Text(text).font(HFont.smallStrong).foregroundStyle(fg).padding(.horizontal, 12).padding(.vertical, 6).background(bg, in: Capsule())
    }

    private func attachmentCard(_ a: Attachment) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack {
                Sage.preview
                AttachmentPreview(attachment: a)
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
            Image(systemName: symbol).font(.system(size: 15)).foregroundStyle(Sage.accent)
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

    @ViewBuilder
    private func sheetView(_ s: Sheet) -> some View {
        switch s {
        case .share: shareSheet.presentationDetents([.medium, .large])
        case .book:
            BookAppointmentSheet(patientId: item?.ownerId ?? "", doctors: doctors, defaultDoctor: me?.primaryRole == .doctor ? me?.id : nil) { a in
                item = try await env.repo.bookAppointment(itemId, a)
            }
        case .alert: AddAlertSheet { a in item = try await env.repo.addAlert(itemId, a) }
        case .close: CloseItemSheet(canRate: item?.can("rate") ?? false) { f, r in item = try await env.repo.closeItem(itemId, feedback: f, rating: r) }
        case .move(let a, let v):
            MoveVisitSheet(visit: v) { d, t in
                item = try await env.repo.visitAction(itemId, appointmentId: a.id, visitDate: v.originalDate, action: "MOVED", newDate: d, newTime: t)
            }
        case .addFiles:
            NavigationStack { UploadView(request: nil, addToItemId: itemId) { _ in sheet = nil; Task { await refresh() } } }
        }
    }

    private var shareSheet: some View {
        let available = contacts.filter { c in c.primaryRole == .doctor || c.primaryRole == .hospital }
            .filter { c in !(item?.accessList.contains { $0.granteeId == c.id } ?? false) }
        return NavigationStack {
            List {
                if available.isEmpty {
                    Text("Everyone in your contacts already has access. Add a doctor or hospital from Search first.").font(HFont.body)
                }
                ForEach(available) { c in
                    Button {
                        run { try await env.repo.share(itemId, with: c.id) }
                        sheet = nil
                    } label: {
                        HStack(spacing: 12) {
                            Avatar(initials: c.initials, size: 36, photoUrl: c.photoUri)
                            VStack(alignment: .leading) {
                                Text(c.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                                Text(c.primaryRole == .hospital ? "All its doctors" : c.headline).font(HFont.small).foregroundStyle(Sage.muted)
                            }
                        }
                    }
                }
            }
            .navigationTitle("Share this item").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { sheet = nil } } }
        }
    }

    private func visit(_ a: Appointment, _ v: Visit, _ action: String) {
        run { try await env.repo.visitAction(itemId, appointmentId: a.id, visitDate: v.originalDate, action: action, newDate: nil, newTime: nil) }
    }

    /// Runs a change that returns the updated item; shows the server's reason if it fails.
    private func run(_ f: @escaping () async throws -> DataItem) {
        actionError = nil
        Task {
            do { item = try await f() }
            catch { actionError = error.localizedDescription }
        }
    }

    private func refresh() async { if let it = try? await env.repo.item(itemId) { item = it } }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            item = try await env.repo.item(itemId)
            contacts = (try? await env.repo.connections()) ?? []
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
/// `thumbnail: true` (cards, thumbnail strip) loads the small server thumbnail and falls back to
/// the full image only when there is none, so opening an item doesn't download every photo.
struct AttachmentImage: View {
    let attachment: Attachment
    var contentMode: ContentMode = .fit
    var thumbnail = false
    @State private var image: UIImage?
    @State private var failed = false

    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().aspectRatio(contentMode: contentMode) }
            else if failed { Image(systemName: "photo").foregroundStyle(Sage.muted) }
            else { ProgressView() }
        }
        .task(id: "\(attachment.uri)|\(thumbnail)") {
            do {
                var url: URL?
                if thumbnail { url = try? await AttachmentStore.thumbnailURL(for: attachment) }
                // No thumbnail: only an image falls back to its full file (never download a whole PDF for a card).
                if url == nil {
                    guard attachment.kind == .image else { failed = true; return }
                    url = try await AttachmentStore.localURL(for: attachment)
                }
                image = url.flatMap { UIImage(contentsOfFile: $0.path) }
                failed = image == nil
            } catch { failed = true }
        }
    }
}

/// Preview for a card or the viewer's strip: the thumbnail for images and PDFs; a PDF with no
/// thumbnail yet shows an icon (the PDF itself downloads only when opened).
struct AttachmentPreview: View {
    let attachment: Attachment
    var iconSize: CGFloat = 30
    var iconColor: Color = Sage.muted

    var body: some View {
        if attachment.kind == .image {
            AttachmentImage(attachment: attachment, contentMode: .fill, thumbnail: true)
        } else if attachment.thumbUri?.isEmpty == false {
            // A page is taller than the card: show its top, where the heading is.
            AttachmentImage(attachment: attachment, contentMode: .fill, thumbnail: true)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top).clipped()
        } else {
            Image(systemName: "doc.richtext").font(.system(size: iconSize)).foregroundStyle(iconColor)
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
                                AttachmentPreview(attachment: a, iconSize: 17, iconColor: .white)
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
