import Darwin
import Foundation
import Testing
import ScannerAPI
@testable import ScannerTransport

@Test @MainActor func unixRoundTripAndExclusiveEndpoint() async throws {
    let directory = "/tmp/scanner-test-\(UUID().uuidString)"
    let path = directory + "/s.sock"
    let server = UnixServer(path: path) { bytes in
        let request = try! Wire.decoder().decode(RPCRequest.self, from: bytes)
        return RPCResponse(id: request.id, result: .object(["ok": .bool(true)]))
    }
    try server.start()
    defer { server.stopAccepting(); try? FileManager.default.removeItem(atPath: directory) }
    let second = UnixServer(path: path) { _ in RPCResponse(id: .null) }
    #expect(throws: ScannerError.self) { try second.start() }
    let attrs = try FileManager.default.attributesOfItem(atPath: path)
    #expect((attrs[.posixPermissions] as? NSNumber)?.intValue == 0o600)
    let response = try await UnixClient(path: path).callAsync(RPCRequest(method: "v1.devices"))
    #expect(response.result == .object(["ok": .bool(true)]))
}

@Test @MainActor func refusesUnsafeSocketDirectoryAndRegularFiles() throws {
    let directory = "/tmp/scanner-test-\(UUID().uuidString)"
    try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o755])
    defer { try? FileManager.default.removeItem(atPath: directory) }
    let path = directory + "/s.sock"
    let server = UnixServer(path: path) { _ in RPCResponse(id: .null) }
    #expect(throws: ScannerError.self) { try server.start() }
    try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory)
    try Data("do not overwrite".utf8).write(to: URL(fileURLWithPath: path))
    #expect(throws: ScannerError.self) { try server.start() }
    #expect(try String(contentsOfFile: path, encoding: .utf8) == "do not overwrite")
}

@Test func framingLimitsAndTimeout() throws {
    var fds: [Int32] = [0, 0]
    #expect(socketpair(AF_UNIX, SOCK_STREAM, 0, &fds) == 0)
    defer { Darwin.close(fds[0]); Darwin.close(fds[1]) }
    try UnixIO.writeLine(Data("abcdef".utf8), to: fds[0])
    #expect(throws: ScannerError.self) { try UnixIO.readLine(fds[1], limit: 3, seconds: 1) }
    #expect(throws: ScannerError.self) { try UnixIO.readLine(fds[1], limit: 100, seconds: 0) }
    #expect(throws: ScannerError.self) { try UnixIO.address("/" + String(repeating: "x", count: 104)) }
}

@Test func hardwareOwnershipIsSeparateFromEndpointOwnership() throws {
    let directory = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let path = directory.appendingPathComponent("hardware.lock").path
    do {
        let owner = try HardwareOwnerLock(path: path)
        _ = withExtendedLifetime(owner) {
            #expect(throws: ScannerError.self) { try HardwareOwnerLock(path: path) }
        }
    }
    let newOwner = try HardwareOwnerLock(path: path)
    withExtendedLifetime(newOwner) {}
}

@Test(arguments: [4095, 4096, 350_000, 1_000_000])
func largeSocketFramesPreserveBytesAcrossReadBoundaries(count: Int) async throws {
    var fds: [Int32] = [0, 0]
    #expect(socketpair(AF_UNIX, SOCK_STREAM, 0, &fds) == 0)
    let writerFD = fds[0], readerFD = fds[1]
    defer { Darwin.close(writerFD); Darwin.close(readerFD) }
    let expected = Data(repeating: 97, count: count)
    let writer = Task.detached { try UnixIO.writeLine(expected, to: writerFD) }
    let actual = try UnixIO.readLine(readerFD, limit: count + 1, seconds: 5)
    try await writer.value
    #expect(actual == expected)
}

@Test func socketFrameRejectsTrailingNonWhitespaceInSameRead() throws {
    var fds: [Int32] = [0, 0]
    #expect(socketpair(AF_UNIX, SOCK_STREAM, 0, &fds) == 0)
    defer { Darwin.close(fds[0]); Darwin.close(fds[1]) }
    try UnixIO.writeLine(Data("first\nsecond".utf8), to: fds[0])
    #expect(throws: ScannerError.self) { try UnixIO.readLine(fds[1], limit: 100, seconds: 1) }
}
