// Development-only discovery probe. Does not open sessions or request scans.
import Foundation
import ImageCaptureCore

struct DeviceReport: Encodable {
    let name: String?
    let uuid: String?
    let transport: String?
    let usbVendorID: Int32
    let usbProductID: Int32
    let driverPath: String
    let driverVersion: String?
}

struct DiscoveryReport: Encodable {
    let schemaVersion = 1
    let observedAt: Date
    let operatingSystem: String
    let observationSeconds: Double
    let localEnumerationCompleted: Bool
    let devices: [DeviceReport]
    let note: String
}

// ImageCaptureCore uses the main run loop. Runtime isolation checks verify
// delegate callbacks arrive on the main actor when importing the ObjC protocol.
@MainActor
final class DiscoveryProbe: NSObject, @preconcurrency ICDeviceBrowserDelegate {
    let browser = ICDeviceBrowser()
    var localEnumerationCompleted = false

    func deviceBrowser(_ browser: ICDeviceBrowser, didAdd device: ICDevice, moreComing: Bool) {
        log("Discovered: \(device.name ?? "unnamed scanner")")
    }

    func deviceBrowser(_ browser: ICDeviceBrowser, didRemove device: ICDevice, moreGoing: Bool) {
        log("Removed: \(device.name ?? "unnamed scanner")")
    }

    func deviceBrowserDidEnumerateLocalDevices(_ browser: ICDeviceBrowser) {
        localEnumerationCompleted = true
        log("Local enumeration callback received")
    }

    func run(seconds: Double) throws {
        browser.delegate = self
        browser.browsedDeviceTypeMask = ICDeviceTypeMask(
            rawValue: ICDeviceTypeMask.scanner.rawValue | ICDeviceLocationTypeMask.local.rawValue
        )!
        log("Observing local scanners for \(seconds) seconds; no sessions or scans requested")
        browser.start()
        defer {
            browser.stop()
            browser.delegate = nil
        }
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            RunLoop.main.run(until: min(deadline, Date().addingTimeInterval(0.1)))
        }
        let devices = (browser.devices ?? []).compactMap { device -> DeviceReport? in
            guard device is ICScannerDevice else { return nil }
            return DeviceReport(
                name: device.name, uuid: device.uuidString, transport: device.transportType,
                usbVendorID: device.usbVendorID, usbProductID: device.usbProductID,
                driverPath: device.modulePath, driverVersion: device.moduleVersion
            )
        }
        let report = DiscoveryReport(
            observedAt: Date(), operatingSystem: ProcessInfo.processInfo.operatingSystemVersionString,
            observationSeconds: seconds, localEnumerationCompleted: localEnumerationCompleted,
            devices: devices,
            note: "Discovery only. An empty list does not establish hardware incompatibility or permission status."
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        FileHandle.standardOutput.write(try encoder.encode(report))
        FileHandle.standardOutput.write(Data("\n".utf8))
        log("Found \(devices.count) scanner(s)")
    }

    private func log(_ message: String) {
        FileHandle.standardError.write(Data("[imagecapture-probe] \(message)\n".utf8))
    }
}

@main
struct ProbeCommand {
    @MainActor
    static func main() throws {
        try DiscoveryProbe().run(seconds: 15)
    }
}
