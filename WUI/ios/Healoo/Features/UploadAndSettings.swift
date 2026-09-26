import SwiftUI
import PhotosUI
import UniformTypeIdentifiers

// MARK: - Upload (10% pinned header; several images and PDFs in one item)

struct UploadView: View {
    let targetUserId: String?
    /// When set, the screen only adds files to this existing item.
    let addToItemId: String?
    /// Called with the created (or changed) item's id.
    let onDone: (String) -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(Router.self) private var router
    @Environment(\.dismiss) private var dismiss

    @State private var me: UserProfile?
    @State private var owner: UserProfile?
    @State private var patients: [UserProfile] = []      // doctor/lab flow: connected patients
    @State private var contacts: [UserProfile] = []

    /// New item: the primary part it starts from (messages start from a person's page).
    @State private var kind: PrimaryKind = .report
    @State private var title = ""
    @State private var keywords = ""
    @State private var links: [String] = []
    @State private var files: [PendingAttachment] = []
    @State private var shareWith: Set<String> = []
    @State private var isReport = true                   // add-files mode: are these report files?

    // Appointment
    @State private var doctorId: String?
    @State private var apptDate = Calendar.current.date(byAdding: .day, value: 1, to: Date())!
    @State private var apptTime = Calendar.current.date(bySettingHour: 10, minute: 0, second: 0, of: Date())!
    @State private var apptFrequency: Frequency?
    @State private var apptPeriod: Period?
    @State private var apptNotes = ""
    // Alert
    @State private var alertType = "MEDICATION"
    @State private var alertText = ""
    @State private var alertDate = Date()
    @State private var alertTime = Calendar.current.date(bySettingHour: 20, minute: 0, second: 0, of: Date())!
    @State private var alertFrequency: Frequency? = .daily
    @State private var alertPeriod: Period? = .oneMonth

    @State private var photoItems: [PhotosPickerItem] = []
    @State private var showPhotos = false
    @State private var showPDFs = false
    @State private var showCamera = false
    @State private var showLink = false
    @State private var newLink = "https://"
    @State private var message: String?
    @State private var busy = false

