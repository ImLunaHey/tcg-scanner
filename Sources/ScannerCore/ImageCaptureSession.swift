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
