import SwiftUI

/// Designations offered as chips (`designations` in WUI/app-config.json); any other can be typed.
private let designations = AppConfig.current.designations

/// Hospital administrators: the hospital's profile; add users and doctors, delete (deactivate) doctors.
/// Users can't be deleted (Documentation/Administration_Design.md).
struct AdminView: View {
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var me: UserProfile?
    @State private var doctors: [AdminAccount]?
    @State private var error: String?
    @State private var notice: String?
    @State private var form: Bool?                 // true = doctor, false = user
    @State private var confirm: AdminAccount?
    /// The hospital's profile; hospital administrators only (platform administrators use the web dashboard).
    @State private var hospital: HospitalProfile?
    @State private var hospitalError: String?
    @State private var editingHospital = false

    private let filters = [SearchFilter<AdminAccount>(label: "Active") { $0.active }, SearchFilter(label: "Deleted") { !$0.active }]

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: "Administration", subtitle: me?.hospital ?? "Users and doctors", onBack: { dismiss() }) { EmptyView() }
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    if let notice {
                        HStack {
                            Text(notice).font(HFont.caption).foregroundStyle(Sage.accent)
                            Spacer()
                            Button("OK") { self.notice = nil }.font(HFont.captionStrong).foregroundStyle(Sage.accent)
                        }
                        .padding(12).background(Sage.sageTint, in: RoundedRectangle(cornerRadius: 12))
                    }
                    if me?.primaryRole == .administrator { hospitalCard }
                    HStack(spacing: 10) {
                        addButton("Add user", "person.badge.plus") { form = false }
                        addButton("Add doctor", "stethoscope") { form = true }
                    }
                    Label("Users can't be deleted: they own their health records. Deleting a doctor deactivates the account — they can't sign in and leave the hospital; their past messages and appointments stay.",
                          systemImage: "info.circle")
                        .font(HFont.caption).foregroundStyle(Sage.sandInk).padding(12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Sage.sandTint, in: RoundedRectangle(cornerRadius: 12))
                    SectionHeader(title: "Doctors" + (doctors.map { " · \($0.filter(\.active).count)" } ?? ""))
                    if let error { ErrorView(message: error) { Task { await load() } } }
                    else if let doctors {
                        FilteredSearchBar(
                            placeholder: "Search doctors by name, designation or Healoo ID", candidates: doctors,
                            matches: { d, q in d.displayName.lowercased().contains(q) || d.headline.lowercased().contains(q) || d.publicId.lowercased().contains(q) },
                            onPick: { confirm = $0 }, filters: filters, showAllWhenBlank: true, maxResults: 200,
                            emptyText: doctors.isEmpty ? "No doctors at this hospital yet. Add one above." : "No doctor matches."
                        ) { d in UserResultRow(user: d.profile, trailing: d.active ? "Delete" : "Reactivate") }
                    } else { LoadingView() }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .toolbar(.hidden, for: .navigationBar)
        .task { await load() }
        .sheet(item: Binding(get: { form.map { FormKind(doctor: $0) } }, set: { form = $0?.doctor })) { kind in
            NavigationStack {
                AccountForm(doctor: kind.doctor, hospital: me?.hospital) { account in
                    notice = "Created \(account.displayName) · Healoo ID \(account.publicId). They sign in with this ID or with \(account.email ?? "their email")."
                    form = nil
                    if kind.doctor { Task { await load() } }
                }
            }
            .environment(env)
        }
        .sheet(isPresented: $editingHospital) {
            if let hospital {
                NavigationStack {
                    HospitalForm(hospital: hospital) { saved in
                        self.hospital = saved
                        notice = "Saved \(saved.displayName)'s profile."
                        editingHospital = false
                        Task { me = try? await env.repo.me() }   // the header shows the hospital's name
                    }
                }
                .environment(env)
            }
        }
        .alert(confirm.map { $0.active ? "Delete \($0.displayName)?" : "Reactivate \($0.displayName)?" } ?? "",
               isPresented: Binding(get: { confirm != nil }, set: { if !$0 { confirm = nil } }), presenting: confirm) { d in
            Button(d.active ? "Delete" : "Reactivate", role: d.active ? .destructive : nil) { Task { await setActive(d, !d.active) } }
            Button("Cancel", role: .cancel) {}
        } message: { d in
            Text(d.active ? "They won't be able to sign in and will leave \(me?.hospital ?? "the hospital"). Their past messages, appointments and records stay. You can reactivate them later."
                          : "They can sign in again and rejoin \(me?.hospital ?? "the hospital").")
        }
    }

    private struct FormKind: Identifiable { let doctor: Bool; var id: Bool { doctor } }

    /// Name, headline, address, phone and registration number, with Edit.
    @ViewBuilder private var hospitalCard: some View {
        SectionHeader(title: "Hospital profile")
        if let hospitalError {
            ErrorView(message: hospitalError) { Task { await loadHospital() } }
        } else if let h = hospital {
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(h.displayName).font(HFont.section).foregroundStyle(Sage.ink)
                        if let headline = h.headline { Text(headline).font(HFont.caption).foregroundStyle(Sage.muted) }
                    }
                    Spacer()
                    Button("Edit") { editingHospital = true }.font(HFont.captionStrong).foregroundStyle(Sage.accent)
                }
                detailLine("Address", h.location)
                detailLine("Phone", h.phone)
                detailLine("Registration", h.officialNumber)
                detailLine("Healoo ID", h.publicId)
            }
            .padding(16).frame(maxWidth: .infinity, alignment: .leading)
            .background(Sage.surface, in: RoundedRectangle(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Sage.border))
        } else { LoadingView() }
    }

    private func detailLine(_ label: String, _ value: String?) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Text(label).font(HFont.caption).foregroundStyle(Sage.muted).frame(width: 96, alignment: .leading)
            Text(value ?? "Not set").font(HFont.caption).foregroundStyle(value == nil ? Sage.muted : Sage.inkSoft)
        }
    }

    private func loadHospital() async {
        hospitalError = nil
        do { hospital = try await env.repo.adminHospital() }
        catch { hospitalError = "Couldn't load the hospital profile: \(error.localizedDescription)" }
    }

    private func addButton(_ label: String, _ symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(label, systemImage: symbol).font(HFont.bodyStrong).foregroundStyle(Sage.accent)
                .frame(maxWidth: .infinity, minHeight: 60)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 14))
                .overlay(RoundedRectangle(cornerRadius: 14).stroke(Sage.border))
        }
        .buttonStyle(.plain)
    }

    private func load() async {
        error = nil
        do {
            me = try await env.repo.me()
            doctors = try await env.repo.adminDoctors()
        } catch { self.error = "Couldn't load your hospital's doctors: \(error.localizedDescription)" }
        if me?.primaryRole == .administrator { await loadHospital() }
    }

    private func setActive(_ d: AdminAccount, _ active: Bool) async {
        do {
            if active { _ = try await env.repo.adminReactivateDoctor(d.id) } else { try await env.repo.adminDeactivateDoctor(d.id) }
            notice = active ? "\(d.displayName) is active again." : "\(d.displayName) was deleted (deactivated)."
            await load()
        } catch { notice = "That didn't work: \(error.localizedDescription)" }
    }
}

