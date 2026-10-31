import Darwin
import Foundation
import ScannerAPI

/// Test-only subprocess client. All pipe I/O and buffering are isolated to one dispatch queue.
final class StdioPeer: @unchecked Sendable {
    private let process = Process()
    private let input = Pipe()
    private let output = Pipe()
    private let errors = Pipe()
    private let io = DispatchQueue(label: "scanner.mcp.test-client")
    private var pending = Data()
    init(socket: String, readOnly: Bool = false) throws {
        let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        #if DEBUG
        let configuration = "debug"
        #else
        let configuration = "release"
        #endif
        process.executableURL = repo.appendingPathComponent(".build/\(configuration)/scanner-mcp")
        process.arguments = ["--socket", socket] + (readOnly ? ["--read-only"] : [])
        process.standardInput = input; process.standardOutput = output; process.standardError = errors
        try process.run()
        try input.fileHandleForReading.close()
        try output.fileHandleForWriting.close()
        try errors.fileHandleForWriting.close()
    }
    func request(_ method: String, _ params: JSONValue = .object([:])) async throws -> JSONValue {
        try await withCheckedThrowingContinuation { continuation in
            io.async {
                do {
                    let id = UUID().uuidString
                    let request: JSONValue = .object(["jsonrpc": .string("2.0"), "id": .string(id), "method": .string(method), "params": params])
                    try self.input.fileHandleForWriting.write(contentsOf: Wire.encoder().encode(request) + Data([10]))
                    let response = try self.readLine()
                    guard case .object(let object) = response, object["id"] == .string(id), object["error"] == nil, let result = object["result"] else {
                        throw ScannerError("testProtocol", "Unexpected MCP response: \(response)")
                    }
                    continuation.resume(returning: result)
                } catch { continuation.resume(throwing: error) }
            }
        }
    }
    func initialize() async throws -> JSONValue {
        let result = try await request("initialize", .object(["protocolVersion": .string("2025-11-25"), "capabilities": .object([:]),
                                                "clientInfo": .object(["name": .string("ScannerTests"), "version": .string("1")])]))
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            io.async {
                do {
                    let notification: JSONValue = .object(["jsonrpc": .string("2.0"), "method": .string("notifications/initialized")])
                    try self.input.fileHandleForWriting.write(contentsOf: Wire.encoder().encode(notification) + Data([10]))
                    continuation.resume()
                } catch { continuation.resume(throwing: error) }
            }
        }
        return result
    }
    func tool(_ name: String, _ arguments: JSONValue = .object([:])) async throws -> JSONValue {
        try await request("tools/call", .object(["name": .string(name), "arguments": arguments]))
    }
    private func readLine() throws -> JSONValue {
        let deadline = ContinuousClock.now + .seconds(10)
        while true {
            if let newline = pending.firstIndex(of: 10) {
                let data = pending.prefix(upTo: newline); pending.removeSubrange(...newline)
                return try Wire.decoder().decode(JSONValue.self, from: data)
            }
            guard pending.count <= 4 * 1024 * 1024, ContinuousClock.now < deadline else { throw ScannerError("testTimeout", "MCP response timeout or limit exceeded.") }
            var fd = pollfd(fd: output.fileHandleForReading.fileDescriptor, events: Int16(POLLIN), revents: 0)
            let ready = poll(&fd, 1, 100)
            if ready < 0 && errno == EINTR { continue }
            if ready == 0 { continue }
            guard ready > 0 else { throw ScannerError("testProtocol", "Cannot poll MCP stdout.") }
            var buffer = [UInt8](repeating: 0, count: 4096)
            let count = Darwin.read(output.fileHandleForReading.fileDescriptor, &buffer, buffer.count)
            if count < 0 && errno == EINTR { continue }
            guard count > 0 else { throw ScannerError("testProtocol", "MCP stdout closed unexpectedly.") }
            pending.append(contentsOf: buffer.prefix(count))
        }
    }
    func close() async throws -> Int32 {
        try await withCheckedThrowingContinuation { continuation in
            io.async {
                do {
                    try self.input.fileHandleForWriting.close()
                    let deadline = ContinuousClock.now + .seconds(3)
                    while self.process.isRunning && ContinuousClock.now < deadline { Thread.sleep(forTimeInterval: 0.02) }
                    guard !self.process.isRunning else { self.process.terminate(); throw ScannerError("testTimeout", "MCP did not exit after stdin EOF.") }
                    continuation.resume(returning: self.process.terminationStatus)
                } catch { continuation.resume(throwing: error) }
            }
        }
    }
    deinit { if process.isRunning { process.terminate() } }
}
