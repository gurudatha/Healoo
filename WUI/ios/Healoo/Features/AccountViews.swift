import SwiftUI

// MARK: - Sign in (Auth0, developer sign-in, or demo account picker)

struct LoginView: View {
    @Environment(AppEnvironment.self) private var env
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    Image("HealooMark").resizable().scaledToFit().frame(width: 52, height: 52)
                        .accessibilityHidden(true)   // the name next to it is the label
                    Text("Healoo").font(HFont.display(36, relativeTo: .largeTitle)).foregroundStyle(.white)
                }
                Text("Your reports, doctors and messages in one place. You decide who sees what.")
                    .font(HFont.body).foregroundStyle(Sage.onPrimarySoft)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 24).padding(.vertical, 40)
            .sageHeaderBackground()

            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    if let demo = env.repo as? MockRepository {
                        Text("Demo mode — choose who to sign in as").font(HFont.section).foregroundStyle(Sage.ink)
                        Text("Sample data only. Switch accounts from Settings → Log out.").font(HFont.caption).foregroundStyle(Sage.muted)
                        ForEach(demo.demoAccounts) { u in
                            Button {
                                busy = true
                                demo.signIn(as: u.id)
                                Task { try? await env.completeSignIn(); busy = false }
                            } label: {
                                HStack(spacing: 12) {
                                    Avatar(initials: u.initials)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(u.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                                        Text("\(u.headline) · \(u.publicId)").font(HFont.small).foregroundStyle(Sage.muted)
                                    }
                                    Spacer()
                                }
                                .padding(12).background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
                            }
                            .buttonStyle(.plain).disabled(busy)
                        }
                    } else {
                        if env.auth.isConfigured {
                            Button(busy ? "Opening sign-in…" : "Sign in or create account") {
                                busy = true; error = nil
                                Task {
                                    do { try await env.auth.login(); try await env.completeSignIn() }
                                    catch { self.error = "Sign-in didn't finish. Check your connection and try again." }
                                    busy = false
                                }
                            }
                            .buttonStyle(PrimaryButtonStyle()).disabled(busy)
                            Text("You'll sign in on a secure Healoo page, then come back here.").font(HFont.caption).foregroundStyle(Sage.muted)
                        }
                        if env.auth.dev.isEnabled {
                            DevSignInSection(busy: $busy).padding(.top, env.auth.isConfigured ? 12 : 0)
                        }
                        if !env.auth.isConfigured && !env.auth.dev.isEnabled {
                            Text("Sign-in isn't set up in this build. Fill in HealooAuth0Domain and HealooAuth0ClientId in project.yml, set HealooDevSignIn to true for the trial server's test accounts, or set HealooUseFakeData to true for demo data.")
                                .font(HFont.body).foregroundStyle(Sage.clay)
                        }
                    }
                    if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
                }
                .padding(24)
            }
        }
        .background(Sage.background)
    }
}

// MARK: - Developer sign-in (debug builds, trial server with AUTH_MODE=dev)

struct DevSignInSection: View {
    @Environment(AppEnvironment.self) private var env
    @Binding var busy: Bool
    @State private var accounts: [DevAccount]?
    @State private var loadError: String?
    @State private var healooId = ""
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Developer sign-in").font(HFont.section).foregroundStyle(Sage.ink)
            Text("Trial server only. Signs in without Auth0 using the server's test accounts. Not available in release builds.")
                .font(HFont.caption).foregroundStyle(Sage.muted)

