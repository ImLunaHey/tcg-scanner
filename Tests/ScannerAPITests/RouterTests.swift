import Foundation
import Testing
import ScannerAPI
import ScannerCore
import ScannerServiceSupport

private actor Mock: ScannerBackend {
    func devices() -> [Device] { [Device(id: "mock", name: "Mock scanner")] }
    func inspect(deviceID: String) -> Inspection {
        var r = Inspection(device: Device(id: deviceID, name: "Mock scanner"))
        r.sessionOpened = true; r.sessionClosed = true
        return r
    }
}
private func router() -> DiagnosticRouter { DiagnosticRouter(coordinator: ScannerCoordinator(backend: Mock()), backendName: "mock") }

@Test func routesAllReadOnlyMethods() async throws {
    for method in ["v1.devices", "v1.capabilities", "v1.settings", "v1.diagnostics"] {
        let request = RPCRequest(method: method)
        let response = await router().respond(to: try Wire.encoder().encode(request))
        #expect(response.id == request.id)
        #expect(response.result != nil)
        #expect(response.error == nil)
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
        let response = await router().respond(to: Data(text.utf8))
        #expect(response.error?.code == code)
        #expect(response.result == nil)
    }
}
@Test func missingDeviceUsesStructuredError() async throws {
    let request = RPCRequest(method: "v1.capabilities", deviceID: "missing")
    let response = await router().respond(to: try Wire.encoder().encode(request))
    #expect(response.error?.data?.kind == "deviceNotFound")
}
