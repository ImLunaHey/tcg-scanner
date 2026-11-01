import Foundation
import Testing
import ScannerAPI
import ScannerCore
import ScannerServiceSupport

private actor BackgroundBackend: ScannerBackend {
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        try await store.markScanning(settings: [:])
        return CaptureOutcome()
    }
    func devices() -> [Device] { [Device(id: "background", name: "Mock")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Mock")) }
}

@Test func backgroundRequestIsOptionalAndRejectsUnknownValues() throws {
    let original = ScanRequest()
    let data = try Wire.encoder().encode(original)
    #expect(try Wire.decoder().decode(ScanRequest.self, from: data).background == nil)
    var json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    json["background"] = "white"
    #expect(try Wire.decoder().decode(ScanRequest.self, from: JSONSerialization.data(withJSONObject: json)).background == .white)
    json["background"] = "unknown"
    #expect(throws: (any Error).self) { try Wire.decoder().decode(ScanRequest.self, from: JSONSerialization.data(withJSONObject: json)) }
}

@Test func whiteBackgroundSurvivesRPCAndParticipatesInIdempotency() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: BackgroundBackend(), storageRoot: root)
    let router = DiagnosticRouter(coordinator: coordinator)
    var scan = ScanRequest(background: .white)
    var request = RPCRequest(method: "v1.jobs.create")
    request.params = try .wrapping(scan)
    let response = try await router.respond(to: Wire.encoder().encode(request))
    #expect(response.error == nil)
    let record = try #require(response.result).decode(ScanRecord.self)
    #expect(record.request.background == .white)
    #expect(try await coordinator.startScan(scan).id == record.id)
    scan.background = .black
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(scan) }
    await coordinator.shutdownScans()
}

@Test func overscanAllowsCardPreparationButRejectsWhiteBacking() throws {
    try ScanRequest(overscan: true).validate()
    #expect(throws: ScannerError.self) { try ScanRequest(background: .white, overscan: true).validate() }
    try ScanRequest(prepareCards: true, overscan: true).validate()
    try ScanRequest(prepareCards: true, background: .black, overscan: true).validate()
    let data = try Wire.encoder().encode(ScanRequest(overscan: true))
    #expect(try Wire.decoder().decode(ScanRequest.self, from: data).overscan == true)
    #expect(try Wire.decoder().decode(ScanRequest.self, from: Wire.encoder().encode(ScanRequest())).overscan == nil)
}

@Test func newCardJobsDefaultToOverscanWithoutRewritingSubmissionIdentity() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: BackgroundBackend(), storageRoot: root)
    let cardRequest = ScanRequest(prepareCards: true)
    let card = try await coordinator.startScan(cardRequest)
    #expect(card.request.overscan == true)
    #expect(try await coordinator.startScan(cardRequest).id == card.id)
    let white = try await coordinator.startScan(ScanRequest(prepareCards: true, background: .white))
    #expect(white.request.overscan == nil)
    let disabled = try await coordinator.startScan(ScanRequest(prepareCards: true, overscan: false))
    #expect(disabled.request.overscan == false)
    let document = try await coordinator.startScan(ScanRequest())
    #expect(document.request.overscan == nil)
    await coordinator.shutdownScans()
}
