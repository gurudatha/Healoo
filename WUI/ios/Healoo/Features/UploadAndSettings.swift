import SwiftUI
import PhotosUI
import UniformTypeIdentifiers

// MARK: - Upload (10% pinned header; several images and PDFs in one item)

struct UploadView: View {
    let targetUserId: String?
    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router

    @State private var me: UserProfile?
    @State private var owner: UserProfile?
    @State private var patients: [UserProfile] = []      // doctor/lab flow: connected patients
    @State private var contacts: [UserProfile] = []
    @State private var threads: [ThreadSummary] = []

    @State private var type: CoreItemType = .report
    @State private var title = ""
    @State private var date = Date()
    @State private var statusIndex = 0
    @State private var keywords = ""
    @State private var links: [String] = []
    @State private var files: [PendingAttachment] = []
    @State private var shareWith: Set<String> = []
    @State private var linkTo: ThreadSummary?

    @State private var photoItems: [PhotosPickerItem] = []
    @State private var showPhotos = false
    @State private var showPDFs = false
    @State private var showCamera = false
    @State private var showLink = false
    @State private var newLink = "https://"
    @State private var message: String?
    @State private var busy = false
    @State private var uploadedItem: String?

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 8), count: 3)

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: "New upload", subtitle: "For \(owner?.displayName ?? "…") · \(type.label)",
                         backSymbol: "xmark", backLabel: "Cancel upload", onBack: { router.uploadTarget = nil; router.openTab(.home) }) {
                if let owner { Avatar(initials: owner.initials, size: 36) }
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if me?.isClinical == true { patientPicker }
                    VStack(alignment: .leading, spacing: 8) {
                        FieldLabel("Item type")
                        LazyVGrid(columns: columns, spacing: 8) {
                            ForEach(CoreItemType.allCases) { t in
                                SageChip(label: t.label, selected: type == t) { type = t }.frame(maxWidth: .infinity)
                            }
                        }
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        FieldLabel("Attach · \(files.count) of \(Limits.maxAttachments)")
                        HStack(spacing: 8) {
                            attachButton("Camera", "camera") {
                                if UIImagePickerController.isSourceTypeAvailable(.camera) { showCamera = true }
                                else { message = "This device has no camera. Use Images instead." }
                            }
                            attachButton("Images", "photo.on.rectangle") { showPhotos = true }
                            attachButton("PDFs", "doc.richtext") { showPDFs = true }
                            attachButton("Link", "link") { newLink = "https://"; showLink = true }
                        }
                        if let message { Text(message).font(HFont.small).foregroundStyle(Sage.clay) }
                    }

                    if !files.isEmpty || !links.isEmpty { pendingList }

                    textField("Title", text: $title, placeholder: "e.g. Home BP readings, September")

                    HStack(alignment: .top, spacing: 10) {
                        VStack(alignment: .leading, spacing: 6) {
                            FieldLabel("Date")
                            DatePicker("Date", selection: $date, displayedComponents: .date).labelsHidden().tint(Sage.primary)
                                .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                        }
                        VStack(alignment: .leading, spacing: 6) {
                            FieldLabel("Status")
                            Segmented(options: ["Open", "Closed"], selection: $statusIndex)
                        }
                    }

                    textField("Keywords", text: $keywords, placeholder: "Separate with commas, e.g. BP, home readings")

                    if uploadingForSomeoneElse {
                        Text("\(owner?.displayName ?? "The patient") will own this record and decide who else sees it. You keep access because you uploaded it.")
                            .font(HFont.caption).foregroundStyle(Sage.sandInk).padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Sage.sandTint, in: RoundedRectangle(cornerRadius: 12))
                    } else {
                        shareSection
                    }
                    linkToSection
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .bottomActionBar {
            Button(busy ? "Uploading…" : "Upload \(type.label.lowercased())", action: upload)
                .buttonStyle(PrimaryButtonStyle()).disabled(busy)
        }
        .photosPicker(isPresented: $showPhotos, selection: $photoItems, maxSelectionCount: Limits.maxAttachments, matching: .images)
        .onChange(of: photoItems) { Task { await importPhotos() } }
        .fileImporter(isPresented: $showPDFs, allowedContentTypes: [.pdf], allowsMultipleSelection: true) { importPDFs($0) }
        .fullScreenCover(isPresented: $showCamera) {
            CameraPicker { image in addImageData(image.jpegData(compressionQuality: 0.85), name: "photo_\(Int(Date().timeIntervalSince1970)).jpg") }
                .ignoresSafeArea()
        }
        .alert("Add a link", isPresented: $showLink) {
            TextField("Web address", text: $newLink).textInputAutocapitalization(.never).keyboardType(.URL)
            Button("Add link") { if newLink.hasPrefix("https://"), newLink.count > 10 { links.append(newLink) } }
            Button("Cancel", role: .cancel) {}
        }
        .navigationDestination(item: $uploadedItem) { DataItemView(itemId: $0).toolbar(.hidden, for: .navigationBar) }
        .toolbar(.hidden, for: .navigationBar)
        .task { await loadContext() }
    }

    // MARK: pieces

    private var uploadingForSomeoneElse: Bool { owner != nil && owner?.id != me?.id }

    private var patientPicker: some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel("Upload for")
            Menu {
                if patients.isEmpty { Text("No connected patients yet — add them from Search") }
                ForEach(patients) { p in Button("\(p.displayName) · \(p.publicId)") { owner = p } }
            } label: {
                HStack(spacing: 10) {
                    if let owner { Avatar(initials: owner.initials, size: 28) }
                    Text(owner.map { "\($0.displayName) · \($0.publicId)" } ?? "Choose a patient")
                        .font(HFont.body).foregroundStyle(owner == nil ? Sage.placeholder : Sage.ink).lineLimit(1)
                    Spacer()
                    if targetUserId == nil { Image(systemName: "chevron.up.chevron.down").foregroundStyle(Sage.muted) }
                }
                .padding(.horizontal, 14).frame(minHeight: 52)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(owner == nil ? Sage.clay : Sage.border))
            }
            .disabled(targetUserId != nil)
        }
    }

    private func attachButton(_ label: String, _ symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 4) {
                Image(systemName: symbol).font(.system(size: 18))
                Text(label).font(HFont.smallStrong)
            }
            .foregroundStyle(Sage.primary).frame(maxWidth: .infinity, minHeight: 60)
            .background(Sage.surface, in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(Sage.dashed, style: StrokeStyle(lineWidth: 1.5, dash: [5, 4])))
        }
        .buttonStyle(.plain)
        .disabled(label != "Link" && files.count >= Limits.maxAttachments)
    }

    private var pendingList: some View {
        GroupCard {
            ForEach(Array(files.enumerated()), id: \.element.id) { i, f in
                if i > 0 { RowDivider() }
                HStack(spacing: 10) {
                    ZStack {
                        Sage.sageTint
                        if f.kind == .image, let img = UIImage(contentsOfFile: f.localURL.path) {
                            Image(uiImage: img).resizable().scaledToFill()
                        } else { Image(systemName: "doc.richtext").foregroundStyle(Sage.primary) }
                    }
                    .frame(width: 40, height: 40).clipShape(RoundedRectangle(cornerRadius: 10))
                    VStack(alignment: .leading, spacing: 1) {
                        Text(f.name).font(HFont.captionStrong).foregroundStyle(Sage.ink).lineLimit(1)
                        Text("\(i + 1). \(f.kind == .pdf ? "PDF" : "Image") · \(f.size / 1024) KB").font(HFont.small).foregroundStyle(Sage.muted)
                    }
                    Spacer(minLength: 0)
                    iconButton("chevron.up", "Move \(f.name) up", enabled: i > 0) { files.swapAt(i, i - 1) }
                    iconButton("chevron.down", "Move \(f.name) down", enabled: i < files.count - 1) { files.swapAt(i, i + 1) }
                    iconButton("xmark", "Remove \(f.name)", tint: Sage.clay) { files.remove(at: i) }
                }
                .padding(.leading, 10).frame(minHeight: 60)
            }
            ForEach(Array(links.enumerated()), id: \.offset) { i, link in
                if !files.isEmpty || i > 0 { RowDivider() }
                HStack(spacing: 10) {
                    Image(systemName: "link").foregroundStyle(Sage.primary)
                    Text(link).font(HFont.caption).foregroundStyle(Sage.ink).lineLimit(1)
                    Spacer(minLength: 0)
                    iconButton("xmark", "Remove link", tint: Sage.clay) { links.remove(at: i) }
                }
                .padding(.leading, 12).frame(minHeight: 52)
            }
        }
    }

    private func iconButton(_ symbol: String, _ label: String, enabled: Bool = true, tint: Color = Sage.muted, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol).foregroundStyle(enabled ? tint : Sage.border).frame(width: 40, height: 44) }
            .disabled(!enabled).accessibilityLabel(label)
    }

    private func textField(_ label: String, text: Binding<String>, placeholder: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel(label)
            TextField(placeholder, text: text, prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
                .font(HFont.body).foregroundStyle(Sage.ink).padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
        }
    }

    private var shareSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Share with")
            if contacts.isEmpty {
                Text("Connect with a doctor or hospital to share with them.").font(HFont.caption).foregroundStyle(Sage.muted)
            } else {
                GroupCard {
                    ForEach(Array(contacts.enumerated()), id: \.element.id) { i, c in
                        if i > 0 { RowDivider() }
                        let on = shareWith.contains(c.id)
                        Button { if on { shareWith.remove(c.id) } else { shareWith.insert(c.id) } } label: {
                            HStack(spacing: 12) {
                                Image(systemName: on ? "checkmark.square.fill" : "square").font(.system(size: 20))
                                    .foregroundStyle(on ? Sage.primary : Sage.muted)
                                Text(c.displayName).font(HFont.body).foregroundStyle(Sage.ink)
                                Spacer()
                                Text(c.primaryRole == .hospital ? "All its doctors" : "Direct").font(HFont.small).foregroundStyle(Sage.muted)
                            }
                            .padding(.horizontal, 14).frame(minHeight: 48)
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(on ? .isSelected : [])
                    }
                }
            }
        }
    }

    private var linkToSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel("Link to message")
            Menu {
                Button("None") { linkTo = nil }
                ForEach(threads) { t in Button("\(t.otherUser.displayName) · “\(t.lastMessage)”") { linkTo = t } }
            } label: {
                HStack {
                    Text(linkTo.map { "\($0.otherUser.displayName) · “\($0.lastMessage)”" } ?? "None")
                        .font(HFont.body).foregroundStyle(Sage.ink).lineLimit(1)
                    Spacer()
                    Image(systemName: "chevron.up.chevron.down").foregroundStyle(Sage.muted)
                }
                .padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
            }
        }
    }

    // MARK: file intake (limits from design doc 4.5)

    private func importPhotos() async {
        let items = photoItems
        photoItems = []
        for item in items {
            let data = try? await item.loadTransferable(type: Data.self)
            let ext = item.supportedContentTypes.first?.preferredFilenameExtension ?? "jpg"
            addImageData(data, name: "image_\(files.count + 1).\(ext)",
                         mime: item.supportedContentTypes.first?.preferredMIMEType ?? "image/jpeg")
        }
    }

    private func addImageData(_ data: Data?, name: String, mime: String = "image/jpeg") {
        guard let data else { message = "Couldn't read one of the images."; return }
        guard files.count < Limits.maxAttachments else { message = "Only \(Limits.maxAttachments) files per item."; return }
        guard Int64(data.count) <= Limits.imageMaxBytes else { message = "\(name) is over 10 MB."; return }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString)-\(name)")
        do {
            try data.write(to: url)
            files.append(PendingAttachment(localURL: url, kind: .image, name: name, mime: mime, size: Int64(data.count)))
            message = nil
        } catch { message = "Couldn't save \(name) for upload." }
    }

    private func importPDFs(_ result: Result<[URL], Error>) {
        guard case .success(let urls) = result else { message = "Couldn't open the selected files."; return }
        var rejected: [String] = []
        for src in urls {
            guard files.count < Limits.maxAttachments else { rejected.append("only \(Limits.maxAttachments) files per item"); break }
            let scoped = src.startAccessingSecurityScopedResource()
            defer { if scoped { src.stopAccessingSecurityScopedResource() } }
            let size = (try? src.resourceValues(forKeys: [.fileSizeKey]).fileSize).flatMap { $0 }.map(Int64.init) ?? 0
            guard size <= Limits.pdfMaxBytes else { rejected.append("\(src.lastPathComponent) is over 25 MB"); continue }
            let dst = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString)-\(src.lastPathComponent)")
            do {
                try FileManager.default.copyItem(at: src, to: dst)
                files.append(PendingAttachment(localURL: dst, kind: .pdf, name: src.lastPathComponent, mime: "application/pdf", size: size))
            } catch { rejected.append("\(src.lastPathComponent) couldn't be copied") }
        }
        message = rejected.isEmpty ? nil : "Some files weren't added: \(rejected.joined(separator: "; "))."
    }

    private func resetForm() {
        title = ""; keywords = ""; links = []; files = []; shareWith = []; linkTo = nil
        statusIndex = 0; date = Date(); type = .report; message = nil
    }

    private func loadContext() async {
        do {
            let self_ = try await env.repo.me()
            me = self_
            let all = try await env.repo.connections()
            patients = all.filter { $0.primaryRole == .patient }
            if let targetUserId { owner = try await env.repo.user(targetUserId) }
            else if !self_.isClinical { owner = self_ }          // clinicians must pick a patient first
            contacts = all.filter { $0.primaryRole == .doctor || $0.primaryRole == .hospital }
            threads = try await env.repo.threads()
        } catch { message = "Couldn't load your contacts. You can still upload." }
    }

    private func upload() {
        guard let owner else { message = "Choose the patient this record is for."; return }
        guard !files.isEmpty || !links.isEmpty else { message = "Add at least one image, PDF or link."; return }
        let f = DateFormatter(); f.dateFormat = "yyyy-MM-dd"; f.locale = Locale(identifier: "en_US_POSIX")
        let draft = UploadDraft(
            ownerId: owner.id, type: type, title: title.trimmingCharacters(in: .whitespaces), date: f.string(from: date),
            keywords: keywords.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty },
            links: links, status: statusIndex == 0 ? .open : .closed,
            // Only the owner decides sharing (doc 2.3); an uploader keeps access automatically.
            shareWith: uploadingForSomeoneElse ? [] : Array(shareWith), pointerToMessage: linkTo?.id)
        busy = true
        Task {
            do {
                let item = try await env.repo.upload(draft, files: files)
                busy = false
                uploadedItem = item.id      // opens the new item
                resetForm()                 // the Upload tab is empty again when the user comes back
            } catch {
                busy = false
                message = "Upload didn't finish: \(error.localizedDescription). Your files are still here — try again."
            }
        }
    }
}

