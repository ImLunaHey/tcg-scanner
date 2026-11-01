import Foundation
import ScannerAPI

extension ScannerCoordinator {
    /// Explicit, bounded diagnostics. Never used by discovery or ordinary scanning.
    public func startButtonObservation(seconds: Int) async throws {
        guard (30...300).contains(seconds) else { throw ScannerError("invalidParams", "Observation must last 30–300 seconds.") }
        guard !inspecting, buttonMonitorTask == nil, activeScanID == nil, queue.isEmpty,
              !stopping, !paused, !requiresRestart else { throw ScannerError("busy", "Service must be idle for button observation.") }
        inspecting = true
        buttonMonitorTask = Task {
            do {
                try await releaseAutomaticButton()
                let device = try await selectDevice(nil)
                try Task.checkCancellation()
                try await backend.observeButton(deviceID: device.id, seconds: seconds)
            }
            catch is CancellationError { /* Normal explicit stop or service shutdown. */ }
            catch {
                let failure = (error as? ScannerError) ?? ScannerError("buttonObservation", error: error)
                if failure.kind == "cleanup" || failure.kind == "closeSession" { requiresRestart = true }
                try? database.event("diagnostics.buttonMonitor.failed", payload: .wrapping(failure))
            }
            buttonMonitorTask = nil
            inspecting = false
            scheduleNext()
        }
    }
    public func stopButtonObservation() async {
        let task = buttonMonitorTask
        task?.cancel()
        await task?.value
    }
}
