import Foundation
import ImageCaptureCore
import ScannerAPI

public struct CaptureOutcome: Sendable {
    public var errors: [ScannerError]
    public var cancelled: Bool
    public var sessionClosed: Bool
    public init(errors: [ScannerError] = [], cancelled: Bool = false, sessionClosed: Bool = true) {
        self.errors = errors; self.cancelled = cancelled; self.sessionClosed = sessionClosed
    }
}

@MainActor
final class ImageCaptureScanSession: ImageCaptureConnection {
    private let store: ScanStorage
    private var completed = false
    private var requested = false
    private var completionError: ScannerError?
    private var sequence = 0
    private var seenURLs: Set<URL> = []
    private var saves: [Task<Void, Error>] = []

    init(scanner: ICScannerDevice, store: ScanStorage, log: @escaping @Sendable (String) -> Void) {
        self.store = store; super.init(scanner: scanner, log: log)
    }
    func capture(_ request: ScanRequest) async -> CaptureOutcome {
        var outcome = CaptureOutcome(sessionClosed: false)
        let configuration = ScanConfiguration()
        do {
            try await connect()
            guard scanner.availableFunctionalUnitTypes.contains(NSNumber(value: 3)) else {
                throw ScannerError("unsupportedSetting", "No ADF functional unit available.")
            }
            try await select(.documentFeeder)
            let settings = try configuration.apply(to: scanner, request: request, incoming: store.incomingDirectory)
            try await store.markScanning(settings: settings)
            try Task.checkCancellation()
            requested = true; requestScan()
            log("scan requested: \(request.duplex ? "duplex" : "simplex"), \(request.dpi) DPI, A4")
            try await wait("scan completion", seconds: 120) { completed }
            if let completionError { throw completionError }
        } catch {
            if error is CancellationError { outcome.cancelled = true }
            else { outcome.errors.append(Self.convert(error)) }
        }
        await Task { @MainActor in
            if requested && !completed {
                scanner.cancelScan()
                let deadline = ContinuousClock.now + .seconds(15)
                while !completed && ContinuousClock.now < deadline { try? await Task.sleep(for: .milliseconds(50)) }
                if !completed { outcome.errors.append(ScannerError("cleanup", "Scan cancellation was not acknowledged; restart service before another scan.")) }
            }
            for save in saves {
                do { try await save.value } catch { outcome.errors.append(Self.convert(error)) }
            }
            // Restore settings only after scanning has stopped. If not, quarantine the backend.
            if !requested || completed { configuration.restore() }
            for error in await close() where !outcome.errors.contains(error) { outcome.errors.append(error) }
            outcome.sessionClosed = sessionClosed
        }.value
        return outcome
    }
    private func requestScan() { scanner.requestScan() }
    func scannerDevice(_ scanner: ICScannerDevice, didScanTo url: URL) {
        sequence += 1
        let number = sequence
        guard seenURLs.insert(url).inserted else {
            failure = ScannerError("driver", "Driver reused an output URL; stopping to avoid ambiguous originals.")
            scanner.cancelScan(); return
        }
        let store = self.store
        saves.append(Task {
            do { try await store.preserve(url, sequence: number) }
            catch { failure = Self.convert(error); scanner.cancelScan(); throw error }
        })
        log("received image \(number): \(url.lastPathComponent)")
        if number >= 100 {
            failure = ScannerError("resourceLimit", "Basic scan limit of 100 image callbacks reached; cancelling. Files retained.")
            scanner.cancelScan()
        }
    }
    func scannerDevice(_ scanner: ICScannerDevice, didCompleteScanWithError error: Error?) {
        completed = true
        if let error { completionError = ScannerError("scan", error: error) }
        log("scan completion callback\(error.map { ": \($0.localizedDescription)" } ?? "")")
    }
}
