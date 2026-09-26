import Foundation

struct DevAccount: Decodable, Identifiable, Hashable {
    let publicId: String
    let displayName: String
    let role: String

    var id: String { publicId }
    var initials: String {
        displayName.replacingOccurrences(of: "Dr. ", with: "")
            .split(separator: " ").prefix(2).compactMap { $0.first.map { String($0).uppercased() } }.joined()
    }
    /// "DOCTOR" -> "Doctor", "APP_ADMINISTRATOR" -> "App administrator"
    var roleLabel: String {
        let s = role.lowercased().replacingOccurrences(of: "_", with: " ")
        return s.prefix(1).uppercased() + s.dropFirst()
    }
}

struct DevSignInError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// Developer sign-in for the local-network trial (backend AUTH_MODE=dev, NETWORK_MODE=lan).
/// GET /dev/users lists the server's seeded accounts; POST /dev/token returns a 12-hour token.
/// The Healoo ID is remembered so an expired token is renewed silently.
///
/// Debug builds only (`#if DEBUG`), with HealooDevSignIn = true and HealooUseFakeData = false.
/// The backend never offers these endpoints in internet mode.
final class DevSignIn {
    let isEnabled: Bool
    private let base: URL
    private let defaults = UserDefaults(suiteName: "healoo.devauth") ?? .standard
    private let session = URLSession(configuration: .ephemeral)
    private let decoder: JSONDecoder = { let d = JSONDecoder(); d.keyDecodingStrategy = .convertFromSnakeCase; return d }()

    private static let disabled = "This server doesn't offer developer sign-in. Start it with NETWORK_MODE=lan and AUTH_MODE=dev."
    private enum Key { static let id = "public_id", token = "token", expires = "expires_at" }

    init(info: [String: Any], baseURL: URL, useFakeData: Bool) {
        #if DEBUG
        isEnabled = ((info["HealooDevSignIn"] as? Bool) ?? false) && !useFakeData
        #else
        isEnabled = false
        #endif
        base = baseURL
    }

    /// Seeded accounts offered by the server.
    func accounts() async throws -> [DevAccount] {
        struct Wrap: Decodable { let data: [DevAccount] }
        let data = try await call(URLRequest(url: base.appendingPathComponent("dev/users")), notFound: Self.disabled)
        return try decoder.decode(Wrap.self, from: data).data
    }

    /// Gets a token for `publicId` and remembers it. Returns the access token.
    func signIn(publicId: String) async throws -> String {
        let id = publicId.trimmingCharacters(in: .whitespaces).uppercased()
        guard id.range(of: #"^HL-[A-Z0-9]{5}$"#, options: .regularExpression) != nil else {
            throw DevSignInError(message: "A Healoo ID looks like HL-2M9P4.")
        }
        var req = URLRequest(url: base.appendingPathComponent("dev/token"))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONSerialization.data(withJSONObject: ["public_id": id])
        struct TokenResponse: Decodable { let accessToken: String; let expiresIn: Double }
        let res = try decoder.decode(TokenResponse.self, from: try await call(req, notFound: nil))
        defaults.set(id, forKey: Key.id)
        defaults.set(res.accessToken, forKey: Key.token)
        defaults.set(Date().addingTimeInterval(res.expiresIn).timeIntervalSince1970, forKey: Key.expires)
        return res.accessToken
    }

    /// A stored token still valid for at least a minute, if any.
    var storedToken: String? {
        guard isEnabled, let t = defaults.string(forKey: Key.token) else { return nil }
        let expires = defaults.double(forKey: Key.expires)
        return expires - Date().timeIntervalSince1970 > 60 ? t : nil
    }

    var hasStoredAccount: Bool { isEnabled && defaults.string(forKey: Key.id) != nil }

    /// New token for the remembered account, or nil (then the user signs in again).
    func renew() async -> String? {
        guard let id = defaults.string(forKey: Key.id) else { return nil }
        return try? await signIn(publicId: id)
    }

    func clear() {
        [Key.id, Key.token, Key.expires].forEach { defaults.removeObject(forKey: $0) }
    }

    private func call(_ req: URLRequest, notFound: String?) async throws -> Data {
        var req = req
        req.timeoutInterval = 10
        let data: Data, resp: URLResponse
        do {
            (data, resp) = try await session.data(for: req)
        } catch {
            let code = (error as? URLError)?.code
            let cause: String
            switch code {
            case .serverCertificateUntrusted?, .serverCertificateHasUnknownRoot?, .serverCertificateHasBadDate?, .secureConnectionFailed?:
                cause = "The connection worked but the certificate isn't trusted: install and trust healoo-local-ca.crt on this device."
            case .cannotConnectToHost?, .timedOut?, .networkConnectionLost?, .notConnectedToInternet?:
                cause = "No connection: check the server is running, the address, the firewall (port 8443) and the network."
            case .cannotFindHost?:
                cause = "Unknown host name: check HealooAPIBaseURL."
            default:
                cause = "Network error."
            }
            throw DevSignInError(message: "Can't reach the trial server at \(base.absoluteString). \(cause) (\(error.localizedDescription))")
        }
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        if (200..<300).contains(code) { return data }
        if code == 404, let notFound { throw DevSignInError(message: notFound) }
        struct ErrorBody: Decodable { struct E: Decodable { let message: String }; let error: E }
        let message = (try? decoder.decode(ErrorBody.self, from: data))?.error.message
        throw DevSignInError(message: message ?? "The server answered \(code).")
    }
}