/// Add a user (patient) or a doctor. Doctors join the administrator's hospital.
private struct AccountForm: View {
    let doctor: Bool
    let hospital: String?
    let onCreated: (AdminAccount) -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var email = ""
    @State private var location = ""
    @State private var designation = ""
    @State private var reg = ""
    @State private var busy = false
    @State private var error: String?

    private var valid: Bool { name.trimmingCharacters(in: .whitespaces).count > (doctor ? 4 : 1) && email.contains("@") }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text(doctor ? "The doctor joins \(hospital ?? "your hospital")." : "A user (patient) owns their records; they can't be deleted later.")
                    .font(HFont.caption).foregroundStyle(Sage.muted)
                field("Full name", $name, doctor ? "e.g. Dr. Asha Verma" : "e.g. Asha Verma")
                field("Email", $email, "Used to link their sign-in").keyboardType(.emailAddress).textInputAutocapitalization(.never)
                if doctor {
                    FieldLabel("Designation")
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(designations, id: \.self) { d in SageChip(label: d, selected: designation == d) { designation = designation == d ? "" : d } }
                        }
                    }
                    field("Or type one", $designation, "e.g. Dermatologist")
                    field("Registration number (optional)", $reg, "")
                }
                field("City (optional)", $location, "")
                if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Saving…" : (doctor ? "Add doctor" : "Add user"), action: save)
                    .buttonStyle(PrimaryButtonStyle()).disabled(busy || !valid)
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle(doctor ? "Add a doctor" : "Add a user").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .onAppear { if doctor && name.isEmpty { name = "Dr. " } }
    }

    private func field(_ label: String, _ text: Binding<String>, _ placeholder: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel(label)
            TextField(placeholder, text: text, prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
                .font(HFont.body).foregroundStyle(Sage.ink).padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
        }
    }

    private func save() {
        func opt(_ s: String) -> String? { let t = s.trimmingCharacters(in: .whitespaces); return t.isEmpty ? nil : t }
        let account = NewAccount(displayName: name.trimmingCharacters(in: .whitespaces), email: email.trimmingCharacters(in: .whitespaces),
                                 location: opt(location), designation: doctor ? opt(designation) : nil, officialNumber: doctor ? opt(reg) : nil)
        busy = true; error = nil
        Task {
            do {
                let created: AdminAccount
                if doctor { created = try await env.repo.adminCreateDoctor(account) } else { created = try await env.repo.adminCreateUser(account) }
                onCreated(created)
            }
            catch { self.error = error.localizedDescription }
            busy = false
        }
    }
}

