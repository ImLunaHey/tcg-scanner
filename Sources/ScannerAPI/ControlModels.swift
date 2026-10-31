import Foundation

/// Defaults for future jobs, never immediate transport-safety or driver writes.
public struct ScanSettings: Codable, Sendable, Equatable {
    public var dpi: Int
    public var duplex: Bool
    public var format: OriginalFormat
    public init(dpi: Int = 300, duplex: Bool = false, format: OriginalFormat = .tiff) {
        self.dpi = dpi; self.duplex = duplex; self.format = format
    }
    public func validate() throws { try ScanRequest(dpi: dpi, duplex: duplex, format: format).validate() }
}
public struct ProfilePatch: Codable, Sendable {
    public var name: String?
    public var dpi: Int?
    public var duplex: Bool?
    public var format: OriginalFormat?
    public init(name: String? = nil, dpi: Int? = nil, duplex: Bool? = nil, format: OriginalFormat? = nil) {
        self.name = name; self.dpi = dpi; self.duplex = duplex; self.format = format
    }
    public var isEmpty: Bool { name == nil && dpi == nil && duplex == nil && format == nil }
}
public struct DeviceStatus: Codable, Sendable {
    public var schemaVersion = 1
    public var device: Device
    public var service: ServiceStatus
    public var inspection: Inspection?
    public var configuredSettings: ScanSettings?
    public var ready: Bool?
    public var warnings: [String]
    public init(device: Device, service: ServiceStatus, inspection: Inspection?, configuredSettings: ScanSettings?, ready: Bool?, warnings: [String]) {
        self.device = device; self.service = service; self.inspection = inspection
        self.configuredSettings = configuredSettings; self.ready = ready; self.warnings = warnings
    }
}
public struct ImageMetadata: Codable, Sendable {
    public var schemaVersion = 1
    public var jobID: UUID
    public var image: OriginalImage
    public init(jobID: UUID, image: OriginalImage) { self.jobID = jobID; self.image = image }
}
public struct ImageChunk: Codable, Sendable {
    public var schemaVersion = 1
    public var metadata: ImageMetadata
    public var offset: Int
    public var nextOffset: Int
    public var eof: Bool
    public var base64: String
    public var chunkSHA256: String
    public init(metadata: ImageMetadata, offset: Int, data: Data, sha256: String) {
        self.metadata = metadata; self.offset = offset; nextOffset = offset + data.count
        eof = nextOffset == metadata.image.bytes; base64 = data.base64EncodedString(); chunkSHA256 = sha256
    }
}
