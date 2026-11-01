import Darwin
import Foundation
import ScannerAPI

public struct StoreConfiguration: Codable, Sendable {
    public var baseURL: String
    public var scannerInstanceID: String
    public var tokenFile: String
    public static var defaultURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Scanner/store/config.json")
    }
    public static func load() throws -> Self? {
        guard FileManager.default.fileExists(atPath: defaultURL.path) else { return nil }
        let config = try Wire.decoder().decode(Self.self, from: privateFile(defaultURL.path, limit: 4096))
        _ = try config.validatedURL()
        guard !config.scannerInstanceID.isEmpty, config.scannerInstanceID.utf8.count <= 128 else {
            throw ScannerError("configuration", "Store scanner instance ID must be stable and nonempty.")
        }
        return config
    }
    func validatedURL() throws -> URL {
        guard let url = URL(string: baseURL), url.scheme == "https", url.host != nil,
              url.user == nil, url.password == nil, url.query == nil, url.fragment == nil,
              url.path.isEmpty || url.path == "/" else {
            throw ScannerError("configuration", "Store must be an HTTPS origin without credentials, query or path.")
        }
        return url
    }
    func token() throws -> String {
        let token = String(decoding: try Self.privateFile(tokenFile, limit: 4096), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        guard (32...1024).contains(token.utf8.count), token.utf8.allSatisfy({ $0 > 32 && $0 < 127 }) else {
            throw ScannerError("configuration", "Store token is empty or invalid.")
        }
        return token
    }
    static func privateFile(_ path: String, limit: Int) throws -> Data {
        guard path.hasPrefix("/"), URL(fileURLWithPath: path).resolvingSymlinksInPath().path == path else {
            throw ScannerError("configuration", "Store configuration paths must be absolute and contain no symlinks.")
        }
        let fd = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
        guard fd >= 0 else { throw ScannerError("configuration", "Store configuration or token file is unavailable.") }
        defer { Darwin.close(fd) }
        var info = stat()
        guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_uid == getuid(),
              info.st_mode & 0o077 == 0, info.st_nlink == 1, info.st_size > 0, info.st_size <= limit else {
            throw ScannerError("configuration", "Store configuration files must be private regular files owned by this user.")
        }
        var bytes = [UInt8](repeating: 0, count: Int(info.st_size))
        guard Darwin.read(fd, &bytes, bytes.count) == bytes.count else { throw ScannerError("configuration", "Cannot read Store configuration.") }
        return Data(bytes)
    }
}
