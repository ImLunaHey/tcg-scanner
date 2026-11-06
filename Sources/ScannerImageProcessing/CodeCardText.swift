import Foundation

/// Classifies public printed instructions/branding. Never returns redemption
/// strings, reads QR payloads, uploads images or attempts code redemption.
enum CodeCardText {
    static func hasRedemptionURL(_ words: [String]) -> Bool {
        let text = words.joined(separator: " ")
        // Exact public host/path, allowing OCR whitespace. Do not accept lookalike
        // domains or longer paths, and never extract the private redemption code.
        return text.range(of: #"(?i)(?<![a-z0-9.\-/])(?:https?://)?(?:www\.)?pokemon\s*\.\s*com\s*/\s*(?:redeem|tcgo)(?![a-z0-9/_-])"#, options: .regularExpression) != nil
    }
    static func hasRedemptionWording(_ words: [String]) -> Bool {
        let text = words.joined(separator: " ").lowercased()
        return text.contains("redeem") || text.contains("reward") || text.contains("pokemon.com/")
    }
    static func matches(_ words: [String]) -> Bool {
        let text = words.joined(separator: " ").lowercased()
            .folding(options: .diacriticInsensitive, locale: Locale(identifier: "en_US_POSIX"))
        let compact = text.filter { !$0.isWhitespace }
        if hasRedemptionURL(words) { return true }
        // OCR may report the stacked logo and its LIVE/ONLINE line separately.
        let branded = compact.contains("pokemon") &&
            (compact.contains("tcg") || compact.contains("tradingcardgame") ||
             (compact.contains("redeem") && compact.contains("reward"))) &&
            (compact.contains("live") || compact.contains("online"))
        let instructions = ["redeem", "reward", "playtoday", "playonline", "code"].contains { compact.contains($0) }
        return branded && instructions
    }
}
