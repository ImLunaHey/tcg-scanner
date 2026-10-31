import Darwin
import Foundation
import ScannerTransport
import ScannerCore

@MainActor
final class ServiceLifetime {
    private let coordinator: ScannerCoordinator
    private let server: UnixServer
    private let log: @Sendable (String) -> Void
    private var signals: [DispatchSourceSignal] = []
    private var shuttingDown = false
    var finished = false
    init(server: UnixServer, coordinator: ScannerCoordinator, log: @escaping @Sendable (String) -> Void) { self.server = server; self.coordinator = coordinator; self.log = log }

    func installSignalHandlers() {
        for number in [SIGINT, SIGTERM] {
            signal(number, SIG_IGN)
            let source = DispatchSource.makeSignalSource(signal: number, queue: .main)
            source.setEventHandler { [weak self] in MainActor.assumeIsolated { self?.shutdown() } }
            source.resume()
            signals.append(source)
        }
    }
    private func shutdown() {
        guard !shuttingDown else { return }
        shuttingDown = true
        server.stopAccepting(releaseLockWhenDrained: false)
        log("Shutting down; cancelling any active scan and closing sessions.")
        Task { @MainActor in
            await coordinator.shutdownScans()
            let deadline = ContinuousClock.now + .seconds(120)
            while server.activeConnections > 0 && ContinuousClock.now < deadline {
                try? await Task.sleep(for: .milliseconds(100))
            }
            if server.activeConnections > 0 { log("Shutdown deadline reached; outstanding clients will disconnect.") }
            server.releaseShutdownLock()
            finished = true
        }
    }
}
