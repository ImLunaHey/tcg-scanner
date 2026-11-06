import Foundation
import Vision

/// Basic Energy can have only a title and copyright line. Require both in their
/// upright locations rather than relaxing the general four-line text threshold.
enum EnergyTextOrientation {
    struct Line {
        let text: String
        let confidence: Float
        let center: CGPoint
        let turns: Int
    }
    static func rotation(_ observations: [VNRecognizedTextObservation]) -> Int? {
        rotation(lines: observations.compactMap { observation in
            guard let text = observation.topCandidates(1).first else { return nil }
            let angle = atan2(observation.topRight.y - observation.topLeft.y,
                              observation.topRight.x - observation.topLeft.x)
            let turns = (Int((angle / (.pi / 2)).rounded()) + 4) % 4
            return Line(text: text.string, confidence: text.confidence,
                center: CGPoint(x: observation.boundingBox.midX, y: observation.boundingBox.midY), turns: turns)
        })
    }
    static func rotation(lines: [Line]) -> Int? {
        let candidates = (0..<4).filter { turns in
            let matching = lines.filter { $0.turns == turns && $0.confidence >= 0.5 }
            let title = matching.contains { line in
                let text = line.text.uppercased().filter(\.isLetter)
                let point = upright(line.center, turns: turns)
                return line.confidence >= 0.7 && ["ENERGY", "BASICENERGY"].contains(text) && point.y > 0.8 && point.x < 0.8
            }
            let footer = matching.contains { line in
                let text = line.text.lowercased().folding(options: .diacriticInsensitive, locale: Locale(identifier: "en_US_POSIX"))
                return upright(line.center, turns: turns).y < 0.12 && text.contains("pokemon") &&
                    (text.contains("nintendo") || text.contains("creatures") || text.contains("game freak"))
            }
            return title && footer
        }
        return candidates.count == 1 ? candidates[0] : nil
    }
    private static func upright(_ p: CGPoint, turns: Int) -> CGPoint {
        switch turns {
        case 1: CGPoint(x: p.y, y: 1 - p.x)
        case 2: CGPoint(x: 1 - p.x, y: 1 - p.y)
        case 3: CGPoint(x: 1 - p.y, y: p.x)
        default: p
        }
    }
}