            if let loadError {
                Text(loadError).font(HFont.small).foregroundStyle(Sage.clay)
                Button("Try again") { Task { await load() } }
                    .font(HFont.bodyStrong).foregroundStyle(Sage.primary).disabled(busy)
            } else if let accounts {
                if accounts.isEmpty {
                    Text("The server has no test accounts yet. Run care-seed on the server.").font(HFont.small).foregroundStyle(Sage.muted)
                }
                ForEach(accounts) { a in
                    Button { signIn(a.publicId) } label: {
                        HStack(spacing: 12) {
                            Avatar(initials: a.initials)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(a.displayName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                                Text("\(a.roleLabel) · \(a.publicId)").font(HFont.small).foregroundStyle(Sage.muted)
                            }
                            Spacer()
                        }
                        .padding(12).background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))
                    }
                    .buttonStyle(.plain).disabled(busy)
                }
            } else {
                Text("Loading test accounts…").font(HFont.small).foregroundStyle(Sage.muted)
            }

            VStack(alignment: .leading, spacing: 6) {
                FieldLabel("Or enter a Healoo ID")
                TextField("HL-2M9P4", text: $healooId, prompt: Text("HL-2M9P4").foregroundStyle(Sage.placeholder))
                    .font(HFont.body).textInputAutocapitalization(.characters).autocorrectionDisabled()
                    .padding(.horizontal, 14).frame(minHeight: 48)
                    .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                    .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
            }
            Button(busy ? "Signing in…" : "Sign in with this ID") { signIn(healooId) }
                .buttonStyle(PrimaryButtonStyle())
                .disabled(busy || healooId.trimmingCharacters(in: .whitespaces).isEmpty)
            if let error { Text(error).font(HFont.small).foregroundStyle(Sage.clay) }
        }
        .padding(14)
        .background(Sage.sandTint, in: RoundedRectangle(cornerRadius: Radius.card))
        .task { await load() }
    }

    private func load() async {
        loadError = nil
        do { accounts = try await env.auth.dev.accounts() }
        catch { loadError = error.localizedDescription }
    }

    private func signIn(_ id: String) {
        busy = true; error = nil
        Task {
            do { try await env.auth.devLogin(publicId: id); try await env.completeSignIn() }
            catch { self.error = error.localizedDescription }
            busy = false
        }
    }
}

// MARK: - Edit profile

struct EditProfileView: View {
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var me: UserProfile?
    @State private var name = ""
    @State private var location = ""
    @State private var saving = false
    @State private var message: String?

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: "Edit profile", subtitle: me?.publicId ?? "", backSymbol: "xmark", backLabel: "Cancel", onBack: { dismiss() }) { EmptyView() }
            ScrollView {
                if let me {
                    VStack(alignment: .leading, spacing: 16) {
                        VStack(spacing: 10) {
                            QRCodeView(content: HealooQR.content(for: me.publicId)).frame(width: 180, height: 180)
                                .accessibilityLabel("QR code for your Healoo ID \(me.publicId)")
                            Text(me.publicId).font(HFont.section).foregroundStyle(Sage.ink)
                            Text("Others can scan this from Search to find you and add you to their contacts. It shows your profile only, never your records.")
                                .font(HFont.caption).foregroundStyle(Sage.muted).multilineTextAlignment(.center)
                        }
                        .padding(20).frame(maxWidth: .infinity)
                        .background(Sage.surface, in: RoundedRectangle(cornerRadius: Radius.card))

                        field("Name shown to others", $name, "")
                        field("Location", $location, "City or area")

                        if me.primaryRole == .doctor {
                            GroupCard {
                                readOnly("Registration number", me.officialNumber ?? "Not set")
                                RowDivider()
                                readOnly("Current hospital", me.hospital ?? "Independent")
                            }
                            Text("Your hospital administrator updates these.").font(HFont.small).foregroundStyle(Sage.muted)
                        }
                        if let message { Text(message).font(HFont.small).foregroundStyle(Sage.clay) }
                    }
                    .padding(20)
                } else { LoadingView() }
            }
        }
        .background(Sage.background)
        .bottomActionBar {
            Button(saving ? "Saving…" : "Save changes", action: save).buttonStyle(PrimaryButtonStyle()).disabled(saving || me == nil)
        }
        .toolbar(.hidden, for: .navigationBar)
        .task {
            guard me == nil, let u = try? await env.repo.me() else { return }
            me = u; name = u.displayName; location = u.location ?? ""
        }
    }

    private func field(_ label: String, _ text: Binding<String>, _ placeholder: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            FieldLabel(label)
            TextField(placeholder, text: text).font(HFont.body).padding(.horizontal, 14).frame(minHeight: 48)
                .background(Sage.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Sage.border))
        }
    }

    private func readOnly(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label).font(HFont.body).foregroundStyle(Sage.inkSoft)
            Spacer()
            Text(value).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
        }
        .padding(.horizontal, 14).frame(minHeight: 52)
    }

    private func save() {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty else { message = "Enter the name people should see."; return }
        saving = true
        Task {
            do {
                let updated = try await env.repo.updateProfile(ProfileUpdate(displayName: name, location: location.isEmpty ? nil : location))
                env.auth.session = .signedIn(updated)
                dismiss()
            } catch { message = "Your changes weren't saved. Check your connection and try again." }
            saving = false
        }
    }
}

