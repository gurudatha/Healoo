import Foundation
import Combine

/// Live channel to the Tokio/Tungstenite gateway (design doc 4.4): wss://<host>/v1/ws
/// Authenticated with the Auth0 access token in the upgrade request.
/// Server frames: {"type":"message.new","message":{…}}, {"type":"item.updated","item_id":"…"}, {"type":"ping"} → {"type":"pong"}.
/// Reconnects with exponential backoff (1 s … 30 s) until stop().
final class RealtimeClient {
    private let url: URL
    private let decoder: JSONDecoder
    private let subject = PassthroughSubject<RealtimeEvent, Never>()
    private var task: URLSessionWebSocketTask?
    private var running = false
    private var attempt = 0
    private var tokenExpired = false
    /// Renews the access token; called before reconnecting after the server closes with 4001.
    private let refresh: (() async -> String?)?
    private lazy var session = URLSession(configuration: .default)

    var events: AnyPublisher<RealtimeEvent, Never> { subject.receive(on: DispatchQueue.main).eraseToAnyPublisher() }

    init(baseURL: URL, decoder: JSONDecoder, refresh: (() async -> String?)? = nil) {
        self.refresh = refresh
        var comps = URLComponents(url: baseURL.appendingPathComponent("v1/ws"), resolvingAgainstBaseURL: false)!
        comps.scheme = comps.scheme == "http" ? "ws" : "wss"
        url = comps.url!
        self.decoder = decoder
    }

    func start() {
        guard !running else { return }
        running = true
        connect()
    }

    func stop() {
        running = false
        task?.cancel(with: .normalClosure, reason: nil)
        task = nil
    }

    private func connect() {
        guard running else { return }
        guard let token = TokenStore.accessToken else { scheduleReconnect(); return }
        var req = URLRequest(url: url)
        req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        let t = session.webSocketTask(with: req)
        task = t
        t.resume()
        subject.send(.connection(true))
        receive(on: t)
    }

    private func receive(on t: URLSessionWebSocketTask) {
        t.receive { [weak self] result in
            guard let self, t === self.task else { return }
            switch result {
            case .success(let msg):
                self.attempt = 0
                if case .string(let text) = msg { self.handle(text, on: t) }
                self.receive(on: t)
            case .failure:
                if t.closeCode.rawValue == 4001 { self.tokenExpired = true }
                self.subject.send(.connection(false))
                self.scheduleReconnect()
            }
        }
    }

    private struct Frame: Decodable { let type: String; let message: Message?; let itemId: String? }

    private func handle(_ text: String, on t: URLSessionWebSocketTask) {
        guard let frame = try? decoder.decode(Frame.self, from: Data(text.utf8)) else { return }
        switch frame.type {
        case "message.new": if let m = frame.message { subject.send(.newMessage(m)) }
        case "item.updated": if let id = frame.itemId { subject.send(.itemChanged(id)) }
        case "ping": t.send(.string(#"{"type":"pong"}"#)) { _ in }
        default: break
        }
    }

    private func scheduleReconnect() {
        guard running else { return }
        let wait = min(30.0, pow(2.0, Double(min(attempt, 5))))
        attempt += 1
        DispatchQueue.global().asyncAfter(deadline: .now() + wait) { [weak self] in
            guard let self else { return }
            guard self.tokenExpired, let refresh = self.refresh else { self.connect(); return }
            self.tokenExpired = false
            Task { _ = await refresh(); self.connect() }
        }
    }
}
