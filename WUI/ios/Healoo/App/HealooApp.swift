import SwiftUI

@main
struct HealooApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var env = AppEnvironment()
    @Environment(\.scenePhase) private var scenePhase

    init() {
        // Sage tab bar: white, primary tint for the selected tab.
        let tab = UITabBarAppearance()
        tab.configureWithOpaqueBackground()
        tab.backgroundColor = .white
        tab.shadowColor = UIColor(Sage.divider)
        UITabBar.appearance().standardAppearance = tab
        UITabBar.appearance().scrollEdgeAppearance = tab
    }

    var body: some Scene {
        WindowGroup {
            SessionGate().environment(env).tint(Sage.primary)
        }
        // Live channel only while in the foreground; push covers the rest (doc 4.4 / 8).
        .onChange(of: scenePhase) { _, phase in
            guard env.auth.me != nil else { return }
            if phase == .active { env.repo.startRealtime() } else if phase == .background { env.repo.stopRealtime() }
        }
    }
}

enum AppTab: Hashable { case home, search, upload, messages, settings }

enum Route: Hashable {
    case item(String)
    case user(String, messages: Bool)
    case search(query: String, role: Role?)
    case editProfile
    case activeSharing
}

/// Chooses between splash, sign-in and the app, based on the Auth0 / demo session.
struct SessionGate: View {
    @Environment(AppEnvironment.self) private var env

    var body: some View {
        Group {
            switch env.auth.session {
            case .checking:
                ZStack { Sage.primary.ignoresSafeArea(); ProgressView().tint(.white) }
            case .signedOut:
                LoginView()
            case .signedIn(let me):
                RootView().id(me.id)          // new account -> fresh navigation and screens
            }
        }
        .task {
            guard env.auth.session == .checking else { return }
            if !env.useFakeData, await env.auth.restore(), (try? await env.completeSignIn()) != nil { return }
            await env.auth.signedOut()
        }
    }
}

@Observable
final class Router {
    var tab: AppTab = .home
    var home = NavigationPath()
    var search = NavigationPath()
    var messages = NavigationPath()
    var settings = NavigationPath()
    var uploadTarget: String? = nil     // doctor/lab flow: the patient the upload is for

    func openTab(_ t: AppTab) { tab = t }

    /// "Upload for <patient>" from a user page.
    func upload(for patientId: String) { uploadTarget = patientId; tab = .upload }

    /// Opened from a notification.
    func open(_ link: DeepLink) {
        tab = .home
        switch link {
        case .item(let id): home.append(Route.item(id))
        case .conversation(let uid): home.append(Route.user(uid, messages: true))
        }
    }
}

struct RootView: View {
    @State private var router = Router()

    var body: some View {
        TabView(selection: $router.tab) {
            NavigationStack(path: $router.home) {
                LandingView().withRoutes()
            }
            .tabItem { Label("Home", systemImage: "house") }.tag(AppTab.home)

            NavigationStack(path: $router.search) {
                SearchView(initialQuery: "", initialRole: nil).withRoutes()
            }
            .tabItem { Label("Search", systemImage: "magnifyingglass") }.tag(AppTab.search)

            NavigationStack {
                UploadView(targetUserId: router.uploadTarget).id(router.uploadTarget ?? "self")
                    .navigationDestination(for: Route.self) { $0.destination.toolbar(.hidden, for: .navigationBar) }
            }
            .tabItem { Label("Upload", systemImage: "square.and.arrow.up") }.tag(AppTab.upload)

            NavigationStack(path: $router.messages) {
                ThreadsView().withRoutes()
            }
            .tabItem { Label("Messages", systemImage: "bubble.left") }.tag(AppTab.messages)

            NavigationStack(path: $router.settings) {
                SettingsView().withRoutes()
            }
            .tabItem { Label("Settings", systemImage: "gearshape") }.tag(AppTab.settings)
        }
        .environment(router)
        .onChange(of: PushManager.shared.pendingLink, initial: true) { _, link in
            if let link { router.open(link); PushManager.shared.pendingLink = nil }
        }
    }
}

extension Route {
    @ViewBuilder var destination: some View {
        switch self {
        case .item(let id): DataItemView(itemId: id)
        case .user(let id, let messages): UserPageView(userId: id, startOnMessages: messages)
        case .search(let q, let role): SearchView(initialQuery: q, initialRole: role)
        case .editProfile: EditProfileView()
        case .activeSharing: ActiveSharingView()
        }
    }
}

extension View {
    func withRoutes() -> some View {
        toolbar(.hidden, for: .navigationBar)
            .navigationDestination(for: Route.self) { $0.destination.toolbar(.hidden, for: .navigationBar) }
    }
}
