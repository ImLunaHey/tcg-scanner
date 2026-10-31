import CoreGraphics
import Foundation
import Hummingbird
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerHTTPSupport

private actor BrowserMock: ScannerBackend {
    func devices() -> [Device] { [Device(id: "browser-mock", name: "Browser test scanner", transport: "Mock")] }
    func inspect(deviceID: String) -> Inspection {
        var report = Inspection(device: devices()[0]); report.sessionOpened = true; report.sessionClosed = true
        var unit = UnitSnapshot(); unit.type = 3; unit.className = "Mock document feeder"
        unit.settings["resolutionDPI"] = Setting(current: .number(300), allowed: .array([.number(300), .number(600)]), readOnly: false, evidence: .softwareImplemented)
        unit.settings["supportsDuplexScanning"] = Setting(current: .bool(true), readOnly: true, evidence: .softwareImplemented)
        unit.settings["documentLoaded"] = Setting(current: .bool(true), readOnly: true, evidence: .softwareImplemented)
        report.units = [unit]; return report
    }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        do {
            try await store.markScanning(settings: [:])
            for sequence in 1...(request.duplex ? 2 : 1) {
                try await Task.sleep(for: .milliseconds(600))
                let context = try #require(CGContext(data: nil, width: 600, height: 840, bitsPerComponent: 8, bytesPerRow: 2400,
                    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
                context.setFillColor(red: sequence == 1 ? 0.82 : 0.25, green: 0.7, blue: 0.65, alpha: 1)
                context.fill(CGRect(x: 0, y: 0, width: 600, height: 840))
                context.setFillColor(red: 0.9, green: 0.94, blue: 0.87, alpha: 1)
                context.fill(CGRect(x: 70, y: 120, width: 460, height: 600))
                let file = store.incomingDirectory.appendingPathComponent("capture-\(sequence).png")
                let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
                CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
                guard CGImageDestinationFinalize(destination) else { throw ScannerError("test", "Cannot write fixture PNG") }
                try await store.preserve(file, sequence: sequence)
            }
            try await Task.sleep(for: .seconds(2))
            return CaptureOutcome()
        } catch is CancellationError { return CaptureOutcome(cancelled: true) }
    }
}

/// Explicit opt-in, isolated browser integration fixture. Never instantiates ImageCaptureCore.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_BROWSER_FIXTURE"] == "1"))
@MainActor func serveBrowserFixture() async throws {
    let env = ProcessInfo.processInfo.environment
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("w-" + String(UUID().uuidString.prefix(12)))
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = BrowserMock(); let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    let socket = root.appendingPathComponent("c.sock").path
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: coordinator, backendName: "browser-mock").respond)
    try server.start(); defer { server.stopAccepting() }
    let host = env["SCANNER_BROWSER_HOST"] ?? "127.0.0.1"
    let config = try GatewayConfiguration(host: host, port: 8171, allowRemote: true)
    let assets = try WebAssets(directory: #require(env["SCANNER_BROWSER_ASSETS"]))
    let gateway = try ScannerGateway(socketPath: socket, token: String(repeating: "e", count: 64), configuration: config, webAssets: assets)
    let app = Application(responder: gateway, configuration: .init(address: .hostname(host, port: 8171)))
    try await app.runService()
    await coordinator.shutdownScans()
}