// MARK: - Active sharing overview

struct ActiveSharingView: View {
    @Environment(AppEnvironment.self) private var env
    @Environment(\.dismiss) private var dismiss
    @State private var groups: [ShareGroup]?
    @State private var error: String?
    @State private var confirm: (name: String, grants: [OwnedGrant])?

    var body: some View {
        VStack(spacing: 0) {
            PinnedHeader(title: "Active sharing", subtitle: subtitle, onBack: { dismiss() }) { EmptyView() }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if let error { ErrorView(message: error) { Task { await load() } } }
                    else if let groups {
                        if groups.isEmpty {
                            Text("You haven't shared anything. Open an item and choose Share with… to give a doctor or hospital access.")
                                .font(HFont.body).foregroundStyle(Sage.muted)
                        }
                        ForEach(groups) { g in group(g) }
                    } else { LoadingView() }
                }
                .padding(20)
            }
        }
        .background(Sage.background)
        .toolbar(.hidden, for: .navigationBar)
        .confirmationDialog(confirmTitle, isPresented: Binding(get: { confirm != nil }, set: { if !$0 { confirm = nil } }), titleVisibility: .visible) {
            Button("Stop sharing", role: .destructive) {
                if let grants = confirm?.grants { Task { await revoke(grants) } }
            }
            Button("Keep sharing", role: .cancel) {}
        }
        .task { if groups == nil { await load() } }
    }

    private var subtitle: String {
        guard let groups else { return "" }
        let n = groups.reduce(0) { $0 + $1.grants.count }
        return "\(n) share\(n == 1 ? "" : "s") across \(groups.count) contacts"
    }

    private var confirmTitle: String {
        guard let c = confirm else { return "" }
        return c.grants.count > 1 ? "Stop sharing everything with \(c.name)?" : "Stop sharing “\(c.grants.first?.itemTitle ?? "")” with \(c.name)?"
    }

    private func group(_ g: ShareGroup) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                let hospital = g.granteeType == .hospital
                Image(systemName: hospital ? "cross.case" : "person").font(.system(size: 15)).foregroundStyle(Sage.primary)
                    .frame(width: 32, height: 32).background(Sage.sageTint, in: RoundedRectangle(cornerRadius: 10))
                VStack(alignment: .leading, spacing: 1) {
                    Text(g.granteeName).font(HFont.bodyStrong).foregroundStyle(Sage.ink)
                    Text(hospital ? "All affiliated doctors · \(g.grants.count) items" : "\(g.grants.count) items").font(HFont.small).foregroundStyle(Sage.muted)
                }
                Spacer()
                Button("Stop all") { confirm = (g.granteeName, g.grants) }.font(HFont.captionStrong).foregroundStyle(Sage.clay).frame(minHeight: 44)
            }
            GroupCard {
                ForEach(Array(g.grants.enumerated()), id: \.element.id) { i, grant in
                    if i > 0 { RowDivider() }
                    HStack(spacing: 12) {
                        NavigationLink(value: Route.item(grant.itemId)) {
                            HStack(spacing: 12) {
                                TypeTile(type: grant.primaryKind, size: 32)
                                Text(grant.itemTitle).font(HFont.body).foregroundStyle(Sage.ink).lineLimit(1)
                                Spacer(minLength: 0)
                            }
                        }
                        .buttonStyle(.plain)
                        Button("Revoke") { confirm = (g.granteeName, [grant]) }.font(HFont.captionStrong).foregroundStyle(Sage.clay).frame(minHeight: 44)
                    }
                    .padding(.horizontal, 12).frame(minHeight: 56)
                }
            }
        }
    }

    private func load() async {
        error = nil
        do { groups = try await env.repo.activeShares() } catch { self.error = "Couldn't load what you've shared. Try again." }
    }

    private func revoke(_ grants: [OwnedGrant]) async {
        for g in grants { _ = try? await env.repo.revoke(g.itemId, grantId: g.grantId) }
        await load()
    }
}
