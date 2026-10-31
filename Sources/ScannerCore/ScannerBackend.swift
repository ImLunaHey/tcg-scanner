import Foundation
import ScannerAPI

public protocol ScannerBackend: Sendable {
    func devices() async throws -> [Device]
    func inspect(deviceID: String) async throws -> Inspection
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome
}

public extension ScannerBackend {
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        throw ScannerError("unsupported", "This backend does not implement scanning.")
    }
}

/// Guards against actor reentrancy: a second request cannot inspect hardware
/// while the first request is suspended awaiting a driver callback.
public actor ScannerCoordinator {
    let backend: any ScannerBackend
    var inspecting = false
    var stopping = false
    var scans: [UUID: ScanStorage] = [:]
    var requests: [UUID: ScanRequest] = [:]
    var scanTask: Task<Void, Never>?
    var activeScanID: UUID?
    let storageRoot: URL
    public init(backend: any ScannerBackend, storageRoot: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Scans")) {
        self.backend = backend; self.storageRoot = storageRoot
    }
    public func devices() async throws -> [Device] { try await backend.devices() }
    public func inspect(deviceID: String?) async throws -> Inspection {
        guard !inspecting && !stopping else { throw ScannerError("busy", "Scanner is busy or shutting down; retry shortly.") }
        inspecting = true
        defer { inspecting = false }
        let selected = try await selectDevice(deviceID)
        return try await backend.inspect(deviceID: selected.id)
    }
    func selectDevice(_ deviceID: String?) async throws -> Device {
        let devices = try await backend.devices()
        let selected: Device
        if let deviceID {
            guard let d = devices.first(where: { $0.id == deviceID }) else {
                throw ScannerError("deviceNotFound", "No scanner matches device ID \(deviceID).")
            }
            selected = d
        } else {
            guard !devices.isEmpty else { throw ScannerError("deviceNotFound", "No ICA scanner found. Check USB, power and the installed macOS driver.") }
            guard devices.count == 1 else { throw ScannerError("ambiguousDevice", "Multiple scanners found. Specify --device using an ID from devices.") }
            selected = devices[0]
        }
        return selected
    }

}
