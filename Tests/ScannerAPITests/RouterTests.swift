import Foundation
import Testing
import ScannerAPI
import ScannerCore
import ScannerServiceSupport
import ScannerTransport

private actor Mock: ScannerBackend {
    func devices() -> [Device] { [Device(id: "mock", name: "Mock scanner")] }
    func inspect(deviceID: String) -> Inspection {
        var r = Inspection(device: Device(id: deviceID, name: "Mock scanner"))
        r.sessionOpened = true; r.sessionClosed = true
        var unit = UnitSnapshot()
        unit.type = 3; unit.className = "MockADF"
        unit.settings["resolutionDPI"] = Setting(current: .number(300), allowed: .array([.number(300), .number(600)]), readOnly: false)
        r.availableFunctionalUnitTypes = [3]; r.units = [unit]
        return r
    }
}
private func router() throws -> DiagnosticRouter { DiagnosticRouter(coordinator: try ScannerCoordinator(backend: Mock()), backendName: "mock") }

@Test func routesAllReadOnlyMethods() async throws {
    for method in ["v1.devices", "v1.capabilities", "v1.settings", "v1.diagnostics"] {
        let request = RPCRequest(method: method)
        let response = try await router().respond(to: try Wire.encoder().encode(request))
        #expect(response.id == request.id)
        #expect(response.result != nil)
        #expect(response.error == nil)
        if method == "v1.capabilities" {
            let report = try #require(response.result).decode(Inspection.self)
            #expect(report.units[0].settings["resolutionDPI"]?.allowed == .array([.number(300), .number(600)]))
        } else if method == "v1.settings" {
            let report = try #require(response.result).decode(SettingsReport.self)
            #expect(report.units[0].settings["resolutionDPI"]?.current == .number(300))
            #expect(report.units[0].settings["resolutionDPI"]?.allowed == nil)
        }
    }
}
@Test func rejectsInvalidRequestsAndWrites() async throws {
    for (text, code) in [
        ("{", -32700),
        ("{}", -32600),
        (#"{"jsonrpc":"1.0","id":1,"method":"v1.devices"}"#, -32600),
        (#"{"jsonrpc":"2.0","id":1,"method":"v1.scan"}"#, -32601),
        (#"{"jsonrpc":"2.0","id":1,"method":"v1.settings","params":{"output":"/tmp/a"}}"#, -32602),
        (#"{"jsonrpc":"2.0","id":1,"method":"v1.settings","params":{"deviceID":7}}"#, -32602)
    ] {
        let response = try await router().respond(to: Data(text.utf8))
        #expect(response.error?.code == code)
        #expect(response.result == nil)
    }
}
@Test func missingDeviceUsesStructuredError() async throws {
    let request = RPCRequest(method: "v1.capabilities", deviceID: "missing")
    let response = try await router().respond(to: try Wire.encoder().encode(request))
    #expect(response.error?.data?.kind == "deviceNotFound")
}

@Test @MainActor func mockBackendThroughRealServiceTransport() async throws {
    let directory = "/tmp/scanner-test-\(UUID().uuidString)"
    let path = directory + "/s.sock"
    let router = try router()
    let server = UnixServer(path: path, handler: router.respond)
    try server.start()
    defer { server.stopAccepting(); try? FileManager.default.removeItem(atPath: directory) }
    for method in ["v1.devices", "v1.capabilities", "v1.settings", "v1.diagnostics"] {
        let response = try await Task.detached { try UnixClient(path: path).call(RPCRequest(method: method)) }.value
        #expect(response.error == nil)
        #expect(response.result != nil)
    }
}

@Test func rejectsMalformedScanRequestsWithoutCreatingFiles() async throws {
    for params in [JSONValue.object([:]), .object(["requestID": .string("bad")]),
                   .object(["shell": .string("ignored")])] {
        var request = RPCRequest(method: "v1.scan.start")
        request.params = params
        let response = try await router().respond(to: try Wire.encoder().encode(request))
        #expect(response.error?.code == -32602)
    }
    for method in ["v1.scan.status", "v1.scan.cancel"] {
        var request = RPCRequest(method: method)
        request.params = .object(["scanID": .string("not-a-uuid")])
        let response = try await router().respond(to: try Wire.encoder().encode(request))
        #expect(response.error?.code == -32602)
    }
}
