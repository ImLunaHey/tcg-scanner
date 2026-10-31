import Foundation

/// Top-left normalized crop after optional deskew, before clockwise quarter turns.
public struct ImageCrop: Codable, Sendable, Equatable {
    public var x: Double; public var y: Double; public var width: Double; public var height: Double
    public init(x: Double = 0, y: Double = 0, width: Double = 1, height: Double = 1) {
        self.x = x; self.y = y; self.width = width; self.height = height
    }
}
public struct ImageEditRequest: Codable, Sendable, Equatable {
    public var id: UUID
    public var jobID: UUID
    public var sequence: Int
    public var crop: ImageCrop
    public var quarterTurns: Int
    /// Clockwise degrees applied to an expanded canvas before cropping. Nil preserves legacy edits.
    public var deskewDegrees: Double?
    public init(id: UUID = UUID(), jobID: UUID, sequence: Int, crop: ImageCrop = ImageCrop(), quarterTurns: Int = 0, deskewDegrees: Double? = nil) {
        self.id = id; self.jobID = jobID; self.sequence = sequence; self.crop = crop; self.quarterTurns = quarterTurns; self.deskewDegrees = deskewDegrees
    }
    public func validate() throws {
        guard (deskewDegrees ?? 0).isFinite, abs(deskewDegrees ?? 0) <= 12,
              (1...100).contains(sequence), (0...3).contains(quarterTurns),
              [crop.x, crop.y, crop.width, crop.height].allSatisfy(\.isFinite),
              crop.x >= 0, crop.y >= 0, crop.width > 0, crop.height > 0,
              crop.x + crop.width <= 1.000000001, crop.y + crop.height <= 1.000000001 else {
            throw ScannerError("invalidParams", "Crop must be inside the deskewed canvas, deskew within ±12 degrees, and rotation 0–3 clockwise quarter turns.")
        }
    }
}
public struct ProcessedImage: Codable, Sendable {
    public var id: UUID { request.id }
    public var request: ImageEditRequest
    public var createdAt: Date
    public var sourceSHA256: String
    public var image: OriginalImage
    public var cardPreparation: CardPreparation?
    public init(request: ImageEditRequest, sourceSHA256: String, image: OriginalImage) {
        self.request = request; self.createdAt = Date(timeIntervalSince1970: Date().timeIntervalSince1970.rounded(.down)); self.sourceSHA256 = sourceSHA256; self.image = image
    }
}
