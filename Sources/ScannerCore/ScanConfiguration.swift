import Foundation
import ImageCaptureCore
import ScannerAPI

@MainActor
final class ScanConfiguration {
    private var retainedValues: [NSNumber] = []
    private var restoreActions: [() -> Void] = []
    func restore() { for action in restoreActions.reversed() { action() } }

    func apply(to scanner: ICScannerDevice, request: ScanRequest, incoming: URL) throws -> [String: JSONValue] {
        try request.validate()
        guard let feeder = scanner.selectedFunctionalUnit as? ICScannerFunctionalUnitDocumentFeeder else {
            throw ScannerError("unsupportedSetting", "Basic scanning requires an ADF.")
        }
        guard feeder.supportedResolutions.contains(request.dpi), feeder.supportedBitDepths.contains(8),
              feeder.supportedMeasurementUnits.contains(0), feeder.supportedDocumentTypes.contains(1) else {
            throw ScannerError("unsupportedSetting", "Driver must expose the requested DPI, 8-bit depth, inches and A4.")
        }
        guard !request.duplex || feeder.supportsDuplexScanning else { throw ScannerError("unsupportedSetting", "Duplex is not supported by this driver.") }
        guard feeder.documentLoaded else { throw ScannerError("feederEmpty", "Load the intended test media before starting a scan.") }
        guard !feeder.scanInProgress else { throw ScannerError("busy", "Scanner is already scanning.") }
        let snapshot = (feeder.measurementUnit, feeder.documentType, feeder.pixelDataType, feeder.bitDepth,
                        feeder.resolution, feeder.duplexScanningEnabled, feeder.scanArea)
        restoreActions.append {
            feeder.measurementUnit = snapshot.0; feeder.documentType = snapshot.1
            feeder.pixelDataType = snapshot.2; feeder.bitDepth = snapshot.3; feeder.resolution = snapshot.4
            feeder.duplexScanningEnabled = snapshot.5; feeder.scanArea = snapshot.6
        }
        // No multifeed, paper-thickness or transport-safety setting is written.
        try setFeature("blank_page_skip", value: 0, on: feeder)
        // Driver-defined uncropped modes: 0 = black backing, 1 = white backing.
        let background = request.background ?? .black
        let cropping = request.overscan == true ? 4 : (background == .white ? 1 : 0)
        try setFeature("cropping", value: cropping, on: feeder)
        feeder.measurementUnit = ICScannerMeasurementUnit(rawValue: 0)!
        feeder.documentType = ICScannerDocumentType(rawValue: 1)!
        feeder.pixelDataType = .RGB
        feeder.bitDepth = ICScannerBitDepth(rawValue: 8)!
        feeder.resolution = request.dpi
        feeder.duplexScanningEnabled = request.duplex
        let size = feeder.documentSize
        guard size.width.isFinite, size.height.isFinite, size.width > 8, size.width < 9,
              size.height > 11, size.height < 12 else { throw ScannerError("invalidSettings", "Driver did not provide usable A4 dimensions in inches.") }
        feeder.scanArea = NSRect(origin: .zero, size: size)
        guard feeder.measurementUnit.rawValue == 0, feeder.documentType.rawValue == 1,
              feeder.pixelDataType == .RGB, feeder.bitDepth.rawValue == 8,
              feeder.resolution == request.dpi, feeder.duplexScanningEnabled == request.duplex,
              abs(feeder.scanArea.width - size.width) < 0.01, abs(feeder.scanArea.height - size.height) < 0.01 else {
            throw ScannerError("invalidSettings", "Driver rejected one or more explicit scan settings; scan not started.")
        }
        let transfer = (scanner.transferMode, scanner.downloadsDirectory, scanner.documentName, scanner.documentUTI)
        restoreActions.append {
            scanner.transferMode = transfer.0; scanner.downloadsDirectory = transfer.1
            scanner.documentName = transfer.2; scanner.documentUTI = transfer.3
        }
        scanner.transferMode = .fileBased
        scanner.downloadsDirectory = incoming
        scanner.documentName = "capture-" + request.requestID.uuidString.lowercased()
        scanner.documentUTI = request.format == .tiff ? "public.tiff" : "public.png"
        return ["paper": .string("A4"), "dpi": .number(Double(request.dpi)), "duplex": .bool(request.duplex),
                "pixelMode": .string("RGB"), "bitsPerChannel": .number(8), "format": .string(request.format.rawValue),
                "scanAreaInches": .array([.number(0), .number(0), .number(size.width), .number(size.height)]),
                "blankPageSkip": .number(0), "cropping": .number(Double(cropping)),
                "background": .string(request.overscan == true ? "driverDetermined" : background.rawValue),
                "overscan": .bool(request.overscan == true), "multifeed": .string("unchanged")]
    }
    private func setFeature(_ name: String, value: Int, on unit: ICScannerFunctionalUnit) throws {
        guard let f = unit.vendorFeatures?.first(where: { $0.internalName == name }) as? ICScannerFeatureEnumeration,
              let chosen = f.values.first(where: { $0.intValue == value }) else {
            throw ScannerError("unsupportedSetting", "Driver does not expose required \(name)=\(value); scan not started.")
        }
        let previous = f.currentValue
        restoreActions.append { f.currentValue = previous }
        retainedValues.append(chosen)
        f.currentValue = chosen
        guard (f.currentValue as? NSNumber)?.intValue == value else {
            throw ScannerError("invalidSettings", "Driver rejected \(name)=\(value); scan not started.")
        }
    }
}
