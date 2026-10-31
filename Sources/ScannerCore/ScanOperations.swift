import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func startScan(_ request: ScanRequest) async throws -> ScanRecord {
        try request.validate()
        if let existing = scans[request.requestID] {
            guard requests[request.requestID] == request else { throw ScannerError("conflict", "Request ID was already used with different settings.") }
            return await existing.snapshot()
        }
        guard !inspecting && !stopping else { throw ScannerError("busy", "Scanner is busy or shutting down.") }
        guard scans.count < 100 else { throw ScannerError("resourceLimit", "This milestone retains at most 100 scan records per service run; restart before continuing.") }
        inspecting = true
        do {
            let device = try await selectDevice(request.deviceID)
            guard !stopping else { throw ScannerError("cancelled", "Service is shutting down.") }
            let store = try ScanStorage(root: storageRoot, device: device, request: request)
            scans[request.requestID] = store; requests[request.requestID] = request
            activeScanID = request.requestID
            scanTask = Task {
                let outcome: CaptureOutcome
                do { outcome = try await backend.capture(deviceID: device.id, request: request, store: store) }
                catch { outcome = CaptureOutcome(errors: [ScannerError("capture", error: error)], cancelled: error is CancellationError, sessionClosed: false) }
                do { try await store.finish(errors: outcome.errors, cancelled: outcome.cancelled, sessionClosed: outcome.sessionClosed) }
                catch { await store.recordPersistenceFailure(error) }
                self.inspecting = false; self.activeScanID = nil; self.scanTask = nil
            }
            return await store.snapshot()
        } catch { inspecting = false; throw error }
    }
    public func scanStatus(_ id: UUID) async throws -> ScanRecord {
        guard let store = scans[id] else { throw ScannerError("scanNotFound", "Scan ID is unknown in this service run. See its on-disk manifest after a restart.") }
        return await store.snapshot()
    }
    public func cancelScan(_ id: UUID) async throws -> ScanRecord {
        guard let store = scans[id] else { throw ScannerError("scanNotFound", "Unknown scan ID.") }
        if activeScanID == id { scanTask?.cancel() }
        return await store.snapshot()
    }
    public func shutdownScans() async {
        stopping = true
        scanTask?.cancel()
        await scanTask?.value
    }
}
