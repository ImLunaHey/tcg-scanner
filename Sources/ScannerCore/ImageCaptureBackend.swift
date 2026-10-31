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
    private let log: @Sendable (String) -> Void

    public init(log: @escaping @Sendable (String) -> Void = { _ in }) { self.log = log }

    public func devices() async throws -> [Device] {
        if !started {
            browser.delegate = self
            browser.browsedDeviceTypeMask = ICDeviceTypeMask(rawValue:
                ICDeviceTypeMask.scanner.rawValue | ICDeviceLocationTypeMask.local.rawValue)!
            started = true
            browser.start()
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
        guard activeSession == nil else { throw ScannerError("busy", "A scanner session is already active.") }
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

    private var scanners: [ICScannerDevice] { (browser.devices ?? []).compactMap { $0 as? ICScannerDevice } }
    private static func describe(_ s: ICScannerDevice) -> Device {
        Device(id: s.uuidString ?? "usb:\(s.usbVendorID):\(s.usbProductID):\(s.usbLocationID)",
               name: s.name ?? "Unnamed scanner", transport: s.transportType,
               usbVendorID: s.usbVendorID, usbProductID: s.usbProductID,
               driverPath: s.modulePath, driverModuleVersion: s.moduleVersion)
    }
    public func deviceBrowser(_ browser: ICDeviceBrowser, didAdd device: ICDevice, moreComing: Bool) {
        log("device connected: \(device.name ?? "unnamed")")
    }
    public func deviceBrowser(_ browser: ICDeviceBrowser, didRemove device: ICDevice, moreGoing: Bool) {
        log("device disconnected: \(device.name ?? "unnamed")")
    }
    public func deviceBrowserDidEnumerateLocalDevices(_ browser: ICDeviceBrowser) {
        enumerated = true
        log("local discovery complete")
    }
}
