import Foundation

public struct RPCRequest: Codable, Sendable {
    public var jsonrpc: String
    public var id: JSONValue
    public var method: String
    public var params: JSONValue?
    public init(method: String, deviceID: String? = nil) {
        jsonrpc = "2.0"; id = .string(UUID().uuidString); self.method = method
        params = deviceID.map { .object(["deviceID": .string($0)]) }
    }
}
public struct RPCFailure: Error, Codable, Sendable {
    public var code: Int
    public var message: String
    public var data: ScannerError?
    public init(code: Int, message: String, data: ScannerError? = nil) {
        self.code = code; self.message = message; self.data = data
    }
}
public struct RPCResponse: Codable, Sendable {
    public var jsonrpc = "2.0"
    public var id: JSONValue
    public var result: JSONValue?
    public var error: RPCFailure?
    public init(id: JSONValue, result: JSONValue? = nil, error: RPCFailure? = nil) {
        self.id = id; self.result = result; self.error = error
    }
}
public struct DeviceList: Codable, Sendable {
    public var schemaVersion = 1
    public var devices: [Device]
    public init(devices: [Device]) { self.devices = devices }
}
public struct SettingsReport: Codable, Sendable {
    public var schemaVersion = 1
    public var device: Device
    public var observedAt: Date
    public var units: [UnitSnapshot]
    public var warnings: [String]
    public var errors: [ScannerError]
    public init(_ report: Inspection) {
        device = report.device; observedAt = report.observedAt; units = report.units
        warnings = report.warnings; errors = report.errors
        for i in units.indices {
            for key in units[i].settings.keys {
                units[i].settings[key]?.allowed = nil
                units[i].settings[key]?.defaultValue = nil
            }
            for j in units[i].vendorFeatures.indices {
                units[i].vendorFeatures[j].fields = units[i].vendorFeatures[j].fields.filter { $0.key == "current" }
            }
        }
    }
}
public struct Diagnostics: Codable, Sendable {
    public var schemaVersion = 1
    public var operatingSystem: String
    public var backend: String
    public var devices: [Device]
    public var inspection: Inspection?
    public var warnings: [String]
    public init(backend: String, devices: [Device], inspection: Inspection?, warnings: [String]) {
        operatingSystem = ProcessInfo.processInfo.operatingSystemVersionString
        self.backend = backend; self.devices = devices; self.inspection = inspection; self.warnings = warnings
    }
}
