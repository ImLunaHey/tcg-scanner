import Foundation
import ScannerAPI
import ScannerCore

public struct DiagnosticRouter: Sendable {
    let coordinator: ScannerCoordinator
    private let backendName: String
    public init(coordinator: ScannerCoordinator, backendName: String = "ImageCaptureCore") {
        self.coordinator = coordinator; self.backendName = backendName
    }
    public func respond(to bytes: Data) async -> RPCResponse {
        let request: RPCRequest
        do { request = try Wire.decoder().decode(RPCRequest.self, from: bytes) }
        catch {
            let validJSON = (try? JSONSerialization.jsonObject(with: bytes, options: .fragmentsAllowed)) != nil
            return RPCResponse(id: .null, error: RPCFailure(code: validJSON ? -32600 : -32700,
                message: validJSON ? "Invalid JSON-RPC request." : "Invalid JSON."))
        }
        guard request.jsonrpc == "2.0", validID(request.id) else {
            return RPCResponse(id: request.id, error: RPCFailure(code: -32600, message: "Expected JSON-RPC 2.0 and a string or numeric ID."))
        }
        do {
            if let response = try await identificationResponse(request) { return response }
            if let response = try await controlResponse(request) { return response }
            if let response = try await serviceResponse(request) { return response }
            if request.method.hasPrefix("v1.scan.") || request.method == "v1.jobs.create" {
                return try await scanResponse(request)
            }
            let deviceID = try parseParams(request.params)
            let result: JSONValue
            switch request.method {
            case "v1.devices":
                guard deviceID == nil else { throw RPCFailure(code: -32602, message: "devices takes no deviceID.") }
                result = try .wrapping(DeviceList(devices: await coordinator.devices()))
            case "v1.capabilities":
                result = try .wrapping(await coordinator.inspect(deviceID: deviceID))
            case "v1.settings":
                let report = try await coordinator.inspect(deviceID: deviceID)
                var settings = SettingsReport(report)
                settings.configuredSettings = try await coordinator.configuredSettings(report.device.id)
                result = try .wrapping(settings)
            case "v1.diagnostics":
                let devices = try await coordinator.devices()
                let report: Inspection?
                if deviceID != nil || devices.count == 1 { report = try await coordinator.inspect(deviceID: deviceID) }
                else { report = nil }
                result = try .wrapping(Diagnostics(backend: backendName, devices: devices, inspection: report,
                    warnings: devices.isEmpty ? ["No ICA scanner found. Check USB, power and the official macOS driver."] :
                        (devices.count > 1 && deviceID == nil ? ["Specify a device ID to inspect one of the connected scanners."] : [])))
            default: throw RPCFailure(code: -32601, message: "Unknown method: \(request.method)")
            }
            return RPCResponse(id: request.id, result: result)
        } catch let error as RPCFailure { return RPCResponse(id: request.id, error: error) }
        catch let error as ScannerError {
            return RPCResponse(id: request.id, error: RPCFailure(code: -32000, message: error.message, data: error))
        } catch {
            return RPCResponse(id: request.id, error: RPCFailure(code: -32603, message: "Internal diagnostic error."))
        }
    }
    private func validID(_ value: JSONValue) -> Bool {
        switch value { case .string, .number: true; default: false }
    }
    private func parseParams(_ params: JSONValue?) throws -> String? {
        guard let params else { return nil }
        guard case .object(let object) = params,
              object.keys.allSatisfy({ $0 == "deviceID" }) else {
            throw RPCFailure(code: -32602, message: "Only the optional deviceID parameter is accepted.")
        }
        guard let value = object["deviceID"] else { return nil }
        guard case .string(let id) = value, !id.isEmpty, id.utf8.count <= 512 else {
            throw RPCFailure(code: -32602, message: "deviceID must be a nonempty string of at most 512 bytes.")
        }
        return id
    }
}

private extension DiagnosticRouter {
    func scanResponse(_ request: RPCRequest) async throws -> RPCResponse {
        guard case .object(let params) = request.params else { throw RPCFailure(code: -32602, message: "Scan parameters must be an object.") }
        let record: ScanRecord
        switch request.method {
        case "v1.scan.start", "v1.jobs.create":
            let allowed: Set<String> = ["requestID", "deviceID", "outputDirectory", "dpi", "duplex", "format", "profileID", "useDeviceSettings", "prepareCards", "background", "overscan"]
            guard params.keys.allSatisfy(allowed.contains) else { throw RPCFailure(code: -32602, message: "Unknown scan parameter.") }
            let options: ScanRequest
            do { options = try JSONValue.object(params).decode(ScanRequest.self) }
            catch { throw RPCFailure(code: -32602, message: "Expected requestID (UUID), dpi (integer), duplex (boolean), format (tiff/png), optional deviceID/outputDirectory.") }
            record = try await coordinator.startScan(options)
        case "v1.scan.status", "v1.scan.cancel":
            guard params.count == 1, case .string(let value) = params["scanID"], let id = UUID(uuidString: value) else {
                throw RPCFailure(code: -32602, message: "Expected only scanID (UUID string).")
            }
            if request.method == "v1.scan.cancel" { record = try await coordinator.cancelScan(id) }
            else { record = try await coordinator.scanStatus(id) }
        default: throw RPCFailure(code: -32601, message: "Unknown scan method.")
        }
        return RPCResponse(id: request.id, result: try .wrapping(record))
    }
}
