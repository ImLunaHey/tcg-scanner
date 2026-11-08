import CoreGraphics
import CryptoKit
import Foundation
import ImageIO
import MCP
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerMCPSupport

private actor MCPMock: ScannerBackend {
    var captures = 0
    var hold = false
    func setHold(_ value: Bool) { hold = value }
    func devices() -> [Device] { [Device(id: "mcp-mock", name: "MCP mock scanner")] }
    func inspect(deviceID: String) -> Inspection {
        var report = Inspection(device: Device(id: deviceID, name: "MCP mock scanner"))
        report.sessionOpened = true; report.sessionClosed = true
        var unit = UnitSnapshot(); unit.type = 3
        unit.settings["resolutionDPI"] = Setting(current: .number(300), allowed: .array([.number(300), .number(600)]), readOnly: false)
        unit.settings["supportsDuplexScanning"] = Setting(current: .bool(true), readOnly: true)
        unit.settings["documentLoaded"] = Setting(current: .bool(true), readOnly: true)
        report.units = [unit]; return report
    }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        captures += 1
        try await store.markScanning(settings: [:])
        for sequence in 1...(request.duplex ? 2 : 1) {
            let context = try #require(CGContext(data: nil, width: 8, height: 8, bitsPerComponent: 8, bytesPerRow: 32,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
            let file = store.incomingDirectory.appendingPathComponent("page-\(sequence).png")
            let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
            CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
            #expect(CGImageDestinationFinalize(destination))
            try await store.preserve(file, sequence: sequence)
        }
        do { while hold { try await Task.sleep(for: .milliseconds(5)) } }
        catch is CancellationError { return CaptureOutcome(cancelled: true) }
        return CaptureOutcome()
    }
}
private func payload(_ value: JSONValue, failure: Bool = false) throws -> JSONValue {
    guard case .object(let result) = value, case .object(let structured) = result["structuredContent"] else { throw ScannerError("test", "Missing structured result.") }
    #expect(result["isError"] == .bool(failure)); #expect(structured["ok"] == .bool(!failure))
    if case .array(let content) = result["content"], case .object(let text) = content.first, case .string(let json) = text["text"] {
        #expect(try Wire.decoder().decode(JSONValue.self, from: Data(json.utf8)) == result["structuredContent"])
    } else { Issue.record("Missing text JSON compatibility result") }
    return try #require(structured[failure ? "error" : "data"])
}
private func poll(_ peer: StdioPeer, id: String, terminal: Bool) async throws -> ScanRecord {
    let deadline = ContinuousClock.now + .seconds(5)
    while ContinuousClock.now < deadline {
        let record = try payload(await peer.tool("scanner_get_job", .object(["id": .string(id)]))).decode(ScanRecord.self)
        if terminal ? record.state.terminal : !record.images.isEmpty { return record }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("testTimeout", "MCP job did not progress.")
}

@Test @MainActor func completeScanningWorkflowThroughCompiledStdioMCP() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("m4-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = MCPMock()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    let path = root.appendingPathComponent("s.sock").path
    let service = UnixServer(path: path, handler: DiagnosticRouter(coordinator: coordinator, backendName: "mock").respond)
    try service.start(); defer { service.stopAccepting() }
    let peer = try StdioPeer(socket: path)
    _ = try await peer.initialize()
    let tools = try await peer.request("tools/list")
    guard case .object(let object) = tools, case .array(let list) = object["tools"] else { throw ScannerError("test", "Missing tools") }
    #expect(list.count == 29)
    _ = try payload(await peer.tool("scanner_list_devices"))
    _ = try payload(await peer.tool("scanner_get_capabilities"))
    _ = try payload(await peer.tool("scanner_get_settings"))
    _ = try payload(await peer.tool("scanner_get_diagnostics"))
    #expect(try payload(await peer.tool("scanner_get_device")).decode(DeviceStatus.self).ready == true)
    #expect(await backend.captures == 0)
    let settings: JSONValue = .object(["dpi": .number(600), "duplex": .bool(true), "format": .string("png")])
    _ = try payload(await peer.tool("scanner_set_settings", settings))
    let profileID = UUID().uuidString
    _ = try payload(await peer.tool("scanner_create_profile", .object(["id": .string(profileID), "name": .string("pokemon"), "dpi": .number(600), "duplex": .bool(true), "format": .string("png")])))
    _ = try payload(await peer.tool("scanner_update_profile", .object(["id": .string("pokemon"), "dpi": .number(300)])))
    #expect(try payload(await peer.tool("scanner_get_profile", .object(["id": .string("pokemon")]))).decode(ScanProfile.self).duplex)
    _ = try payload(await peer.tool("scanner_list_profiles"))
    let requestID = UUID().uuidString
    let arguments: JSONValue = .object(["requestID": .string(requestID), "profileID": .string(profileID)])
    let started = try payload(await peer.tool("scanner_start_scan", arguments)).decode(ScanRecord.self)
    #expect(started.request.dpi == 600 && started.request.format == .png)
    let completed = try await poll(peer, id: requestID, terminal: true)
    #expect(completed.state == .completed && completed.images.count == 2)
    #expect(try payload(await peer.tool("scanner_start_scan", arguments)).decode(ScanRecord.self).id == completed.id)
    #expect(await backend.captures == 1)
    for name in ["scanner_get_batch", "scanner_get_batch_manifest", "scanner_get_scan_metadata"] {
        #expect(try payload(await peer.tool(name, .object(["id": .string(requestID)]))).decode(ScanRecord.self).images.count == 2)
    }
    _ = try payload(await peer.tool("scanner_list_batches")); _ = try payload(await peer.tool("scanner_list_jobs"))
    _ = try payload(await peer.tool("scanner_get_events")); _ = try payload(await peer.tool("scanner_get_service_status"))
    for image in completed.images {
        _ = try payload(await peer.tool("scanner_get_image_metadata", .object(["jobID": .string(requestID), "sequence": .number(Double(image.sequence))])))
        var bytes = Data(); var offset = 0
        repeat {
            let chunk = try payload(await peer.tool("scanner_read_image", .object(["jobID": .string(requestID), "sequence": .number(Double(image.sequence)), "offset": .number(Double(offset)), "length": .number(32)]))).decode(ImageChunk.self)
            bytes.append(try #require(Data(base64Encoded: chunk.base64))); offset = chunk.nextOffset
            if chunk.eof { break }
        } while offset < image.bytes
        #expect(SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined() == image.sha256)
    }
    let continuation: JSONValue = .object(["id": .string(requestID), "requestID": .string(UUID().uuidString)])
    for _ in 0..<2 { _ = try payload(await peer.tool("scanner_continue_batch", continuation)) }
    #expect(try await poll(peer, id: requestID, terminal: true).images.count == 4)
    #expect(await backend.captures == 2)
    await backend.setHold(true)
    let secondID = UUID().uuidString
    _ = try payload(await peer.tool("scanner_start_scan", .object(["requestID": .string(secondID), "useDeviceSettings": .bool(true)])))
    _ = try await poll(peer, id: secondID, terminal: false)
    let thirdID = UUID().uuidString
    #expect(try payload(await peer.tool("scanner_start_scan", .object(["requestID": .string(thirdID), "format": .string("png")]))).decode(ScanRecord.self).state == .queued)
    _ = try payload(await peer.tool("scanner_cancel_job", .object(["id": .string(thirdID)])))
    _ = try payload(await peer.tool("scanner_cancel_job", .object(["id": .string(secondID)])))
    #expect(try await poll(peer, id: secondID, terminal: true).state == .cancelled)
    #expect(try await poll(peer, id: thirdID, terminal: true).state == .cancelled)
    #expect(await backend.captures == 3)
    #expect(try await peer.close() == 0)
    #expect(try await coordinator.scanStatus(completed.id).state == .completed)
    await coordinator.shutdownScans()
}

@Test func mcpPolicyAnnotationsAndErrorsAreEnforced() async throws {
    let server = ScannerMCPServer(socketPath: "/tmp/nonexistent-scanner-\(UUID().uuidString)", readOnly: true)
    let tools = await server.tools()
    #expect(tools.allSatisfy { $0.annotations.readOnlyHint == true })
    #expect(!tools.contains { $0.name == "scanner_start_scan" || $0.name == "scanner_continue_batch" })
    #expect(!tools.contains { $0.name == "scanner_identify_card" || $0.name == "scanner_review_card" })
    #expect(tools.contains { $0.name == "scanner_get_card_identification" })
    let denied = try await server.call(.init(name: "scanner_start_scan", arguments: ["requestID": .string(UUID().uuidString)]))
    #expect(denied.isError == true)
    let unavailable = try await server.call(.init(name: "scanner_list_devices"))
    #expect(try payload(.wrapping(unavailable), failure: true).decode(ScannerError.self).kind == "serviceUnavailable")
    for (name, arguments) in [("scanner_list_devices", ["shell": MCP.Value.string("no")]),
                              ("scanner_read_image", ["jobID": .string(UUID().uuidString), "sequence": .int(1), "path": .string("/etc/passwd")]),
                              ("scanner_get_events", ["waitSeconds": .int(21)])] {
        let result = try await server.call(.init(name: name, arguments: arguments))
        #expect(try payload(.wrapping(result), failure: true).decode(ScannerError.self).kind == "invalidParams")
    }
    let start = try #require(ToolCatalog.all.first { $0.name == "scanner_start_scan" })
    #expect(start.tool.annotations.readOnlyHint == false && start.tool.annotations.destructiveHint == true)
    #expect(throws: ScannerError.self) { try start.parameters([:]) }
    let condition = try #require(ToolCatalog.all.first { $0.name == "scanner_check_condition" })
    #expect(condition.tool.annotations.destructiveHint == false && condition.tool.annotations.readOnlyHint == false)
    #expect(throws: ScannerError.self) { try condition.parameters(["jobID": .string(UUID().uuidString), "sequence": .int(1)]) }
    let identify = try #require(ToolCatalog.all.first { $0.name == "scanner_identify_card" })
    #expect(identify.tool.annotations.openWorldHint == true && identify.tool.annotations.destructiveHint == false)
    #expect(throws: ScannerError.self) { try identify.parameters(["jobID": .string(UUID().uuidString), "sequence": .int(1)]) }
}
