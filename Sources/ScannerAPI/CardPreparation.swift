import Foundation

public struct CardPreparation: Codable, Sendable {
    public var crop: ImageCrop
    public var quarterTurns: Int
    public var orientationVerified: Bool
    public var warnings: [String]
    public var method = "black-background-card-v2"
    public init(crop: ImageCrop, quarterTurns: Int, orientationVerified: Bool, warnings: [String]) {
        self.crop = crop; self.quarterTurns = quarterTurns; self.orientationVerified = orientationVerified; self.warnings = warnings
    }
}
