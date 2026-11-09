import Foundation
import MCP
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerMCPSupport

private actor InventoryBackend: ScannerBackend {
    func devices() throws -> [Device] { throw ScannerError("test", "Inventory must not discover hardware.") }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "Inventory must not inspect hardware.") }
}
private func record(_ name: String, batch: Int, code: Bool = false) -> ScanRecord {
    var r = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(duplex: true, prepareCards: true), outputDirectory: "/unused")
    r.state = .completed; r.batchNumber = batch; r.processedImages = []
    for seq in 1...2 {
        let image = OriginalImage(sequence: seq, relativePath: "\(seq).png", bytes: 1, sha256: "fixture", width: 200, height: 280, imageCount: 1, uniformType: "public.png")
        r.images.append(image)
        var edit = ProcessedImage(request: ImageEditRequest(jobID: r.id, sequence: seq), sourceSHA256: image.sha256, image: image)
        edit.cardPreparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0, orientationVerified: true, warnings: [], orientationBasis: code ? "pokemon-code-text" : seq == 1 ? "pokemon-back-pattern" : "text")
        r.processedImages?.append(edit)
    }
    var identity = CardIdentification(request: IdentificationRequest(jobID: r.id, sequence: 2), sourceSHA256: "fixture")
    identity.state = .awaitingCatalogue
    identity.evidence = RecognitionEvidence(lines: [], candidates: [])
    identity.evidence?.uncataloguedCard = UncataloguedCard(name: name, setCode: name, collectorNumber: "001/100")
    r.cardIdentifications = [identity]
    return r
}
private func page(_ peer: StdioPeer, _ arguments: JSONValue) async throws -> InventoryPage {
    let result = try await peer.tool("scanner_list_inventory", arguments)
    guard case .object(let wrapper) = result, wrapper["isError"] != .bool(true),
          case .object(let structured) = wrapper["structuredContent"], structured["ok"] == .bool(true),
          let data = structured["data"] else { throw ScannerError("test", "Inventory tool failed.") }
    return try data.decode(InventoryPage.self)
}
@Test @MainActor func inventoryThroughReadOnlyStdioPreservesPagesFiltersAndCopyLocations() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let db = try ScannerDatabase()
    let records = [record("Alpha", batch: 1), record("Alpha", batch: 2), record("Beta", batch: 3), record("Code", batch: 4, code: true)]
    for record in records { try db.insert(record, submission: record.request) }
    let service = try ScannerCoordinator(backend: InventoryBackend(), storageRoot: root, database: db)
    let socket = root.appendingPathComponent("mcp.sock").path
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: service).respond)
    try server.start(); defer { server.stopAccepting() }
    let peer = try StdioPeer(socket: socket, readOnly: true)
    _ = try await peer.initialize()
    let first = try await page(peer, .object(["limit": .number(1)]))
    #expect(first.totalCopies == 3 && first.excludedCodeCards == 1 && first.matchingEntries == 2)
    #expect(first.entries.count == 1 && first.nextOffset == 1)
    let second = try await page(peer, .object(["limit": .number(1), "offset": .number(1)]))
    #expect(second.nextOffset == nil && second.entries.count == 1)
    #expect(Set((first.entries + second.entries).map(\.name)) == ["Alpha", "Beta"])
    let found = try await page(peer, .object(["search": .string("Alpha")]))
    #expect(found.entries.count == 1 && found.entries[0].quantity == 2)
    #expect(Set(found.entries[0].copies.map(\.batchID)) == Set(records.prefix(2).map(\.id)))
    #expect(found.entries[0].copies.allSatisfy { $0.cardNumber == 1 })
    let filtered = try await page(peer, .object(["set": .string(found.entries[0].setKey)]))
    #expect(filtered.matchingEntries == 1 && filtered.entries[0].name == "Alpha")
    #expect(try await page(peer, .object(["search": .string("NoSuchCard")])).entries.isEmpty)
    #expect(try await peer.close() == 0)
}
@Test func inventoryToolSchemaIsReadOnlyAndRejectsUnboundedInputs() throws {
    let tool = try #require(ToolCatalog.all.first { $0.name == "scanner_list_inventory" })
    #expect(tool.tool.annotations.readOnlyHint == true && tool.tool.annotations.destructiveHint == false)
    for args: [String: MCP.Value] in [["limit": .int(101)], ["offset": .int(-1)], ["search": .string(String(repeating: "x", count: 161))], ["set": .string(String(repeating: "x", count: 513))], ["path": .string("/tmp")]] {
        #expect(throws: ScannerError.self) { try tool.parameters(args) }
    }
}
