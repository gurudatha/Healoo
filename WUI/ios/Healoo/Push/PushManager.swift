import UIKit
import UserNotifications
import FirebaseCore
import FirebaseMessaging

/// Where to go when the app is opened from a notification.
enum DeepLink: Equatable {
    case item(String)
    case conversation(String)
}

/// Push (design doc 8): APNs delivers to iOS; FCM maps its registration token to the APNs token,
/// so the backend sends every platform through FCM. The FCM token is registered with POST /v1/devices.
///
/// Payload keys: type ("message" | "report" | "alert" | "booking"), title, body, item_id?, user_id?
/// Firebase runs only when GoogleService-Info.plist is in the app bundle; otherwise push is skipped.
@Observable
final class PushManager: NSObject, UNUserNotificationCenterDelegate, MessagingDelegate {
    static let shared = PushManager()

    var pendingLink: DeepLink?
    private(set) var firebaseReady = false
    @ObservationIgnored private weak var repo: HealooRepository?
    @ObservationIgnored private var fcmToken: String?

    func configure() {
        UNUserNotificationCenter.current().delegate = self
        guard Bundle.main.path(forResource: "GoogleService-Info", ofType: "plist") != nil else { return }
        FirebaseApp.configure()
        Messaging.messaging().delegate = self
        firebaseReady = true
    }

    /// After sign-in: ask for permission, register with APNs; the FCM token then reaches the backend.
    @MainActor func registerIfPossible(repo: HealooRepository) {
        self.repo = repo
        guard firebaseReady else { return }
        Task {
            let granted = (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound])) ?? false
            if granted { UIApplication.shared.registerForRemoteNotifications() }
            if let token = try? await Messaging.messaging().token() { await send(token) }
        }
    }

    @MainActor func unregister(repo: HealooRepository) async {
        guard firebaseReady else { return }
        if let token = fcmToken { try? await repo.unregisterDevice(token: token) }
        try? await Messaging.messaging().deleteToken()
        fcmToken = nil
        self.repo = nil
    }

    func didRegister(apnsToken: Data) { if firebaseReady { Messaging.messaging().apnsToken = apnsToken } }

    private func send(_ token: String) async {
        guard token != fcmToken, let repo else { return }
        if (try? await repo.registerDevice(token: token)) != nil { fcmToken = token }
    }

    // MARK: MessagingDelegate
    func messaging(_ messaging: Messaging, didReceiveRegistrationToken token: String?) {
        guard let token else { return }
        Task { await send(token) }
    }

    // MARK: UNUserNotificationCenterDelegate
    /// Show banners while the app is open too (the live channel updates the screen underneath).
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        [.banner, .sound, .list]
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        let link: DeepLink? = if let id = info["item_id"] as? String { .item(id) }
                              else if let uid = info["user_id"] as? String { .conversation(uid) }
                              else { nil }
        await MainActor.run { pendingLink = link }
    }
}

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        PushManager.shared.configure()
        return true
    }

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        PushManager.shared.didRegister(apnsToken: deviceToken)
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        // Simulator without push or missing entitlement: the app keeps working without push.
    }
}
