import Darwin
import Foundation
import ScannerTransport

@MainActor
final class ServiceLifetime {
    private let server: UnixServer
    private let log: @Sendable (String) -> Void
    private var signals: [DispatchSourceSignal] = []
    private var shuttingDown = false
    var finished = false
    init(server: UnixServer, log: @escaping @Sendable (String) -> Void) { self.server = server; self.log = log }

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
        server.stopAccepting()
        log("Shutting down; allowing active diagnostic sessions to close.")
        Task { @MainActor in
            let deadline = ContinuousClock.now + .seconds(120)
            while server.activeConnections > 0 && ContinuousClock.now < deadline {
                try? await Task.sleep(for: .milliseconds(100))
            }
            if server.activeConnections > 0 { log("Shutdown deadline reached; outstanding clients will disconnect.") }
            finished = true
        }
    }
}
