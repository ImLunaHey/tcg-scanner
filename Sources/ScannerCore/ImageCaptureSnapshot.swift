import Foundation
import CoreFoundation
import ImageCaptureCore
import ScannerAPI

typealias Value = JSONValue

extension JSONValue {
    init(_ value: Any) {
        switch value {
        case let n as NSNumber:
            self = CFGetTypeID(n) == CFBooleanGetTypeID() ? .bool(n.boolValue) : .number(n.doubleValue)
        case let s as String: self = .string(s)
        case let a as [Any]: self = .array(a.map(JSONValue.init))
        case let d as [String: Any]: self = .object(d.mapValues(JSONValue.init))
        case is NSNull: self = .null
        default: self = .object(["unrecognizedType": .string(String(describing: type(of: value)))])
        }
    }
}

extension Feature {
    init(_ f: ICScannerFeature) {
        self.init()
        internalName = f.internalName; label = f.humanReadableName
        tooltip = f.tooltip; type = f.type.rawValue; fields = [:]
        switch f {
        case let v as ICScannerFeatureEnumeration:
            readOnly = false
            fields = ["current": Value(v.currentValue), "default": Value(v.defaultValue),
                      "values": Value(v.values), "labels": Value(v.menuItemLabels),
                      "tooltips": Value(v.menuItemLabelsTooltips)]
        case let v as ICScannerFeatureRange:
            readOnly = false
            fields = ["current": .number(v.currentValue), "default": .number(v.defaultValue),
                      "minimum": .number(v.minValue), "maximum": .number(v.maxValue),
                      "step": .number(v.stepSize)]
        case let v as ICScannerFeatureBoolean:
            readOnly = false; fields = ["current": .bool(v.value)]
        case let v as ICScannerFeatureTemplate:
            readOnly = true; fields = ["targets": Value(v.targets)]
        default: readOnly = nil
        }
    }
}

extension UnitSnapshot {
    init(_ u: ICScannerFunctionalUnit) {
        self.init()
        type = u.type.rawValue; className = NSStringFromClass(Swift.type(of: u))
        vendorFeatures = (u.vendorFeatures ?? []).map(Feature.init)
        templates = u.templates.map(Feature.init)
        settings = [:]
        func number<T: BinaryInteger>(_ n: T) -> Value { .number(Double(n)) }
        func indices(_ s: IndexSet) -> Value { .array(s.map { .number(Double($0)) }) }
        func size(_ s: NSSize) -> Value { .array([.number(s.width), .number(s.height)]) }
        settings["resolutionDPI"] = Setting(current: number(u.resolution), allowed: indices(u.supportedResolutions), readOnly: false)
        settings["preferredResolutionsDPI"] = Setting(current: indices(u.preferredResolutions), readOnly: true)
        settings["nativeResolutionDPI"] = Setting(current: .array([number(u.nativeXResolution), number(u.nativeYResolution)]), readOnly: true)
        settings["pixelDataType"] = Setting(current: number(u.pixelDataType.rawValue), readOnly: false)
        settings["bitDepth"] = Setting(current: number(u.bitDepth.rawValue), allowed: indices(u.supportedBitDepths), readOnly: false)
        settings["measurementUnit"] = Setting(current: number(u.measurementUnit.rawValue), allowed: indices(u.supportedMeasurementUnits), readOnly: false)
        settings["physicalSize"] = Setting(current: size(u.physicalSize), readOnly: true)
        let r = u.scanArea
        settings["scanAreaXYWH"] = Setting(current: .array([.number(r.origin.x), .number(r.origin.y), .number(r.width), .number(r.height)]), readOnly: false)
        settings["scanAreaOrientation"] = Setting(current: number(u.scanAreaOrientation.rawValue), readOnly: false)
        settings["scaleFactorPercent"] = Setting(current: number(u.scaleFactor), allowed: indices(u.supportedScaleFactors), readOnly: false)
        settings["preferredScaleFactors"] = Setting(current: indices(u.preferredScaleFactors), readOnly: true)
        settings["acceptsThreshold"] = Setting(current: .bool(u.acceptsThresholdForBlackAndWhiteScanning), readOnly: true)
        settings["usesThreshold"] = Setting(current: .bool(u.usesThresholdForBlackAndWhiteScanning), readOnly: false)
        settings["threshold"] = Setting(current: .number(Double(u.thresholdForBlackAndWhiteScanning)), defaultValue: .number(Double(u.defaultThresholdForBlackAndWhiteScanning)), readOnly: false)
        settings["state"] = Setting(current: number(u.state.rawValue), readOnly: true)
        settings["scanInProgress"] = Setting(current: .bool(u.scanInProgress), readOnly: true)
        settings["scanProgressPercent"] = Setting(current: .number(u.scanProgressPercentDone), readOnly: true)
        settings["canPerformOverviewScan"] = Setting(current: .bool(u.canPerformOverviewScan), readOnly: true)
        settings["overviewScanInProgress"] = Setting(current: .bool(u.overviewScanInProgress), readOnly: true)
        settings["overviewResolution"] = Setting(current: number(u.overviewResolution), readOnly: false)
        if let f = u as? ICScannerFunctionalUnitDocumentFeeder {
            settings["documentType"] = Setting(current: number(f.documentType.rawValue), allowed: indices(f.supportedDocumentTypes), readOnly: false)
            settings["documentSize"] = Setting(current: size(f.documentSize), readOnly: true)
            settings["supportsDuplexScanning"] = Setting(current: .bool(f.supportsDuplexScanning), readOnly: true)
            settings["duplexScanningEnabled"] = Setting(current: .bool(f.duplexScanningEnabled), readOnly: false)
            settings["documentLoaded"] = Setting(current: .bool(f.documentLoaded), readOnly: true)
            settings["oddPageOrientation"] = Setting(current: number(f.oddPageOrientation.rawValue), readOnly: false)
            settings["evenPageOrientation"] = Setting(current: number(f.evenPageOrientation.rawValue), readOnly: false)
            settings["reverseFeederPageOrder"] = Setting(current: .bool(f.reverseFeederPageOrder), readOnly: true)
        }
    }
}
