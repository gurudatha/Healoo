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
            SessionGate().environment(env).tint(Sage.accent)
        }
        // Live channel only while in the foreground; push covers the rest (doc 4.4 / 8).
        .onChange(of: scenePhase) { _, phase in
            guard env.auth.me != nil else { return }
            if phase == .active { env.repo.startRealtime() } else if phase == .background { env.repo.stopRealtime() }
        }
    }
}

/// The global screens; each is the operation of the same id in page-operations.json.
enum AppTab: String, Hashable, CaseIterable {
    case home, search, upload, messages, settings
    var opId: String { rawValue }
}

/// Upload screen opened from a person's page (see UploadView).
struct UploadRequest: Hashable {
    var to: String?
    var mode: PrimaryKind? = nil
    var doctorId: String? = nil
    var hospitalId: String? = nil
}

enum Route: Hashable {
    case item(String)
    case discussion(String)             // an item's messages
    case user(String, messages: Bool)
    case search(query: String, role: Role?)
    case editProfile
    case activeSharing
    case administration
}

/// Chooses between splash, sign-in and the app, based on the Auth0 / demo session.
struct SessionGate: View {
    @Environment(AppEnvironment.self) private var env

    var body: some View {
        Group {
            switch env.auth.session {
            case .checking:
                ZStack { Sage.primary.ignoresSafeArea(); ProgressView().tint(Sage.onPrimary) }
            case .signedOut:
                LoginView()
            case .signedIn(let me):
                RootView().id(me.id)          // new account -> fresh navigation and screens
            }
        }
        // Scroll bars: show them on every vertical list, and flash them when a screen opens so
        // it is obvious when there is more below.
        .scrollIndicators(.visible, axes: .vertical)
        .scrollIndicatorsFlash(onAppear: true)
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
    /// What the Upload tab opens with (from a person's page); nil = a plain new item.
    var uploadRequest: UploadRequest? = nil
    /// Person pages showing; while one is, it draws its own operation bar instead of the global one.
    var personPages = 0

    /// Home always means the landing page: drop whatever was opened on top of it (e.g. Search).
    func openTab(_ t: AppTab) {
        if t == .home { home = NavigationPath() }
        tab = t
    }

    /// Push onto the navigation stack of the tab that is showing.
    func push(_ route: Route) {
        switch tab {
        case .home: home.append(route)
        case .search: search.append(route)
        case .messages: messages.append(route)
        case .settings: settings.append(route)
        case .upload: tab = .home; home.append(route)
        }
    }

    /// Upload or Book from a person's page: that person always receives the item.
    func upload(_ request: UploadRequest) { uploadRequest = request; tab = .upload }

    /// Opened from a notification.
    func open(_ link: DeepLink) {
        tab = .home
        switch link {
        case .item(let id): home.append(Route.item(id))
        case .discussion(let id): home.append(Route.discussion(id))
        case .conversation(let uid): home.append(Route.user(uid, messages: true))
        }
    }
}

struct RootView: View {
    @State private var router = Router()
    private let globalOps = PageOperations.resolve(.global, facts: [])

    var body: some View {
        // The system tab bar is hidden: the bottom bar comes from the "global" page in
        // page-operations.json (at most four, the rest behind the three-dot menu).
        TabView(selection: $router.tab) {
            NavigationStack(path: $router.home) {
                LandingView().withRoutes()
            }
            .toolbar(.hidden, for: .tabBar).tag(AppTab.home)

            NavigationStack(path: $router.search) {
                SearchView(initialQuery: "", initialRole: nil).withRoutes()
            }
            .toolbar(.hidden, for: .tabBar).tag(AppTab.search)

            NavigationStack {
                UploadView(request: router.uploadRequest, addToItemId: nil) { id in
                    router.uploadRequest = nil; router.tab = .home; router.home.append(Route.item(id))
                }
                .id(router.uploadRequest)
                .navigationDestination(for: Route.self) { $0.destination.toolbar(.hidden, for: .navigationBar).toolbar(.hidden, for: .tabBar) }
            }
            .toolbar(.hidden, for: .tabBar).tag(AppTab.upload)

            NavigationStack(path: $router.messages) {
                ConversationsView().withRoutes()
            }
            .toolbar(.hidden, for: .tabBar).tag(AppTab.messages)

            NavigationStack(path: $router.settings) {
                SettingsView().withRoutes()
            }
            .toolbar(.hidden, for: .tabBar).tag(AppTab.settings)
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if router.personPages == 0 {
                OperationBar(ops: globalOps, selected: router.tab.opId) { id in
                    if let t = AppTab.allCases.first(where: { $0.opId == id }) { router.openTab(t) }
                }
            }
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
        case .discussion(let id): DiscussionView(itemId: id)
        case .user(let id, let messages): UserPageView(userId: id, startOnMessages: messages)
        case .search(let q, let role): SearchView(initialQuery: q, initialRole: role)
        case .editProfile: EditProfileView()
        case .activeSharing: ActiveSharingView()
        case .administration: AdminView()
        }
    }
}

extension View {
    func withRoutes() -> some View {
        toolbar(.hidden, for: .navigationBar)
            .navigationDestination(for: Route.self) { $0.destination.toolbar(.hidden, for: .navigationBar).toolbar(.hidden, for: .tabBar) }
    }
}
