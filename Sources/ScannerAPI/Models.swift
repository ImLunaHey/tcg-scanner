import Foundation

public enum Evidence: String, Codable, Sendable {
    case hardwareTested, driverExposedNotTested, softwareImplemented, unsupported, unknown
}

public struct Device: Codable, Sendable, Equatable {
    public var id: String
    public var name: String
    public var transport: String?
    public var usbVendorID: Int32
    public var usbProductID: Int32
    public var driverPath: String
    public var driverModuleVersion: String?
    public init(id: String, name: String, transport: String? = nil, usbVendorID: Int32 = 0,
                usbProductID: Int32 = 0, driverPath: String = "", driverModuleVersion: String? = nil) {
        self.id = id; self.name = name; self.transport = transport
        self.usbVendorID = usbVendorID; self.usbProductID = usbProductID
        self.driverPath = driverPath; self.driverModuleVersion = driverModuleVersion
    }
}

public struct Setting: Codable, Sendable {
    public var current: JSONValue
    public var allowed: JSONValue?
    public var defaultValue: JSONValue?
    public var readOnly: Bool
    public var evidence: Evidence
    public init(current: JSONValue, allowed: JSONValue? = nil, defaultValue: JSONValue? = nil,
                readOnly: Bool, evidence: Evidence = .driverExposedNotTested) {
        self.current = current; self.allowed = allowed; self.defaultValue = defaultValue
        self.readOnly = readOnly; self.evidence = evidence
    }
}

public struct Feature: Codable, Sendable {
    public var internalName: String?
    public var label: String?
    public var tooltip: String?
    public var type: UInt = 0
    public var fields: [String: JSONValue] = [:]
    public var readOnly: Bool?
    public var evidence: Evidence = .driverExposedNotTested
    public init() {}
}

public struct UnitSnapshot: Codable, Sendable {
    public var type: UInt = 0
    public var className = ""
    public var settings: [String: Setting] = [:]
    public var vendorFeatures: [Feature] = []
    public var templates: [Feature] = []
    public init() {}
}

public struct Inspection: Codable, Sendable {
    public var schemaVersion = 1
    public var observedAt = Date()
    public var device: Device
    public var sessionOpened = false
    public var sessionClosed = false
    public var availableFunctionalUnitTypes: [UInt] = []
    public var units: [UnitSnapshot] = []
    public var errors: [ScannerError] = []
    public var warnings: [String] = []
    public var unknownCapabilities = ["firmware", "maintenanceCounters", "paperThickness", "continuousScanning",
                                      "supportedPixelModes", "automaticOrientation", "deskew", "brightness", "contrast", "gamma"]
    public var limitations = [
        "Property reads only; no scan or setting write was requested.",
        "Current values are not defaults. Missing values remain unknown.",
        "Writable means an API setter exists, not that setting writes were tested.",
        "Units, document types and orientations use ImageCaptureCore enum codes.",
        "Dimensions use the reported measurement unit. Resolution is DPI; bit depth is per channel.",
        "This initial backend expands ADF-specific fields and common fields for other functional units."
    ]
    public init(device: Device) { self.device = device }
}

public struct ScannerError: Error, Codable, Sendable, LocalizedError, Equatable {
    public var kind: String
    public var message: String
    public var domain: String?
    public var code: Int?
    public var errorDescription: String? { message }
    public init(_ kind: String, _ message: String, domain: String? = nil, code: Int? = nil) {
        self.kind = kind; self.message = message; self.domain = domain; self.code = code
    }
    public init(_ operation: String, error: Error) {
        let n = error as NSError
        self.init(operation, n.localizedDescription, domain: n.domain, code: n.code)
    }
}
