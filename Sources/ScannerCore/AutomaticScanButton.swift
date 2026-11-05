import Foundation
import ScannerAPI

extension ScannerCoordinator {
    /// Called by the service's device monitor. Never opens a competing session.
    public func maintainScanButton() {
        guard let deviceID = scanButtonDeviceID, automaticButtonTask == nil,
              buttonMonitorTask == nil, !inspecting, activeScanID == nil, queue.isEmpty,
              !stopping, !paused, !requiresRestart, persistenceError == nil,
              ContinuousClock.now >= nextButtonAttempt else { return }
        automaticButtonTask = Task {
            defer {
                automaticButtonTask = nil
                // Debounce repeated presses and back off unavailable/offline drivers.
                nextButtonAttempt = ContinuousClock.now + .seconds(5)
            }
            do {
                _ = try await selectDevice(deviceID)
                try Task.checkCancellation()
                try await backend.waitForScanButton(deviceID: deviceID)
                try Task.checkCancellation()
                let request = ScanRequest(deviceID: deviceID, dpi: 600, duplex: true, format: .png, prepareCards: true)
                let record = try await submitScan(request, fromButton: true)
                try database.event("scanner.button.jobCreated", payload: .object([
                    "jobID": .string(record.id.uuidString), "deviceID": .string(deviceID)
                ]))
            } catch is CancellationError { /* A client, sleep or shutdown took ownership. */ }
            catch {
                let failure = (error as? ScannerError) ?? ScannerError("scanButton", error: error)
                if failure.kind == "cleanup" || failure.kind == "closeSession" { requiresRestart = true }
                try? database.event("scanner.button.failed", payload: .wrapping(failure))
            }
        }
    }

    /// Caller reserves inspecting/activeScanID before awaiting this handoff.
    /// The monitor owns its task slot until it has finished closing the session.
    func releaseAutomaticButton() async throws {
        let task = automaticButtonTask
        task?.cancel()
        await task?.value
        guard !requiresRestart else { throw ScannerError("cleanup", "Button listener could not confirm session closure; disconnect and reconnect the scanner.") }
        try Task.checkCancellation()
    }
}
