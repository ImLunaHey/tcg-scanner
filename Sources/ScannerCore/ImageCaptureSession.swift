import Foundation
import ImageCaptureCore
import ScannerAPI

struct ScanButtonTrigger {
    private(set) var accepted = false
    mutating func receive(isScan: Bool, documentLoaded: Bool) {
        if isScan && documentLoaded { accepted = true }
    }
}

@MainActor
final class ImageCaptureSession {
    let connection: ImageCaptureConnection
    private let device: Device
    init(scanner: ICScannerDevice, device: Device, log: @escaping @Sendable (String) -> Void) {
        connection = ImageCaptureConnection(scanner: scanner, log: log); self.device = device
    }
    func waitForScanButton() async throws {
        var failure: Error?
        do {
            try await connection.connect()
            try await connection.select(.documentFeeder)
            guard let feeder = connection.scanner.selectedFunctionalUnit as? ICScannerFunctionalUnitDocumentFeeder else {
                throw ScannerError("unsupported", "Scan-button batches require an ADF.")
            }
            // Only the installed driver's already-enabled, documented feature is used.
            // Never change vendor or transport-safety settings to enable the listener.
            guard let feature = feeder.vendorFeatures?.first(where: { $0.internalName == "scan_button_monitor" }),
                  Feature(feature).fields["current"] == .number(1) else {
                throw ScannerError("unsupported", "Enable Scan button monitor in the official driver before using automatic batches.")
            }
            var trigger = ScanButtonTrigger()
            connection.onButton = { [connection] button in
                guard button == ICButtonTypeScan, !trigger.accepted else { return }
                if !feeder.documentLoaded {
                    connection.log("button-monitor: Scan press ignored; ADF is empty")
                }
                trigger.receive(isScan: true, documentLoaded: feeder.documentLoaded)
            }
            connection.log("button-monitor: ARMED; Scan starts a 600 DPI duplex PNG card batch when paper is loaded")
            while !trigger.accepted {
                connection.traceButtonFeederState()
                try Task.checkCancellation()
                if let error = connection.failure { throw error }
                try await Task.sleep(for: .milliseconds(50))
            }
            try Task.checkCancellation()
        } catch { failure = error }
        connection.onButton = nil
        let cleanup = await connection.close()
        connection.log("button-monitor: automatic listener released; sessionClosed=\(connection.sessionClosed)")
        // Failed cleanup must take precedence over cancellation and button delivery.
        if let error = cleanup.first(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) ?? cleanup.first { throw error }
        if let failure { throw failure }
        guard connection.sessionClosed else { throw ScannerError("cleanup", "Scan-button session did not close.") }
    }
    func observeButton(seconds: Int) async -> [ScannerError] {
        var errors: [ScannerError] = []
        do {
            try await connection.connect()
            try await connection.select(.documentFeeder)
            let scanner = connection.scanner
            if let feature = scanner.selectedFunctionalUnit.vendorFeatures?.first(where: { $0.internalName == "scan_button_monitor" }) {
                let data = try Wire.encoder().encode(Feature(feature))
                connection.log("button-monitor: driver feature \(String(decoding: data, as: UTF8.self))")
            }
            connection.log("button-monitor: SESSION READY for \(seconds) seconds; no scan command or driver setting change")
            let deadline = ContinuousClock.now + .seconds(seconds)
            while ContinuousClock.now < deadline {
                connection.traceButtonFeederState()
                try Task.checkCancellation()
                if let failure = connection.failure { throw failure }
                try await Task.sleep(for: .milliseconds(250))
            }
        } catch is CancellationError {
            connection.log("button-monitor: observation cancelled")
        } catch { errors.append(ImageCaptureConnection.convert(error)) }
        errors += await connection.close()
        connection.log("button-monitor: observation ended; sessionClosed=\(connection.sessionClosed)")
        return errors
    }
    func inspect() async -> Inspection {
        var report = Inspection(device: device)
        do {
            try await connection.connect()
            let scanner = connection.scanner
            report.availableFunctionalUnitTypes = scanner.availableFunctionalUnitTypes.map(\.uintValue)
            for raw in report.availableFunctionalUnitTypes {
                guard let type = ICScannerFunctionalUnitType(rawValue: raw) else {
                    throw ScannerError("unsupportedUnit", "Unknown functional unit type \(raw)")
                }
                try await connection.select(type)
                report.units.append(UnitSnapshot(scanner.selectedFunctionalUnit))
                connection.log("inspected functional unit \(raw)")
            }
        } catch { report.errors.append(ImageCaptureConnection.convert(error)) }
        for error in await connection.close() where !report.errors.contains(error) { report.errors.append(error) }
        report.sessionOpened = connection.sessionOpened; report.sessionClosed = connection.sessionClosed
        report.observedAt = Date(); report.warnings = InspectionWarnings.evaluate(report.units)
        return report
    }
}
