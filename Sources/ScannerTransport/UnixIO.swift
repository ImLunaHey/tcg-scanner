import Darwin
import Foundation
import ScannerAPI

public enum SocketLocation {
    public static var defaultPath: String {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Scanner/run/control.sock").path
    }
}

enum UnixIO {
    static func failure(_ operation: String) -> ScannerError {
        ScannerError("transport", "\(operation): \(String(cString: strerror(errno)))", domain: NSPOSIXErrorDomain, code: Int(errno))
    }
    static func address(_ path: String) throws -> sockaddr_un {
        var address = sockaddr_un()
        let bytes = Array(path.utf8) + [0]
        guard path.hasPrefix("/"), !path.utf8.contains(0), bytes.count <= MemoryLayout.size(ofValue: address.sun_path) else {
            throw ScannerError("transport", "Unix socket path must be absolute and at most 103 UTF-8 bytes.")
        }
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: bytes + Array(repeating: 0, count: $0.count - bytes.count)) }
        return address
    }
    static func withAddress<T>(_ address: inout sockaddr_un, _ body: (UnsafePointer<sockaddr>, socklen_t) -> T) -> T {
        withUnsafePointer(to: &address) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { body($0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
    }
    static func prepare(_ fd: Int32) throws {
        var one: Int32 = 1
        guard setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, socklen_t(MemoryLayout.size(ofValue: one))) == 0,
              fcntl(fd, F_SETFD, FD_CLOEXEC) == 0 else { throw failure("configure socket") }
    }
    static func sameUser(_ fd: Int32) throws {
        var uid: uid_t = 0; var gid: gid_t = 0
        guard getpeereid(fd, &uid, &gid) == 0, uid == getuid() else {
            throw ScannerError("accessDenied", "Unix socket peer must be the same macOS user.")
        }
    }
    static func awaitIO(_ fd: Int32, events: Int16, deadline: ContinuousClock.Instant) throws {
        while true {
            let remaining = ContinuousClock.now.duration(to: deadline)
            guard remaining > .zero else { throw ScannerError("timeout", "Local API I/O timed out.") }
            let ms = min(1000, max(1, Int(remaining.components.seconds * 1000 + remaining.components.attoseconds / 1_000_000_000_000_000)))
            var p = pollfd(fd: fd, events: events, revents: 0)
            let result = poll(&p, 1, Int32(ms))
            if result > 0 { return }
            if result < 0 && errno != EINTR { throw failure("poll") }
        }
    }
    static func readLine(_ fd: Int32, limit: Int, seconds: Int) throws -> Data {
        var data = Data()
        let deadline = ContinuousClock.now + .seconds(seconds)
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            try awaitIO(fd, events: Int16(POLLIN), deadline: deadline)
            let count = recv(fd, &buffer, buffer.count, 0)
            if count < 0 && (errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK) { continue }
            guard count > 0 else { throw ScannerError("transport", "Peer closed the connection before a complete message arrived.") }
            data.append(contentsOf: buffer.prefix(count))
            guard data.count <= limit else { throw ScannerError("resourceLimit", "Local API message exceeds size limit.") }
            if let newline = data.firstIndex(of: 10) {
                guard data.suffix(from: data.index(after: newline)).allSatisfy({ $0 == 10 || $0 == 13 || $0 == 32 }) else {
                    throw ScannerError("transport", "Only one request per connection is supported.")
                }
                return Data(data[..<newline])
            }
        }
    }
    static func writeLine(_ data: Data, to fd: Int32) throws {
        guard data.count <= 4 * 1024 * 1024 else { throw ScannerError("resourceLimit", "Response exceeds size limit.") }
        let bytes = data + Data([10])
        let deadline = ContinuousClock.now + .seconds(10)
        try bytes.withUnsafeBytes { buffer in
            var sent = 0
            while sent < buffer.count {
                try awaitIO(fd, events: Int16(POLLOUT), deadline: deadline)
                let count = send(fd, buffer.baseAddress!.advanced(by: sent), buffer.count - sent, 0)
                if count < 0 && (errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK) { continue }
                guard count > 0 else { throw failure("send") }
                sent += count
            }
        }
    }
}
