import Foundation
import Vision

/// Sparse code inserts need only a public URL or the two agreeing logo lines.
/// Never uses a private redemption code as evidence.
enum CodeCardOrientation {
    struct Line { let text: String; let confidence: Float; let turns: Int }
    static func rotation(_ observations: [VNRecognizedTextObservation]) -> (turns: Int, basis: String)? {
        rotation(lines: observations.compactMap { observation in
            guard let candidate = observation.topCandidates(1).first else { return nil }
            let angle = atan2(observation.topRight.y - observation.topLeft.y, observation.topRight.x - observation.topLeft.x)
            return Line(text: candidate.string, confidence: candidate.confidence, turns: (Int((angle / (.pi / 2)).rounded()) + 4) % 4)
        })
    }
    static func rotation(lines: [Line]) -> (turns: Int, basis: String)? {
        let candidates = (0..<4).compactMap { turns -> (Int, String)? in
            let words = lines.filter { $0.turns == turns && $0.confidence >= 0.7 }.map(\.text)
            if CodeCardText.hasRedemptionURL(words) { return (turns, "pokemon-code-url") }
            let labels = words.map { $0.uppercased().filter(\.isLetter) }
            if labels.contains("LIVE") && labels.contains("TRADINGCARDGAME") { return (turns, "pokemon-code-text") }
            return nil
        }
        return candidates.count == 1 ? candidates[0] : nil
    }
}