/// Edit the hospital's name, headline, address, phone and registration number.
private struct HospitalForm: View {
    let hospital: HospitalProfile
    let onSaved: (HospitalProfile) -> Void
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var headline = ""
    @State private var location = ""
    @State private var phone = ""
    @State private var reg = ""
    @State private var busy = false
    @State private var error: String?

    private var phoneOk: Bool { phone.allSatisfy { $0.isNumber || " +-()".contains($0) } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("Patients and doctors see these details on the hospital's profile.").font(HFont.caption).foregroundStyle(Sage.muted)
                field("Hospital name", $name, "", limit: 80)
                field("Headline (optional)", $headline, "e.g. Multi-speciality hospital", limit: 80)
                field("Address (optional)", $location, "Street, area, city", limit: 120)
                field("Phone (optional)", $phone, "e.g. +91 80 4000 1234", limit: 30).keyboardType(.phonePad)
                if !phoneOk { Text("Use digits, spaces and + - ( ) only.").font(HFont.small).foregroundStyle(Sage.clay) }
                field("Registration number (optional)", $reg, "", limit: 40)
                if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                Button(busy ? "Saving…" : "Save", action: save)
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(busy || name.trimmingCharacters(in: .whitespaces).isEmpty || !phoneOk)
            }
            .padding(20)
        }
        .background(Sage.background)
        .navigationTitle("Edit hospital profile").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .onAppear {
            name = hospital.displayName; headline = hospital.headline ?? ""; location = hospital.location ?? ""
            phone = hospital.phone ?? ""; reg = hospital.officialNumber ?? ""
        }
    }

    private func field(_ label: String, _ text: Binding<String>, _ placeholder: String, limit: Int) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel(label)
            TextField(placeholder, text: Binding(get: { text.wrappedValue }, set: { text.wrappedValue = String($0.prefix(limit)) }),
                      prompt: Text(placeholder).foregroundStyle(Sage.placeholder))
                .font(HFont.body).foregroundStyle(Sage.ink).padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
        }
    }

    private func save() {
        func opt(_ s: String) -> String? { let t = s.trimmingCharacters(in: .whitespaces); return t.isEmpty ? nil : t }
        let update = HospitalUpdate(displayName: name.trimmingCharacters(in: .whitespaces), location: opt(location), phone: opt(phone),
                                    officialNumber: opt(reg), headline: opt(headline))
        busy = true; error = nil
        Task {
            do { onSaved(try await env.repo.adminUpdateHospital(update)) }
            catch { self.error = error.localizedDescription }
            busy = false
        }
    }
}
