import Darwin
import Foundation
import ScannerAPI

/// Non-secret, per-user storage policy. The SQLite ledger stays on the local disk.
public struct StorageConfiguration: Codable, Sendable {
    public var storageRoot: String
    public var requiredSMBShare: String?
    public static var defaultURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Scanner/storage.json")
    }
    public init(storageRoot: String, requiredSMBShare: String? = nil) {
        self.storageRoot = storageRoot; self.requiredSMBShare = requiredSMBShare
    }
    public static func load(from url: URL = defaultURL) throws -> Self {
        if FileManager.default.fileExists(atPath: url.path) {
            return try JSONDecoder().decode(Self.self, from: Data(contentsOf: url))
        }
        return Self(storageRoot: FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Scans").path)
    }
    public func validate() throws {
        guard storageRoot.hasPrefix("/"), !storageRoot.utf8.contains(0), !URL(fileURLWithPath: storageRoot).pathComponents.contains("..") else {
            throw ScannerError("invalidSettings", "Storage root must be an absolute path without parent traversal.")
        }
        guard let requiredSMBShare else { return }
        guard let expected = URLComponents(string: requiredSMBShare), expected.scheme == "smb", let host = expected.host,
              !host.isEmpty, expected.user == nil, expected.password == nil, expected.query == nil, expected.fragment == nil,
              expected.path.split(separator: "/").count == 1 else {
            throw ScannerError("invalidSettings", "Expected an SMB share URL without credentials, such as smb://lake/scans.")
        }
        var info = statfs()
        guard statfs(storageRoot, &info) == 0 else { throw ScannerError("storageUnavailable", "Mount \(requiredSMBShare) at \(storageRoot) before scanning.") }
        let type = withUnsafeBytes(of: &info.f_fstypename) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        let mountedAt = withUnsafeBytes(of: &info.f_mntonname) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        let mountedFrom = withUnsafeBytes(of: &info.f_mntfromname) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        let actual = URLComponents(string: "smb:" + mountedFrom)
        guard type == "smbfs", mountedAt == URL(fileURLWithPath: storageRoot).standardizedFileURL.path,
              actual?.host?.lowercased() == host.lowercased(), actual?.path == expected.path else {
            throw ScannerError("storageUnavailable", "Expected \(requiredSMBShare) mounted at \(storageRoot); refusing to write to another volume or local fallback directory.")
        }
    }
}
