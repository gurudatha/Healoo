import Foundation

/// Unsent message text, kept while the app runs: leaving a discussion (or a person's Message sheet)
/// and coming back shows what was typed. Memory only, so closing the app clears everything.
/// Keys: "item:<itemId>" for a discussion, "person:<userId>" for a new message to a person.
@MainActor
enum Drafts {
    private static var texts: [String: String] = [:]

    static func get(_ key: String) -> String { texts[key] ?? "" }

    static func set(_ key: String, _ text: String) {
        if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { texts[key] = nil } else { texts[key] = text }
    }

    static func clear() { texts.removeAll() }
}
