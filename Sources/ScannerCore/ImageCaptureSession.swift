import Foundation
import ImageCaptureCore
import ScannerAPI

@MainActor
final class ImageCaptureSession: NSObject, @preconcurrency ICScannerDeviceDelegate {
    private let scanner: ICScannerDevice
    private var report: Inspection
    private let log: @Sendable (String) -> Void
    private var openCompleted = false
    private var ready = false
    private var selected = false
    private var closing = false
    private var closeCompleted = false
    private var failure: ScannerError?

    init(scanner: ICScannerDevice, device: Device, log: @escaping @Sendable (String) -> Void) {
        self.scanner = scanner; self.report = Inspection(device: device); self.log = log
    }

    func inspect() async -> Inspection {
        scanner.delegate = self
        defer { scanner.delegate = nil }
        requestOpen()
        var original: ICScannerFunctionalUnitType?
        do {
            try await wait("open session") { openCompleted }
            try await wait("device readiness") { ready }
            report.availableFunctionalUnitTypes = scanner.availableFunctionalUnitTypes.map(\.uintValue)
            original = scanner.selectedFunctionalUnit.type
            for raw in report.availableFunctionalUnitTypes {
                guard let type = ICScannerFunctionalUnitType(rawValue: raw) else {
                    throw ScannerError("unsupportedUnit", "Unknown functional unit type \(raw)")
                }
                try await select(type)
                report.units.append(UnitSnapshot(scanner.selectedFunctionalUnit))
                log("inspected functional unit \(raw)")
            }
        } catch { report.errors.append(Self.convert(error)) }

        // A fresh task allows cleanup to finish even if the request was cancelled.
        await Task { @MainActor in
            if let original, report.sessionOpened, !closeCompleted {
                do { try await select(original) }
                catch { report.errors.append(Self.convert(error)) }
            }
            if !closeCompleted && (report.sessionOpened || !openCompleted) {
                closing = true
                requestClose()
                let deadline = ContinuousClock.now + .seconds(10)
                while !closeCompleted && ContinuousClock.now < deadline {
                    try? await Task.sleep(for: .milliseconds(50))
                }
                if !closeCompleted {
                    report.errors.append(ScannerError("cleanup", "Scanner session closure was not confirmed."))
                }
            }
        }.value
        if let failure, !report.errors.contains(failure) { report.errors.append(failure) }
        report.observedAt = Date()
        report.warnings = InspectionWarnings.evaluate(report.units)
        return report
    }

    // Select the delegate-based overload so all requests have bounded waits.
    private func requestOpen() { scanner.requestOpenSession() }
    private func requestClose() { scanner.requestCloseSession() }

    private func select(_ type: ICScannerFunctionalUnitType) async throws {
        if scanner.selectedFunctionalUnit.type == type { return }
        selected = false
        scanner.requestSelect(type)
        try await wait("select functional unit") { selected }
        guard scanner.selectedFunctionalUnit.type == type else {
            throw ScannerError("driver", "Driver selected a different functional unit.")
        }
    }
    private func wait(_ operation: String, until done: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(15)
        while !done() {
            if let failure { throw failure }
            guard ContinuousClock.now < deadline else { throw ScannerError("timeout", "Timed out waiting to \(operation).") }
            try await Task.sleep(for: .milliseconds(50))
        }
        try Task.checkCancellation()
        if let failure { throw failure }
    }
    private static func convert(_ error: Error) -> ScannerError {
        if let e = error as? ScannerError { return e }
        if error is CancellationError { return ScannerError("cancelled", "Inspection cancelled.") }
        return ScannerError("driver", error: error)
    }
    func device(_ device: ICDevice, didOpenSessionWithError error: Error?) {
        openCompleted = true
        if let error { failure = ScannerError("openSession", error: error) }
        else { report.sessionOpened = true; log("session opened") }
    }
    func deviceDidBecomeReady(_ device: ICDevice) { ready = true; log("device ready") }
    func device(_ device: ICDevice, didCloseSessionWithError error: Error?) {
        closeCompleted = true
        if let error { failure = ScannerError("closeSession", error: error) }
        else { report.sessionClosed = true; log("session closed") }
        if !closing { failure = ScannerError("disconnected", "Scanner session closed unexpectedly.") }
    }
    func didRemove(_ device: ICDevice) { failure = ScannerError("disconnected", "Scanner disconnected during inspection.") }
    func device(_ device: ICDevice, didEncounterError error: Error?) {
        if let error { failure = ScannerError("driver", error: error) }
    }
    func scannerDevice(_ scanner: ICScannerDevice, didSelect functionalUnit: ICScannerFunctionalUnit, error: Error?) {
        if let error { failure = ScannerError("selectUnit", error: error) }
        selected = true
    }
}
