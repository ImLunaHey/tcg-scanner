import ScannerAPI

public protocol ScannerBackend: Sendable {
    func devices() async throws -> [Device]
    func inspect(deviceID: String) async throws -> Inspection
}

/// Guards against actor reentrancy: a second request cannot inspect hardware
/// while the first request is suspended awaiting a driver callback.
public actor ScannerCoordinator {
    private let backend: any ScannerBackend
    private var inspecting = false
    public init(backend: any ScannerBackend) { self.backend = backend }
    public func devices() async throws -> [Device] { try await backend.devices() }
    public func inspect(deviceID: String?) async throws -> Inspection {
        guard !inspecting else { throw ScannerError("busy", "Another diagnostic session is active; retry shortly.") }
        inspecting = true
        defer { inspecting = false }
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
        return try await backend.inspect(deviceID: selected.id)
    }
}
