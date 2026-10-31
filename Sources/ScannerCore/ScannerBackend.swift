import Foundation
import ScannerAPI
import ScannerPersistence

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
    var queue: [UUID] = []
    var paused = false
    var requiresRestart = false
    var persistenceError: ScannerError?
    var eventWaiters = 0
    var thumbnailWorkers = 0
    var editingJobs: Set<UUID> = []
    let database: ScannerDatabase
    var scanTask: Task<Void, Never>?
    var activeScanID: UUID?
    let storageRoot: URL
    let validateStorage: @Sendable () throws -> Void
    public init(backend: any ScannerBackend, storageRoot: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Scans"), database: ScannerDatabase? = nil, validateStorage: @escaping @Sendable () throws -> Void = {}) throws {
        self.backend = backend; self.storageRoot = storageRoot; self.validateStorage = validateStorage
        self.database = try database ?? ScannerDatabase()
    }
    public func devices() async throws -> [Device] {
        let devices = try await backend.devices()
        try database.observeDevices(devices)
        return devices
    }
    public func inspect(deviceID: String?) async throws -> Inspection {
        guard !inspecting && activeScanID == nil && !stopping && !paused && !requiresRestart else { throw ScannerError("busy", "Scanner is busy or shutting down; retry shortly.") }
        inspecting = true
        defer { inspecting = false; scheduleNext() }
        let selected = try await selectDevice(deviceID)
        let report = try await backend.inspect(deviceID: selected.id)
        try database.saveInspection(report)
        return report
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
