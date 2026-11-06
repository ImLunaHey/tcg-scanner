import Foundation

/// Measured ink shape in the upright card footer, independent of catalogue rarity.
public struct RaritySymbolEvidence: Codable, Sendable, Equatable {
    public var method = "black-footer-shape-v1"
    public var symbol: String
    public var similarity: Double // Shape overlap, not identity confidence.
    public var region: ImageCrop
    public var collectorNumber: String
    public var setTotal: String
    public init(symbol: String, similarity: Double, region: ImageCrop, collectorNumber: String, setTotal: String) {
        self.symbol = symbol; self.similarity = similarity; self.region = region
        self.collectorNumber = collectorNumber; self.setTotal = setTotal
    }
}
