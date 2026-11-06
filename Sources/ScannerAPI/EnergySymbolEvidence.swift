import Foundation

/// An Energy type read from the printed corner symbol, not an exact catalogue identity.
public struct EnergySymbolEvidence: Codable, Sendable, Equatable {
    public var method = "energy-corner-shape-v2"
    public var type: String
    public var similarity: Double // Ink-shape overlap, not a calibrated probability.
    public var region: ImageCrop // Normalized top-left coordinates in the upright image.
    public init(type: String, similarity: Double, region: ImageCrop) {
        self.type = type; self.similarity = similarity; self.region = region
    }
    public var displayName: String { type + " Energy" }
}
