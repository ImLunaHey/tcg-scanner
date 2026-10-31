import Foundation

public struct CardPreparation: Codable, Sendable {
    public var crop: ImageCrop
    public var quarterTurns: Int
    public var orientationVerified: Bool
    public var warnings: [String]
    public var sourceCrop: ImageCrop?
    public var deskewDegrees: Double?
    public var orientationBasis: String?
    public var method = "black-background-card-v3"
    public init(crop: ImageCrop, quarterTurns: Int, orientationVerified: Bool, warnings: [String], deskewDegrees: Double? = nil, orientationBasis: String? = nil, sourceCrop: ImageCrop? = nil) {
        self.crop = crop; self.quarterTurns = quarterTurns; self.orientationVerified = orientationVerified; self.warnings = warnings
        self.sourceCrop = sourceCrop
        self.deskewDegrees = deskewDegrees; self.orientationBasis = orientationBasis
    }
}
