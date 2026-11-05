import Foundation
import ImageCaptureCore
import ScannerAPI

/// Must be hosted by a process pumping the macOS main run loop.
@MainActor
public final class ImageCaptureBackend: NSObject, ScannerBackend, @preconcurrency ICDeviceBrowserDelegate {
    private let browser = ICDeviceBrowser()
    // Retain the exact device/session: a different scanner's removal cannot
    // release uncertain ownership, and a replacement may reuse the same UUID.
    private var blockedConnection: ImageCaptureConnection?
    private var needsRestart: Bool { blockedConnection != nil }
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
        guard !needsRestart else { throw ScannerError("cleanup", "Session closure was not confirmed; disconnect and reconnect the scanner before retrying.") }
        guard activeSession == nil && activeScan == nil else { throw ScannerError("busy", "A scanner session is already active.") }
        guard let scanner = scanners.first(where: { Self.describe($0).id == deviceID }) else {
            throw ScannerError("deviceNotFound", "Scanner disconnected before inspection.")
        }
        let session = ImageCaptureSession(scanner: scanner, device: Self.describe(scanner), log: log)
        activeSession = session
        defer { activeSession = nil }
        let report = await session.inspect()
        if report.errors.contains(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) { quarantine(session.connection) }
        return report
    }

    public func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        guard !needsRestart else { throw ScannerError("cleanup", "Disconnect and reconnect the scanner after unconfirmed session cleanup.") }
        guard activeSession == nil && activeScan == nil else { throw ScannerError("busy", "A scanner session is active.") }
        guard let scanner = scanners.first(where: { Self.describe($0).id == deviceID }) else { throw ScannerError("deviceNotFound", "Scanner disconnected.") }
        let session = ImageCaptureScanSession(scanner: scanner, store: store, log: log)
        activeScan = session
        defer { activeScan = nil }
        let outcome = await session.capture(request)
        if outcome.errors.contains(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) || !outcome.sessionClosed { quarantine(session) }
        return outcome
    }

    public func observeButton(deviceID: String, seconds: Int) async throws {
        guard (30...300).contains(seconds) else { throw ScannerError("invalidParams", "Button observation must last 30–300 seconds.") }
        guard !needsRestart, activeSession == nil, activeScan == nil else { throw ScannerError("busy", "Scanner session is unavailable.") }
        guard let scanner = scanners.first(where: { Self.describe($0).id == deviceID }) else { throw ScannerError("deviceNotFound", "Scanner disconnected.") }
        let session = ImageCaptureSession(scanner: scanner, device: Self.describe(scanner), log: log)
        activeSession = session
        defer { activeSession = nil }
        let errors = await session.observeButton(seconds: seconds)
        if errors.contains(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) { quarantine(session.connection) }
        if let error = errors.first(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) ?? errors.first { throw error }
    }

    public func waitForScanButton(deviceID: String) async throws {
        guard !needsRestart else { throw ScannerError("cleanup", "Disconnect and reconnect the scanner after unconfirmed session cleanup.") }
        guard activeSession == nil, activeScan == nil else { throw ScannerError("busy", "Scanner session is active.") }
        guard let scanner = scanners.first(where: { Self.describe($0).id == deviceID }) else { throw ScannerError("deviceNotFound", "Scanner disconnected.") }
        let session = ImageCaptureSession(scanner: scanner, device: Self.describe(scanner), log: log)
        activeSession = session
        defer { activeSession = nil }
        do { try await session.waitForScanButton() }
        catch {
            if let error = error as? ScannerError, error.kind == "cleanup" || error.kind == "closeSession" { quarantine(session.connection) }
            throw error
        }
    }

    func quarantine(_ connection: ImageCaptureConnection) { blockedConnection = connection }

    public func recoverAfterDisconnect() -> Bool {
        guard activeSession == nil, activeScan == nil else { return false }
        guard let blockedConnection else { return true } // No session was acquired.
        guard blockedConnection.removed else { return false }
        self.blockedConnection = nil
        log("Removed scanner session released; ready for discovery and a new job.")
        return true
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
        activeSession?.connection.didRemove(device)
        activeScan?.didRemove(device)
        blockedConnection?.didRemove(device)
        log("device disconnected: \(device.name ?? "unnamed")")
    }
    public func deviceBrowserDidEnumerateLocalDevices(_ browser: ICDeviceBrowser) {
        enumerated = true
        log("local discovery complete")
    }
}
