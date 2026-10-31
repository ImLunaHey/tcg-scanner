import Foundation
import ImageCaptureCore
import ScannerAPI

@MainActor
class ImageCaptureConnection: NSObject, @preconcurrency ICScannerDeviceDelegate {
    let scanner: ICScannerDevice
    let log: @Sendable (String) -> Void
    var failure: ScannerError?
    var sessionOpened = false
    var sessionClosed = false
    private var openRequested = false
    private var openCompleted = false
    private var ready = false
    private var selectionCompleted = false
    private var closing = false
    private var closeCompleted = false
    private var originalUnit: ICScannerFunctionalUnitType?

    init(scanner: ICScannerDevice, log: @escaping @Sendable (String) -> Void) { self.scanner = scanner; self.log = log }
    func connect() async throws {
        scanner.delegate = self
        openRequested = true
        requestOpen()
        try await wait("open session") { openCompleted }
        try await wait("device readiness") { ready }
        originalUnit = scanner.selectedFunctionalUnit.type
    }
    func select(_ type: ICScannerFunctionalUnitType) async throws {
        if scanner.selectedFunctionalUnit.type == type { return }
        selectionCompleted = false; scanner.requestSelect(type)
        try await wait("select functional unit") { selectionCompleted }
        guard scanner.selectedFunctionalUnit.type == type else { throw ScannerError("driver", "Unexpected functional unit selected.") }
    }
    func close() async -> [ScannerError] {
        await Task { @MainActor in
            var errors: [ScannerError] = []
            if let originalUnit, sessionOpened, !closeCompleted, failure == nil {
                do { try await select(originalUnit) } catch { errors.append(Self.convert(error)) }
            }
            if !closeCompleted && openRequested && (sessionOpened || !openCompleted) {
                closing = true; requestClose()
                let deadline = ContinuousClock.now + .seconds(10)
                while !closeCompleted && ContinuousClock.now < deadline { try? await Task.sleep(for: .milliseconds(50)) }
                if !closeCompleted { errors.append(ScannerError("cleanup", "Scanner session closure was not confirmed.")) }
            }
            if let failure { errors.append(failure) }
            scanner.delegate = nil
            return errors
        }.value
    }
    func wait(_ operation: String, seconds: Int = 15, until done: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !done() {
            if let failure { throw failure }
            guard ContinuousClock.now < deadline else { throw ScannerError("timeout", "Timed out waiting for \(operation).") }
            try await Task.sleep(for: .milliseconds(50))
        }
        try Task.checkCancellation()
        if let failure { throw failure }
    }
    static func convert(_ error: Error) -> ScannerError {
        if let e = error as? ScannerError { return e }
        if error is CancellationError { return ScannerError("cancelled", "Operation cancelled.") }
        return ScannerError("driver", error: error)
    }
    private func requestOpen() { scanner.requestOpenSession() }
    private func requestClose() { scanner.requestCloseSession() }
    func device(_ device: ICDevice, didOpenSessionWithError error: Error?) {
        openCompleted = true
        if let error { failure = ScannerError("openSession", error: error) }
        else { sessionOpened = true; log("session opened") }
    }
    func deviceDidBecomeReady(_ device: ICDevice) { ready = true; log("device ready") }
    func device(_ device: ICDevice, didCloseSessionWithError error: Error?) {
        closeCompleted = true
        if let error { failure = ScannerError("closeSession", error: error) }
        else { sessionClosed = true; log("session closed") }
        if !closing { failure = ScannerError("disconnected", "Scanner session closed unexpectedly.") }
    }
    func didRemove(_ device: ICDevice) { failure = ScannerError("disconnected", "Scanner disconnected.") }
    func device(_ device: ICDevice, didEncounterError error: Error?) {
        if let error { failure = ScannerError("driver", error: error) }
    }
    func scannerDevice(_ scanner: ICScannerDevice, didSelect functionalUnit: ICScannerFunctionalUnit, error: Error?) {
        if let error { failure = ScannerError("selectUnit", error: error) }
        selectionCompleted = true
    }
}
