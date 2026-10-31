import Foundation
import Testing
import ScannerAPI
import ScannerClient
import ScannerCore
import ScannerServiceSupport
@testable import ScannerMacUI

private actor DesktopMock: ScannerBackend {
    func devices() -> [Device] { [Device(id: "desktop", name: "Desktop mock")] }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome { CaptureOutcome() }
    func inspect(deviceID: String) -> Inspection {
        var result = Inspection(device: Device(id: deviceID, name: "Desktop mock"))
        var unit = UnitSnapshot(); unit.type = 3
        unit.settings["resolutionDPI"] = Setting(current: .number(300), allowed: .array([.number(75), .number(300), .number(600), .number(1200)]), readOnly: false)
        unit.settings["supportsDuplexScanning"] = Setting(current: .bool(true), readOnly: true)
        result.units = [unit]; result.sessionClosed = true
        return result
    }
}
private actor LostReply {
    let router: DiagnosticRouter
    var lost = false
    init(_ router: DiagnosticRouter) { self.router = router }
    func send(_ request: RPCRequest) async throws -> RPCResponse {
        let reply = await router.respond(to: try Wire.encoder().encode(request))
        if request.method == "v1.jobs.create" && !lost { lost = true; throw ScannerError("transport", "Simulated lost reply after commit") }
        return reply
    }
}
@Test @MainActor func desktopRequiresLoadedPaperAndRetriesThePersistedRequest() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let suite = "ScannerTest-" + UUID().uuidString
    let defaults = try #require(UserDefaults(suiteName: suite))
    defer { defaults.removePersistentDomain(forName: suite); try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: DesktopMock(), storageRoot: root)
    let sender = LostReply(DiagnosticRouter(coordinator: coordinator))
    let client = ScannerClient(send: sender.send)
    let model = ScannerModel(client: client, defaults: defaults)
    try await model.refresh()
    #expect(model.connected && model.supportsDuplex)
    #expect(model.dpiValues == [300, 600])
    #expect(!model.canScan)
    model.start()
    #expect(try await coordinator.jobs().jobs.isEmpty)
    model.paperReady = true
    #expect(model.canScan)
    model.start()
    let deadline = ContinuousClock.now + .seconds(5)
    while model.busy && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(5)) }
    let id = try #require(model.pending?.requestID)
    #expect(try await coordinator.jobs().jobs.count == 1)
    let relaunched = ScannerModel(client: client, defaults: defaults)
    #expect(relaunched.pending?.requestID == id)
    try await relaunched.refresh()
    relaunched.paperReady = true
    relaunched.start()
    while relaunched.busy && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(5)) }
    #expect(relaunched.pending == nil)
    #expect(relaunched.selectedJob == id)
    #expect(try await coordinator.jobs().jobs.count == 1)
    await coordinator.shutdownScans()
}
