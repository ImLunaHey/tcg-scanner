import Testing
import ScannerAPI
@testable import ScannerCLI

@Test func parsesDiagnosticCommandsAndOptions() throws {
    let devices = try Scanner.parseAsRoot(["devices", "--json", "--verbose", "--socket", "/tmp/private/sock"]) as! Devices
    #expect(devices.options.json)
    #expect(devices.options.verbose)
    #expect(devices.options.socket == "/tmp/private/sock")
    let caps = try Scanner.parseAsRoot(["capabilities", "--device", "abc"]) as! Capabilities
    #expect(caps.options.device == "abc")
    #expect(try Scanner.parseAsRoot(["settings"]) is Settings)
    #expect(try Scanner.parseAsRoot(["diagnostics"]) is DiagnosticsCommand)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["scan"]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["settings", "--duplex"]) }
}
@Test func stableExitCodesAndPartialInspectionErrors() {
    #expect(ClientOptions.exitCode(for: "serviceUnavailable") == 3)
    #expect(ClientOptions.exitCode(for: "deviceNotFound") == 4)
    #expect(ClientOptions.exitCode(for: "busy") == 5)
    #expect(ClientOptions.hasInspectionErrors(.object(["inspection": .object(["errors": .array([.string("failed")])])])))
    #expect(!ClientOptions.hasInspectionErrors(.object(["errors": .array([])])))
}
