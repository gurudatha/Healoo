import Foundation
import Auth0

enum SessionState: Equatable {
    case checking
    case signedOut
    case signedIn(UserProfile)
}

/// Auth0 Universal Login (design doc 5).
/// - login(): hosted login page in an ASWebAuthenticationSession; PKCE handled by the SDK.
/// - Credentials, including the refresh token (scope offline_access), live in the Keychain via CredentialsManager.
/// - freshToken(): a valid access token, renewed when it is about to expire.
/// With an empty HealooAuth0Domain the app runs in demo mode and this class isn't used for sign-in.
///
/// Debug builds can also use `DevSignIn` (trial server with AUTH_MODE=dev). The manager remembers
/// which method signed in, so restore, token renewal and logout go to the right place.
@Observable
final class AuthManager {
    var session: SessionState = .checking

    let dev: DevSignIn
    private enum Method { case auth0, dev }
    @ObservationIgnored private var method: Method?

    let isConfigured: Bool
    private let clientId: String
    private let domain: String
    private let audience: String
    private let credentials: CredentialsManager?

    init(info: [String: Any], baseURL: URL, useFakeData: Bool) {
        dev = DevSignIn(info: info, baseURL: baseURL, useFakeData: useFakeData)
        clientId = (info["HealooAuth0ClientId"] as? String) ?? ""
        domain = (info["HealooAuth0Domain"] as? String) ?? ""
        audience = (info["HealooAuth0Audience"] as? String) ?? ""
        isConfigured = !clientId.isEmpty && !domain.isEmpty
        credentials = isConfigured ? CredentialsManager(authentication: Auth0.authentication(clientId: clientId, domain: domain)) : nil
    }

    var me: UserProfile? { if case .signedIn(let u) = session { u } else { nil } }

    @MainActor func signedOut() { TokenStore.accessToken = nil; session = .signedOut }

    /// True if a stored session was restored (tokens renewed if needed).
    func restore() async -> Bool {
        if dev.hasStoredAccount {
            let stored = dev.storedToken
            let token: String?
            if let stored { token = stored } else { token = await dev.renew() }
            if let token { useDev(token); return true }
        }
        guard let credentials, credentials.canRenew() || credentials.hasValid() else { return false }
        return (try? await use(credentials.credentials())) != nil
    }

    @MainActor func login() async throws {
        let result = try await Auth0.webAuth(clientId: clientId, domain: domain)
            .audience(audience)
            .scope("openid profile email offline_access")
            .start()
        _ = credentials?.store(credentials: result)
        dev.clear()
        _ = use(result)
    }

    /// Developer sign-in with a seeded Healoo ID (debug builds, trial server only).
    func devLogin(publicId: String) async throws {
        guard dev.isEnabled else { throw DevSignInError(message: "Developer sign-in is off in this build.") }
        useDev(try await dev.signIn(publicId: publicId))
    }

    @MainActor func logout() async {
        if method == .auth0, isConfigured {
            try? await Auth0.webAuth(clientId: clientId, domain: domain).clearSession()
        }
        _ = credentials?.clear()
        dev.clear()
        method = nil
        signedOut()
    }

    /// Called by the HTTP client on 401: renew once and return the new token, or nil.
    func freshToken() async -> String? {
        if method == .dev {
            guard let token = await dev.renew() else { return nil }
            return useDev(token)
        }
        guard let credentials else { return nil }
        return try? await use(credentials.credentials(minTTL: 60))
    }

    @discardableResult
    private func use(_ c: Credentials) -> String {
        method = .auth0
        TokenStore.accessToken = c.accessToken
        return c.accessToken
    }

    @discardableResult
    private func useDev(_ token: String) -> String {
        method = .dev
        TokenStore.accessToken = token
        return token
    }
}
