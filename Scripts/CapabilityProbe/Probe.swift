import Foundation
import ImageCaptureCore

struct ProbeError: Error, Encodable {
    let operation: String
    let domain: String
    let code: Int
    let message: String
    init(_ operation: String, _ error: Error) {
        let e = error as NSError
        self.operation = operation; domain = e.domain; code = e.code; message = e.localizedDescription
    }
    init(_ operation: String, message: String) {
        self.operation = operation; domain = "ScannerDiagnostic"; code = 1; self.message = message
    }
}

struct Report: Encodable {
    let schemaVersion = 1
    var observedAt = Date()
    var deviceName: String?
    var deviceID: String?
    var driverPath: String?
    var driverModuleVersion: String?
    var sessionOpened = false
    var sessionClosed = false
    var availableFunctionalUnitTypes: [UInt] = []
    var units: [UnitSnapshot] = []
    var errors: [ProbeError] = []
    let limitations = [
        "No scans or setting writes requested. Functional units may be selected for inspection.",
        "Values are driver-exposed, not scan-tested. Current values are not factory defaults.",
        "Writable means the public API has a setter; acceptance has not been tested.",
        "Pixel data type has no supported-values list in this public API; only current mode is reported.",
        "Missing capabilities are unknown, not proof of hardware non-support.",
        "Measurement units and document types use ImageCaptureCore raw enum values. Sizes use the reported measurement unit."
    ]
}

@MainActor
final class CapabilityProbe: NSObject, @preconcurrency ICDeviceBrowserDelegate, @preconcurrency ICScannerDeviceDelegate {
    let browser = ICDeviceBrowser()
    var scanner: ICScannerDevice?
    var report = Report()
    var openRequested = false
    var openCompleted = false
    var ready = false
    var selectionCompleted = false
    var closing = false

    func log(_ text: String) { FileHandle.standardError.write(Data("[capabilities] \(text)\n".utf8)) }
    func deviceBrowser(_ browser: ICDeviceBrowser, didAdd device: ICDevice, moreComing: Bool) {
        guard scanner == nil, device.usbVendorID == 0x04c5, device.usbProductID == 0x15ff,
              let s = device as? ICScannerDevice else { return }
        scanner = s; s.delegate = self
        log("Found \(s.name ?? "fi-8170")")
    }
    func deviceBrowser(_ browser: ICDeviceBrowser, didRemove device: ICDevice, moreGoing: Bool) {
        if device === scanner { didRemove(device) }
    }
    func didRemove(_ device: ICDevice) {
        report.errors.append(ProbeError("connection", message: "Scanner disconnected"))
    }
    func device(_ device: ICDevice, didOpenSessionWithError error: Error?) {
        openCompleted = true
        if let error { report.errors.append(ProbeError("openSession", error)) }
        else { report.sessionOpened = true; log("Session opened") }
    }
    func deviceDidBecomeReady(_ device: ICDevice) { ready = true; log("Device ready") }
    func device(_ device: ICDevice, didCloseSessionWithError error: Error?) {
        if let error { report.errors.append(ProbeError("closeSession", error)) }
        else { report.sessionClosed = true; log("Session closed") }
        if !closing { report.errors.append(ProbeError("session", message: "Session closed unexpectedly")) }
    }
    func device(_ device: ICDevice, didEncounterError error: Error?) {
        if let error { report.errors.append(ProbeError("device", error)) }
    }
    func scannerDevice(_ scanner: ICScannerDevice, didSelect functionalUnit: ICScannerFunctionalUnit, error: Error?) {
        if let error { report.errors.append(ProbeError("selectFunctionalUnit", error)) }
        selectionCompleted = true
        log("Functional unit callback: \(functionalUnit.type.rawValue)")
    }

    func wait(_ operation: String, seconds: Double = 15, until predicate: () -> Bool) throws {
        let deadline = Date().addingTimeInterval(seconds)
        while !predicate() {
            if let error = report.errors.first { throw error }
            guard Date() < deadline else { throw ProbeError(operation, message: "Timed out after \(seconds) seconds") }
            RunLoop.main.run(until: min(deadline, Date().addingTimeInterval(0.05)))
        }
        if let error = report.errors.first { throw error }
    }

    func run() -> Report {
        browser.delegate = self
        browser.browsedDeviceTypeMask = ICDeviceTypeMask(rawValue: ICDeviceTypeMask.scanner.rawValue | ICDeviceLocationTypeMask.local.rawValue)!
        browser.start()
        defer { scanner?.delegate = nil; browser.stop(); browser.delegate = nil }
        do {
            try wait("discovery") { scanner != nil }
            guard let s = scanner else { throw ProbeError("discovery", message: "No scanner") }
            report.deviceName = s.name; report.deviceID = s.uuidString
            report.driverPath = s.modulePath; report.driverModuleVersion = s.moduleVersion
            openRequested = true
            s.requestOpenSession()
            try wait("openSession") { openCompleted }
            try wait("deviceReady") { ready }
            report.availableFunctionalUnitTypes = s.availableFunctionalUnitTypes.map(\.uintValue)
            let original = s.selectedFunctionalUnit.type
            for raw in report.availableFunctionalUnitTypes {
                guard let type = ICScannerFunctionalUnitType(rawValue: raw) else {
                    throw ProbeError("functionalUnit", message: "Unknown functional unit type \(raw)")
                }
                if s.selectedFunctionalUnit.type != type {
                    selectionCompleted = false; s.requestSelect(type)
                    try wait("selectFunctionalUnit") { selectionCompleted }
                }
                guard s.selectedFunctionalUnit.type == type else {
                    throw ProbeError("functionalUnit", message: "Driver returned a different unit")
                }
                report.units.append(UnitSnapshot(s.selectedFunctionalUnit))
                log("Inspected unit \(raw), \(s.selectedFunctionalUnit.vendorFeatures?.count ?? 0) vendor features")
            }
            if s.selectedFunctionalUnit.type != original {
                selectionCompleted = false; s.requestSelect(original)
                try wait("restoreFunctionalUnit") { selectionCompleted }
            }
        } catch {
            let e = (error as? ProbeError) ?? ProbeError("inspection", error)
            if report.errors.isEmpty { report.errors.append(e) }
            log("\(e.operation): \(e.message)")
        }
        // Always request cleanup after an open attempt, including timeouts.
        if let s = scanner, openRequested {
            closing = true; s.requestCloseSession()
            let deadline = Date().addingTimeInterval(10)
            while !report.sessionClosed && Date() < deadline {
                RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            }
            if !report.sessionClosed { report.errors.append(ProbeError("closeSession", message: "Session closure not confirmed")) }
        }
        report.observedAt = Date()
        return report
    }
}

@main
struct CapabilityCommand {
    @MainActor static func main() throws {
        let report = CapabilityProbe().run()
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        FileHandle.standardOutput.write(try encoder.encode(report))
        FileHandle.standardOutput.write(Data("\n".utf8))
        if !report.errors.isEmpty { exit(1) }
    }
}
