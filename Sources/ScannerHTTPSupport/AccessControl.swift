import CryptoKit
import Darwin
import Foundation
import Security
import ScannerAPI

/// Credentials are never accepted in URLs/cookies or emitted in logs.
public enum GatewayCredential {
    public static var defaultPath: String {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Scanner/http/token").path
    }
    public static func generate() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
            throw ScannerError("configuration", "Secure token generation failed.")
        }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
    public static func create(at path: String) throws {
        guard path.hasPrefix("/") else { throw ScannerError("configuration", "Token path must be absolute.") }
        let parent = URL(fileURLWithPath: path).deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let attributes = try FileManager.default.attributesOfItem(atPath: parent.path)
        guard (attributes[.posixPermissions] as? NSNumber)?.intValue == 0o700 else {
            throw ScannerError("configuration", "Token directory must have mode 0700.")
        }
        let token = try generate() + "\n"
        let fd = open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard fd >= 0 else { throw ScannerError("configuration", "Cannot create token file; existing files are never replaced.") }
        defer { Darwin.close(fd) }
        let bytes = Array(token.utf8)
        guard bytes.withUnsafeBytes({ Darwin.write(fd, $0.baseAddress, $0.count) }) == bytes.count, fsync(fd) == 0 else {
            throw ScannerError("configuration", "Could not persist token file.")
        }
    }
    public static func load(at path: String) throws -> String {
        let fd = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
        guard fd >= 0 else { throw ScannerError("configuration", "Token unavailable; run scanner-http --init-token first.") }
        defer { Darwin.close(fd) }
        var info = stat()
        guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_mode & 0o777 == 0o600,
              info.st_uid == getuid(), info.st_nlink == 1, (64...65).contains(info.st_size) else {
            throw ScannerError("configuration", "Token must be an owned, private (0600), regular file containing 64 hexadecimal characters.")
        }
        var bytes = [UInt8](repeating: 0, count: Int(info.st_size))
        guard Darwin.read(fd, &bytes, bytes.count) == bytes.count else { throw ScannerError("configuration", "Cannot read token.") }
        let token = String(decoding: bytes, as: UTF8.self).trimmingCharacters(in: .newlines)
        guard valid(token) else { throw ScannerError("configuration", "Token must contain 64 lowercase hexadecimal characters.") }
        return token
    }
    static func valid(_ value: String) -> Bool { value.utf8.count == 64 && value.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) } }
    static func digest(_ value: String) -> String { SHA256.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined() }
}

public actor GatewayAccess {
    struct Session { var expiresAt: Date }
    private let rootDigest: [UInt8]
    private var sessions: [String: Session] = [:]
    private let lifetime: TimeInterval
    private var window = Date()
    private var requests = 0
    private var attempts = 0
    private var active = 0
    private var streams = 0
    private var downloads = 0
    private var cameras = 0
    private var cameraWindow = Date()
    private var cameraRequests = 0
    public init(token: String, sessionLifetime: TimeInterval = 3600) throws {
        guard GatewayCredential.valid(token) else { throw ScannerError("configuration", "Invalid gateway token.") }
        rootDigest = Array(GatewayCredential.digest(token).utf8); lifetime = sessionLifetime
    }
    func rateLimit(authenticated: Bool) throws {
        if Date().timeIntervalSince(window) >= 60 { window = Date(); requests = 0; attempts = 0 }
        if authenticated {
            guard requests < 600 else { throw GatewayError(429, "rateLimit", "Request limit reached; retry later.") }
            requests += 1
        } else {
            guard attempts < 30 else { throw GatewayError(429, "rateLimit", "Authentication attempt limit reached; retry later.") }
            attempts += 1
        }
    }
    // Public lookups have their own budget and cannot exhaust login/control quotas.
    func rateLimitCamera() throws {
        if Date().timeIntervalSince(cameraWindow) >= 60 { cameraWindow = Date(); cameraRequests = 0 }
        guard cameraRequests < 60 else { throw GatewayError(429, "rateLimit", "Camera lookup limit reached; retry shortly.") }
        cameraRequests += 1
    }
    func authenticate(_ header: String?) throws -> String {
        guard let header, header.hasPrefix("Bearer ") else {
            try rateLimit(authenticated: false); throw GatewayError(401, "unauthorized", "Bearer authentication is required.")
        }
        let token = String(header.dropFirst(7))
        guard GatewayCredential.valid(token) else {
            try rateLimit(authenticated: false); throw GatewayError(401, "unauthorized", "Invalid or expired credential.")
        }
        let digest = GatewayCredential.digest(token)
        let difference = zip(rootDigest, digest.utf8).reduce(UInt8(0)) { $0 | ($1.0 ^ $1.1) }
        sessions = sessions.filter { $0.value.expiresAt > Date() }
        guard difference == 0 || sessions[digest] != nil else {
            try rateLimit(authenticated: false); throw GatewayError(401, "unauthorized", "Invalid or expired credential.")
        }
        try rateLimit(authenticated: true)
        return difference == 0 ? "root" : digest
    }
    func issue(for principal: String) throws -> (String, Date) {
        guard principal == "root" else { throw GatewayError(403, "accessDenied", "Only the root credential can create sessions.") }
        guard sessions.count < 64 else { throw GatewayError(429, "resourceLimit", "Session limit reached.") }
        let token = try GatewayCredential.generate(); let expiry = Date().addingTimeInterval(lifetime)
        sessions[GatewayCredential.digest(token)] = Session(expiresAt: expiry)
        return (token, expiry)
    }
    func revoke(_ principal: String) throws {
        guard principal != "root" else { throw GatewayError(400, "invalidParams", "Root credential rotation requires replacing its file and restarting the gateway.") }
        sessions.removeValue(forKey: principal)
    }
    func validPrincipal(_ principal: String) -> Bool { principal == "root" || (sessions[principal]?.expiresAt ?? .distantPast) > Date() }
    func acquire(_ kind: String) throws {
        guard active < 16, kind != "events" || streams < 4, kind != "image" || downloads < 2, kind != "camera" || cameras < 1 else {
            throw GatewayError(429, "resourceLimit", "Concurrent request limit reached.")
        }
        active += 1
        if kind == "events" { streams += 1 }
        if kind == "image" { downloads += 1 }
        if kind == "camera" { cameras += 1 }
    }
    func release(_ kind: String) {
        active -= 1
        if kind == "events" { streams -= 1 }
        if kind == "image" { downloads -= 1 }
        if kind == "camera" { cameras -= 1 }
    }
}
