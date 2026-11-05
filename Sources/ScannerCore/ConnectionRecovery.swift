import Foundation
import ScannerAPI

extension ScannerCoordinator {
    /// Device monitoring runs even with the physical-button listener disabled.
    /// Reserve inspection across the backend await to prevent competing clients.
    func recoverScannerConnection() async {
        guard !stopping, !paused, !inspecting, activeScanID == nil,
              automaticButtonTask == nil, buttonMonitorTask == nil else { return }
        inspecting = true
        defer { inspecting = false; scheduleNext() }
        guard await backend.recoverAfterDisconnect() else { return }
        if requiresRestart {
            await failQueuedScansAfterDisconnect()
            requiresRestart = false
            try? database.event("scanner.connection.recovered", payload: .null)
        }
    }

    /// A reconnect must not feed cards for jobs submitted before ownership was lost.
    func failQueuedScansAfterDisconnect() async {
        let pending = queue
        queue.removeAll()
        for id in pending {
            guard let store = scans[id] else { continue }
            do {
                try await store.finish(errors: [ScannerError("disconnected",
                    "Scanner connection was interrupted. Start a new job after reconnecting.")],
                    cancelled: false, sessionClosed: true)
            } catch {
                await store.recordPersistenceFailure(error)
                persistenceError = ScannerError("storage", error: error)
            }
            scans[id] = nil
        }
    }
}
