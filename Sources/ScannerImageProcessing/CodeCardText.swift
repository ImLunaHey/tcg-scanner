import Foundation

/// Classifies public printed instructions/branding. Never returns redemption
/// strings, reads QR payloads, uploads images or attempts code redemption.
enum CodeCardText {
    static func matches(_ words: [String]) -> Bool {
        let text = words.joined(separator: " ").lowercased()
            .folding(options: .diacriticInsensitive, locale: Locale(identifier: "en_US_POSIX"))
        let compact = text.filter { !$0.isWhitespace }
        if compact.contains("pokemon.com/redeem") || compact.contains("pokemon.com/tcgo") { return true }
        // OCR may report the stacked logo and its LIVE/ONLINE line separately.
        let branded = compact.contains("pokemon") &&
            (compact.contains("tcg") || compact.contains("tradingcardgame") ||
             (compact.contains("redeem") && compact.contains("reward"))) &&
            (compact.contains("live") || compact.contains("online"))
        let instructions = ["redeem", "reward", "playtoday", "playonline", "code"].contains { compact.contains($0) }
        return branded && instructions
    }
}