/// UIKit camera wrapper — one photo per capture.
struct CameraPicker: UIViewControllerRepresentable {
    let onImage: (UIImage) -> Void
    @Environment(\.dismiss) private var dismiss

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.delegate = context.coordinator
        return picker
    }
    func updateUIViewController(_ controller: UIImagePickerController, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let parent: CameraPicker
        init(_ parent: CameraPicker) { self.parent = parent }
        func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            if let image = info[.originalImage] as? UIImage { parent.onImage(image) }
            parent.dismiss()
        }
        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { parent.dismiss() }
    }
}

// MARK: - Settings

struct SettingsView: View {
    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @State private var me: UserProfile?
    @State private var prefs: NotificationPrefs?
    @State private var saveError: String?
    @State private var confirmLogout = false

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 14) {
                Text("Settings").font(HFont.screenTitle).foregroundStyle(.white)
                HStack(spacing: 14) {
                    Avatar(initials: me?.initials ?? "", size: 52)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(me?.displayName ?? "").font(.custom(FontName.figtreeSemiBold, size: 17)).foregroundStyle(.white)
                        Text("\(me?.headline ?? "") · \(me?.publicId ?? "")").font(HFont.caption).foregroundStyle(Sage.onPrimarySoft)
                    }
                    Spacer()
                    NavigationLink("Edit", value: Route.editProfile)
                        .font(HFont.captionStrong).foregroundStyle(Sage.primary)
                        .padding(.horizontal, 14).frame(minHeight: 40).background(.white, in: Capsule())
                }
                .padding(12).background(Sage.primaryRaised, in: RoundedRectangle(cornerRadius: 18))
            }
            .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 20)
            .sageHeaderBackground()

            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    FieldLabel("Notifications")
                    if let p = prefs {
                        GroupCard {
                            toggleRow("Messages & alerts", "Push when the app is closed", p.pushMessages) { v in save { $0.pushMessages = v } }
                            RowDivider()
                            toggleRow("New reports", "When a lab uploads for you", p.pushReports) { v in save { $0.pushReports = v } }
                            RowDivider()
                            toggleRow("Quiet hours", "\(p.quietStart) – \(p.quietEnd), no sound", p.quietHours) { v in save { $0.quietHours = v } }
                        }
                    } else { LoadingView() }
                    if let saveError { Text(saveError).font(HFont.small).foregroundStyle(Sage.clay) }
                    FieldLabel("Privacy & sharing")
                    GroupCard {
                        NavigationLink(value: Route.activeSharing) { linkLabel("shield", "Active sharing", nil) }.buttonStyle(.plain)
                        RowDivider()
                        linkRow("person.2", "Contacts", nil) { router.openTab(.search) }
                        RowDivider()
                        linkRow("globe", "Language", "English") {}
                    }
                    Button { confirmLogout = true } label: {
                        Label("Log out", systemImage: "rectangle.portrait.and.arrow.right").font(HFont.bodyStrong).foregroundStyle(Sage.clay)
                            .frame(maxWidth: .infinity, minHeight: 50)
                            .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
                            .overlay(RoundedRectangle(cornerRadius: Radius.card).stroke(Sage.clayBorder))
                    }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .confirmationDialog("Log out of Healoo on this phone?", isPresented: $confirmLogout, titleVisibility: .visible) {
            Button("Log out", role: .destructive) { Task { await env.signOut() } }
            Button("Cancel", role: .cancel) {}
        } message: { Text("You'll stop getting notifications here until you sign in again.") }
        .task {
            me = try? await env.repo.me()
            prefs = try? await env.repo.notificationPrefs()
        }
    }

    /// Optimistic save to PUT /v1/me/notification-prefs; reverts if the server refuses.
    private func save(_ change: (inout NotificationPrefs) -> Void) {
        guard let before = prefs else { return }
        var after = before
        change(&after)
        prefs = after; saveError = nil
        Task {
            do { prefs = try await env.repo.saveNotificationPrefs(after) }
            catch { prefs = before; saveError = "Couldn't save that setting. Try again." }
        }
    }

    private func toggleRow(_ label: String, _ note: String, _ value: Bool, onChange: @escaping (Bool) -> Void) -> some View {
        Toggle(isOn: Binding(get: { value }, set: onChange)) {
            VStack(alignment: .leading, spacing: 1) {
                Text(label).font(HFont.bodyMedium).foregroundStyle(Sage.ink)
                Text(note).font(HFont.small).foregroundStyle(Sage.muted)
            }
        }
        .tint(Sage.primary).padding(.horizontal, 14).frame(minHeight: 56)
    }

    private func linkRow(_ symbol: String, _ label: String, _ value: String?, action: @escaping () -> Void) -> some View {
        Button(action: action) { linkLabel(symbol, label, value) }.buttonStyle(.plain)
    }

    private func linkLabel(_ symbol: String, _ label: String, _ value: String?) -> some View {
            HStack(spacing: 12) {
                Image(systemName: symbol).font(.system(size: 15)).foregroundStyle(Sage.primary)
                    .frame(width: 32, height: 32).background(Sage.sageTint, in: RoundedRectangle(cornerRadius: 10))
                Text(label).font(HFont.bodyMedium).foregroundStyle(Sage.ink)
                Spacer()
                if let value { Text(value).font(HFont.caption).foregroundStyle(Sage.muted) }
                Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold)).foregroundStyle(Sage.muted)
            }
            .padding(.horizontal, 14).frame(minHeight: 52)
            .contentShape(Rectangle())
    }
}
