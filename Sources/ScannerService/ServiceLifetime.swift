import AppKit
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
    private var monitor: Task<Void, Never>?
    private var catalogueMonitor: Task<Void, Never>?
    private var sleepTask: Task<Void, Never>?
    private var observers: [NSObjectProtocol] = []
    private var shuttingDown = false
    var finished = false
    init(server: UnixServer, coordinator: ScannerCoordinator, log: @escaping @Sendable (String) -> Void) { self.server = server; self.coordinator = coordinator; self.log = log }

    func start() {
        installSignalHandlers()
        let center = NSWorkspace.shared.notificationCenter
        observers.append(center.addObserver(forName: NSWorkspace.willSleepNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.log("System sleep: pausing scheduling and cancelling outstanding jobs.")
                self.sleepTask = Task { await self.coordinator.prepareForSleep() }
            }
        })
        observers.append(center.addObserver(forName: NSWorkspace.didWakeNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                Task { @MainActor in
                    await self.sleepTask?.value
                    do { try await self.coordinator.resumeAfterWake() }
                    catch { self.log("Wake handling failed: \(error.localizedDescription)") }
                }
            }
        })
        catalogueMonitor = Task { [coordinator, log] in
            while !Task.isCancelled {
                do { try await coordinator.retryAwaitingCatalogue() }
                catch { if !Task.isCancelled { log("Catalogue retry: \(error.localizedDescription)") } }
                do { try await Task.sleep(for: .seconds(30)) } catch { break }
            }
        }
        monitor = Task { [coordinator, log] in
            var lastError: String?
            while !Task.isCancelled {
                do {
                    _ = try await coordinator.devices()
                    await coordinator.maintainScanButton()
                    lastError = nil
                }
                catch {
                    let message = error.localizedDescription
                    if message != lastError && !Task.isCancelled { log("Device monitor: \(message)"); lastError = message }
                }
                do { try await Task.sleep(for: .seconds(2)) } catch { break }
            }
        }
    }
    private func installSignalHandlers() {
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
        monitor?.cancel()
        catalogueMonitor?.cancel()
        for observer in observers { NSWorkspace.shared.notificationCenter.removeObserver(observer) }
        observers.removeAll()
        server.stopAccepting(releaseLockWhenDrained: false)
        log("Shutting down; cancelling any active scan and closing sessions.")
        Task { @MainActor in
            let deadline = ContinuousClock.now + .seconds(55)
            await coordinator.shutdownScans()
            await monitor?.value
            await catalogueMonitor?.value
            await sleepTask?.value
            while server.activeConnections > 0 && ContinuousClock.now < deadline {
                try? await Task.sleep(for: .milliseconds(100))
            }
            if server.activeConnections > 0 { log("Shutdown deadline reached; outstanding clients will disconnect.") }
            server.releaseShutdownLock()
            finished = true
        }
    }
}
