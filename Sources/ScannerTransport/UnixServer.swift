import Darwin
import Foundation
import ScannerAPI

/// Accepts locally on the main queue; blocking I/O runs on bounded worker queues.
@MainActor
public final class UnixServer {
    private var source: DispatchSourceRead?
    private var lockFD: Int32 = -1
    private let path: String
    private let handler: @Sendable (Data) async -> RPCResponse
    private let log: @Sendable (String) -> Void
    private var active = 0
    private var retainShutdownLock = false
    public var activeConnections: Int { active }

    public init(path: String, log: @escaping @Sendable (String) -> Void = { _ in },
                handler: @escaping @Sendable (Data) async -> RPCResponse) {
        self.path = path; self.handler = handler; self.log = log
    }
    public func start() throws {
        guard source == nil else { throw ScannerError("busy", "Server is already listening.") }
        var address = try UnixIO.address(path)
        let directory = URL(fileURLWithPath: path).deletingLastPathComponent().path
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true,
                                                attributes: [.posixPermissions: 0o700])
        var info = stat()
        guard lstat(directory, &info) == 0, info.st_mode & S_IFMT == S_IFDIR,
              info.st_uid == getuid(), info.st_mode & 0o077 == 0 else {
            throw ScannerError("accessDenied", "Socket directory must be owned by you, not a symlink, and mode 0700: \(directory)")
        }
        lockFD = open(path + ".lock", O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard lockFD >= 0 else { throw UnixIO.failure("open service lock") }
        var success = false
        defer { if !success { Darwin.close(lockFD); lockFD = -1 } }
        guard fstat(lockFD, &info) == 0, info.st_mode & S_IFMT == S_IFREG,
              info.st_uid == getuid(), info.st_mode & 0o077 == 0 else {
            throw ScannerError("accessDenied", "Service lock must be a private regular file owned by you.")
        }
        guard flock(lockFD, LOCK_EX | LOCK_NB) == 0 else { throw ScannerError("busy", "ScannerService is already running at this socket.") }
        if lstat(path, &info) == 0 {
            guard info.st_mode & S_IFMT == S_IFSOCK, info.st_uid == getuid() else {
                throw ScannerError("accessDenied", "Refusing to replace a non-socket or another user's socket.")
            }
            guard unlink(path) == 0 else { throw UnixIO.failure("remove stale socket") }
        } else if errno != ENOENT { throw UnixIO.failure("inspect socket path") }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw UnixIO.failure("socket") }
        defer { if !success { Darwin.close(fd) } }
        try UnixIO.prepare(fd)
        guard fcntl(fd, F_SETFL, O_NONBLOCK) == 0,
              UnixIO.withAddress(&address, { bind(fd, $0, $1) }) == 0 else { throw UnixIO.failure("bind socket") }
        guard chmod(path, 0o600) == 0, listen(fd, 16) == 0 else {
            unlink(path)
            throw UnixIO.failure("listen")
        }
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: .main)
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.acceptConnections(fd) }
        }
        source.setCancelHandler { Darwin.close(fd) }
        self.source = source
        source.resume()
        success = true
    }
    public func stopAccepting(releaseLockWhenDrained: Bool = true) {
        retainShutdownLock = !releaseLockWhenDrained
        guard let source else { return }
        self.source = nil
        unlink(path)
        source.cancel()
        releaseLockIfDrained()
    }
    public func releaseShutdownLock() {
        retainShutdownLock = false
        releaseLockIfDrained()
    }
    private func releaseLockIfDrained() {
        if !retainShutdownLock && source == nil && active == 0 && lockFD >= 0 {
            Darwin.close(lockFD)
            lockFD = -1
        }
    }
    private func connectionFinished() {
        active -= 1
        releaseLockIfDrained()
    }
    private func acceptConnections(_ listener: Int32) {
        guard source != nil else { return }
        for _ in 0..<32 {
            let fd = accept(listener, nil, nil)
            if fd < 0 { if errno == EINTR { continue }; return }
            guard active < 16 else { Darwin.close(fd); continue }
            do {
                try UnixIO.prepare(fd)
                try UnixIO.sameUser(fd)
                // Keep deadlines effective even if a peer stops consuming data.
                guard fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else { throw UnixIO.failure("nonblocking socket") }
            } catch { Darwin.close(fd); log("Rejected local connection: \(error)"); continue }
            active += 1
            let handler = self.handler
            let log = self.log
            DispatchQueue.global(qos: .utility).async {
                do {
                    let bytes = try UnixIO.readLine(fd, limit: CameraIdentification.maximumRequestBytes, seconds: 5)
                    Task {
                        let response = await handler(bytes)
                        DispatchQueue.global(qos: .utility).async {
                            do { try UnixIO.writeLine(Wire.encoder().encode(response), to: fd) }
                            catch { log("Response delivery failed: \(error)") }
                            Darwin.close(fd)
                            Task { @MainActor [weak self] in self?.connectionFinished() }
                        }
                    }
                } catch {
                    log("Request rejected: \(error)")
                    Darwin.close(fd)
                    Task { @MainActor [weak self] in self?.connectionFinished() }
                }
            }
        }
    }
}