    private var addingFiles: Bool { addToItemId != nil }
    private var doctors: [UserProfile] { contacts.filter { $0.primaryRole == .doctor } + (me?.primaryRole == .doctor ? [me!] : []) }

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: addingFiles ? "Add files" : "New item",
                         subtitle: addingFiles ? "To this item" : "For \(owner?.displayName ?? "…") · \(kind.label)",
                         backSymbol: "xmark", backLabel: "Cancel", onBack: cancel) {
                if let owner, !addingFiles { Avatar(initials: owner.initials, size: 36) }
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if addingFiles {
                        attachSection
                        if !files.isEmpty { pendingList }
                        Toggle(isOn: $isReport) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("These are report files").font(HFont.body).foregroundStyle(Sage.ink)
                                Text("Report files open only for doctors the item is shared with.").font(HFont.small).foregroundStyle(Sage.muted)
                            }
                        }
                        .tint(Sage.primary)
                    } else {
                        if me?.isClinical == true && kind != .alert { patientPicker }
                        VStack(alignment: .leading, spacing: 8) {
                            FieldLabel("Start with")
                            HStack(spacing: 8) {
                                ForEach([PrimaryKind.report, .appointment, .alert]) { k in
                                    SageChip(label: k.label, selected: kind == k) { kind = k; message = nil }.frame(maxWidth: .infinity)
                                }
                            }
                            Text("You can add files, messages, appointments and alerts to the item later.").font(HFont.small).foregroundStyle(Sage.muted)
                        }
                        switch kind {
                        case .report:
                            attachSection
                            if !files.isEmpty || !links.isEmpty { pendingList }
                            textField("Title", text: $title, placeholder: "e.g. Home BP readings, September")
                        case .appointment:
                            AppointmentFields(doctors: doctors, doctorId: $doctorId, date: $apptDate, time: $apptTime,
                                              frequency: $apptFrequency, period: $apptPeriod, notes: $apptNotes)
                        case .alert:
                            textField("Title (optional)", text: $title, placeholder: "e.g. Evening medicine")
                            AlertFields(type: $alertType, text: $alertText, date: $alertDate, time: $alertTime,
                                        frequency: $alertFrequency, period: $alertPeriod)
                        case .message:
                            EmptyView()
                        }
                        textField("Keywords", text: $keywords, placeholder: "Separate with commas, e.g. BP, home readings")
                        if uploadingForSomeoneElse && kind != .alert {
                            Text("\(owner?.displayName ?? "The patient") will own this item and decide who else sees it. You keep access because you created it.")
                                .font(HFont.caption).foregroundStyle(Sage.sandInk).padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(Sage.sandTint, in: RoundedRectangle(cornerRadius: 12))
                        } else {
                            shareSection
                        }
                    }
                    if let message { Text(message).font(HFont.small).foregroundStyle(Sage.clay) }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .bottomActionBar {
            Button(busy ? "Saving…" : actionLabel, action: submit).buttonStyle(PrimaryButtonStyle()).disabled(busy)
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
        .toolbar(.hidden, for: .navigationBar)
        .task { await loadContext() }
    }

    private var actionLabel: String {
        if addingFiles { return files.isEmpty ? "Add files" : "Add \(files.count) file\(files.count == 1 ? "" : "s")" }
        switch kind {
        case .report: return "Upload report"
        case .appointment: return "Book appointment"
        case .alert: return "Set alert"
        case .message: return "Send"
        }
    }

    private func cancel() {
        if addingFiles { dismiss() } else { router.uploadTarget = nil; router.openTab(.home) }
    }

    private var attachSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            FieldLabel("Attach · \(files.count) of \(Limits.maxAttachments)")
            HStack(spacing: 8) {
                attachButton("Camera", "camera") {
                    if UIImagePickerController.isSourceTypeAvailable(.camera) { showCamera = true }
                    else { message = "This device has no camera. Use Images instead." }
                }
                attachButton("Images", "photo.on.rectangle") { showPhotos = true }
                attachButton("PDFs", "doc.richtext") { showPDFs = true }
                if !addingFiles { attachButton("Link", "link") { newLink = "https://"; showLink = true } }
            }
        }
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
        title = ""; keywords = ""; links = []; files = []; shareWith = []; message = nil
        kind = .report; alertText = ""; apptNotes = ""; apptFrequency = nil; apptPeriod = nil
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
            if doctorId == nil { doctorId = self_.primaryRole == .doctor ? self_.id : (doctors.count == 1 ? doctors[0].id : nil) }
        } catch { message = "Couldn't load your contacts. You can still continue." }
    }

    private var keywordList: [String] {
        keywords.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }

    private func submit() {
        // Only the owner decides sharing (doc 2.3); a creator acting for a patient keeps access automatically.
        let share = uploadingForSomeoneElse ? [] : Array(shareWith)
        let work: () async throws -> DataItem
        if let addToItemId {
            guard !files.isEmpty else { message = "Add at least one image or PDF."; return }
            let (f, r) = (files, isReport)
            work = { try await env.repo.addAttachments(addToItemId, files: f, isReport: r) }
        } else {
            switch kind {
            case .report:
                guard let owner else { message = "Choose the patient this report is for."; return }
                guard !files.isEmpty || !links.isEmpty else { message = "Add at least one image, PDF or link."; return }
                let draft = ReportDraft(ownerId: owner.id, title: title.trimmingCharacters(in: .whitespaces), keywords: keywordList,
                                        links: links, shareWith: share)
                let f = files
                work = { try await env.repo.createReport(draft, files: f) }
            case .appointment:
                guard let owner else { message = "Choose the patient this appointment is for."; return }
                if let p = AppointmentFields.problem(doctorId: doctorId, date: apptDate, frequency: apptFrequency, period: apptPeriod) { message = p; return }
                let a = AppointmentFields.build(patientId: owner.id, doctorId: doctorId!, date: apptDate, time: apptTime,
                                                frequency: apptFrequency, period: apptPeriod, notes: apptNotes)
                work = { try await env.repo.createAppointment(ownerId: owner.id, a, shareWith: share) }
            case .alert:
                if let p = AlertFields.problem(text: alertText, date: alertDate, frequency: alertFrequency, period: alertPeriod) { message = p; return }
                let a = AlertFields.build(type: alertType, text: alertText, date: alertDate, time: alertTime, frequency: alertFrequency, period: alertPeriod)
                let t = title.trimmingCharacters(in: .whitespaces), shareAlert = Array(shareWith)
                work = { try await env.repo.createAlert(title: t, a, shareWith: shareAlert) }
            case .message:
                return
            }
        }
        busy = true; message = nil
        Task {
            do {
                let item = try await work()
                busy = false
                resetForm()                 // the tab is empty again when the user comes back
                onDone(item.id)
            } catch {
                busy = false
                message = "That didn't finish: \(error.localizedDescription). Nothing was lost — try again."
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
