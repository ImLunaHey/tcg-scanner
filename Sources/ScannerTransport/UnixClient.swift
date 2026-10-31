import Darwin
import Foundation
import ScannerAPI

public struct UnixClient: Sendable {
    public let path: String
    public init(path: String = SocketLocation.defaultPath) { self.path = path }
    /// For concurrent Swift clients: blocking socket I/O stays off the cooperative executor.
    /// Socket deadlines still apply if the awaiting task is cancelled.
    public func callAsync(_ request: RPCRequest) async throws -> RPCResponse {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                do { continuation.resume(returning: try call(request)) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }
    /// Blocking transport for the headless CLI; never called on a GUI actor.
    public func call(_ request: RPCRequest) throws -> RPCResponse {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw UnixIO.failure("socket") }
        defer { Darwin.close(fd) }
        try UnixIO.prepare(fd)
        guard fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else { throw UnixIO.failure("nonblocking socket") }
        var address = try UnixIO.address(path)
        let connected = UnixIO.withAddress(&address, { connect(fd, $0, $1) })
        if connected != 0 && errno == EINPROGRESS {
            try UnixIO.awaitIO(fd, events: Int16(POLLOUT), deadline: .now + .seconds(5))
            var status: Int32 = 0
            var length = socklen_t(MemoryLayout<Int32>.size)
            guard getsockopt(fd, SOL_SOCKET, SO_ERROR, &status, &length) == 0, status == 0 else {
                throw ScannerError("serviceUnavailable", "Local service connection failed.")
            }
        } else if connected != 0 {
            throw ScannerError("serviceUnavailable", "Cannot connect to ScannerService at \(path). Start scanner-service first. \(String(cString: strerror(errno)))")
        }
        try UnixIO.sameUser(fd)
        try UnixIO.writeLine(Wire.encoder().encode(request), to: fd)
        let data = try UnixIO.readLine(fd, limit: 4 * 1024 * 1024, seconds: 90)
        let response = try Wire.decoder().decode(RPCResponse.self, from: data)
        guard response.jsonrpc == "2.0", response.id == request.id,
              (response.result == nil) != (response.error == nil) else {
            throw ScannerError("transport", "Invalid or mismatched service response.")
        }
        return response
    }
}
