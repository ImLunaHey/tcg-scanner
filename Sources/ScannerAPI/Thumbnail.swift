import Foundation

public enum ThumbnailFormat: String, Codable, Sendable { case png, jpeg }

public struct ImageThumbnail: Codable, Sendable {
    public var schemaVersion = 1
    public var base64: String
    public var sha256: String
    public var mediaType: String?
    public var width: Int
    public var height: Int
    public init(data: Data, sha256: String, width: Int, height: Int, mediaType: String = "image/png") {
        self.mediaType = mediaType
        self.base64 = data.base64EncodedString(); self.sha256 = sha256; self.width = width; self.height = height
    }
}
