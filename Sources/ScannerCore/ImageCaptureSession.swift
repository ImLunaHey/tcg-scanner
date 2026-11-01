import Foundation
import ImageCaptureCore
import ScannerAPI

@MainActor
final class ImageCaptureSession {
    private let connection: ImageCaptureConnection
    private let device: Device
    init(scanner: ICScannerDevice, device: Device, log: @escaping @Sendable (String) -> Void) {
        connection = ImageCaptureConnection(scanner: scanner, log: log); self.device = device
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
