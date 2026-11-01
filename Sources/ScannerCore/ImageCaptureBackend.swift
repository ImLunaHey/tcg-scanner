import Foundation
import ImageCaptureCore
import ScannerAPI

/// Must be hosted by a process pumping the macOS main run loop.
@MainActor
public final class ImageCaptureBackend: NSObject, ScannerBackend, @preconcurrency ICDeviceBrowserDelegate {
    private let browser = ICDeviceBrowser()
    private var needsRestart = false
    private var started = false
    private var enumerated = false
    private var activeSession: ImageCaptureSession?
    private var activeScan: ImageCaptureScanSession?
    private let log: @Sendable (String) -> Void

    public init(log: @escaping @Sendable (String) -> Void = { _ in }) { self.log = log }

    public func devices() async throws -> [Device] {
        if !started {
            browser.delegate = self
            browser.browsedDeviceTypeMask = ICDeviceTypeMask(rawValue:
                ICDeviceTypeMask.scanner.rawValue | ICDeviceLocationTypeMask.local.rawValue)!
            started = true
            browser.start()
            log("button-monitor: browser listener started; diagnostic only, no scan action")
        }
        let deadline = ContinuousClock.now + .seconds(15)
        while !enumerated {
            guard ContinuousClock.now < deadline else {
                throw ScannerError("timeout", "ImageCaptureCore local discovery did not complete within 15 seconds.")
            }
            try await Task.sleep(for: .milliseconds(50))
        }
        // The driver's add callback can follow the local enumeration callback.
        try await Task.sleep(for: .milliseconds(500))
        return scanners.map(Self.describe).sorted { $0.id < $1.id }
    }

    public func inspect(deviceID: String) async throws -> Inspection {
        guard !needsRestart else { throw ScannerError("cleanup", "Session closure was not confirmed; restart ScannerService before retrying.") }
        guard activeSession == nil && activeScan == nil else { throw ScannerError("busy", "A scanner session is already active.") }
        guard let scanner = scanners.first(where: { Self.describe($0).id == deviceID }) else {
            throw ScannerError("deviceNotFound", "Scanner disconnected before inspection.")
        }
        let session = ImageCaptureSession(scanner: scanner, device: Self.describe(scanner), log: log)
        activeSession = session
        defer { activeSession = nil }
        let report = await session.inspect()
        if report.errors.contains(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) { needsRestart = true }
        return report
    }

    public func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        guard !needsRestart else { throw ScannerError("cleanup", "Restart ScannerService after the previous unconfirmed session cleanup.") }
        guard activeSession == nil && activeScan == nil else { throw ScannerError("busy", "A scanner session is active.") }
        guard let scanner = scanners.first(where: { Self.describe($0).id == deviceID }) else { throw ScannerError("deviceNotFound", "Scanner disconnected.") }
        let session = ImageCaptureScanSession(scanner: scanner, store: store, log: log)
        activeScan = session
        defer { activeScan = nil }
        let outcome = await session.capture(request)
        if outcome.errors.contains(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) || !outcome.sessionClosed { needsRestart = true }
        return outcome
    }

    private var scanners: [ICScannerDevice] { (browser.devices ?? []).compactMap { $0 as? ICScannerDevice } }
    private static func describe(_ s: ICScannerDevice) -> Device {
        Device(id: s.uuidString ?? "usb:\(s.usbVendorID):\(s.usbProductID):\(s.usbLocationID)",
               name: s.name ?? "Unnamed scanner", transport: s.transportType,
               usbVendorID: s.usbVendorID, usbProductID: s.usbProductID,
               driverPath: s.modulePath, driverModuleVersion: s.moduleVersion)
    }
    public func deviceBrowser(_ browser: ICDeviceBrowser, didAdd device: ICDevice, moreComing: Bool) {
        log("device connected: \(device.name ?? "unnamed")")
        if device is ICScannerDevice {
            log("button-monitor: scanner discovered; id=\(device.uuidString ?? "unknown"); autolaunch=\(device.autolaunchApplicationPath ?? "unset")")
        }
    }
    /// Apple's documented selection callback can indicate a physical button press
    /// when this process is the event target and no device session is open.
    /// Log only: selection requests are not sufficient evidence to start a scan.
    public func deviceBrowser(_ browser: ICDeviceBrowser, requestsSelect device: ICDevice) {
        guard device is ICScannerDevice else { return }
        log("button-monitor: selection event received; id=\(device.uuidString ?? "unknown"); sessionOpen=\(device.hasOpenSession); no scan requested")
    }
    public func deviceBrowser(_ browser: ICDeviceBrowser, didRemove device: ICDevice, moreGoing: Bool) {
        log("device disconnected: \(device.name ?? "unnamed")")
    }
    public func deviceBrowserDidEnumerateLocalDevices(_ browser: ICDeviceBrowser) {
        enumerated = true
        log("local discovery complete")
    }
}
