import Foundation

public enum OriginalFormat: String, Codable, Sendable { case tiff, png }
public enum ScanState: String, Codable, Sendable {
    case queued, preparing, scanning, processing, completed, failed, cancelled
    public var terminal: Bool { self == .completed || self == .failed || self == .cancelled }
}
public struct ScanRequest: Codable, Sendable, Equatable {
    public var requestID: UUID
    public var deviceID: String?
    public var outputDirectory: String?
    public var profileID: UUID?
    public var useDeviceSettings: Bool?
    public var prepareCards: Bool?
    public var dpi: Int
    public var duplex: Bool
    public var format: OriginalFormat
    public init(requestID: UUID = UUID(), deviceID: String? = nil, outputDirectory: String? = nil,
                dpi: Int = 300, duplex: Bool = false, format: OriginalFormat = .png, profileID: UUID? = nil, useDeviceSettings: Bool? = nil, prepareCards: Bool? = nil) {
        self.requestID = requestID; self.deviceID = deviceID; self.outputDirectory = outputDirectory
        self.dpi = dpi; self.duplex = duplex; self.format = format; self.profileID = profileID; self.useDeviceSettings = useDeviceSettings
        self.prepareCards = prepareCards
    }
    public func validate() throws {
        guard !(profileID != nil && useDeviceSettings == true) else { throw ScannerError("invalidSettings", "Choose either a profile or device defaults.") }
        guard (150...600).contains(dpi) else { throw ScannerError("invalidSettings", "DPI must be between 150 and 600 and supported by the scanner.") }
        if let deviceID, deviceID.isEmpty || deviceID.utf8.count > 512 {
            throw ScannerError("invalidSettings", "Invalid scanner ID.")
        }
        if let outputDirectory, outputDirectory.isEmpty || outputDirectory.utf8.count > 2048 || outputDirectory.utf8.contains(0) {
            throw ScannerError("invalidSettings", "Invalid output directory.")
        }
    }
}
public struct OriginalImage: Codable, Sendable {
    public var sequence: Int
    public var relativePath: String
    public var bytes: Int
    public var sha256: String
    public var width: Int
    public var height: Int
    public var imageCount: Int
    public var uniformType: String
    public var side = "unknown"
    public init(sequence: Int, relativePath: String, bytes: Int, sha256: String, width: Int, height: Int,
                imageCount: Int, uniformType: String) {
        self.sequence = sequence; self.relativePath = relativePath; self.bytes = bytes; self.sha256 = sha256
        self.width = width; self.height = height; self.imageCount = imageCount; self.uniformType = uniformType
    }
}
public struct ScanRecord: Codable, Sendable {
    public var schemaVersion = 1
    public var id: UUID
    public var device: Device
    public var request: ScanRequest
    public var state: ScanState = .preparing
    public var createdAt = Date()
    public var startedAt: Date?
    public var completedAt: Date?
    public var profile: ScanProfile?
    public var batchNumber: Int?
    public var batchID: UUID { id }
    public var progress: ScanProgress { ScanProgress(imagesSaved: images.count) }
    public var outputDirectory: String
    public var images: [OriginalImage] = []
    public var processedImages: [ProcessedImage]?
    public var cardIdentifications: [CardIdentification]?
    public var errors: [ScannerError] = []
    public var warnings = ["Images retain driver callback order. Physical front/back identity is not inferred.",
                           "Original means the driver's delivered file, not raw sensor data. No application image processing is applied to originals."]
    public var appliedSettings: [String: JSONValue] = [:]
    public var sessionClosed = false
    public init(device: Device, request: ScanRequest, outputDirectory: String) {
        id = request.requestID; self.device = device; self.request = request; self.outputDirectory = outputDirectory
    }
}
