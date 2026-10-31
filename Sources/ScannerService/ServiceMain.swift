import ArgumentParser
import Darwin
import Foundation
import ScannerAPI
import ScannerCore
import ScannerPersistence
import OSLog
import ScannerServiceSupport
import ScannerTransport

struct ServiceOptions: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scanner-service", abstract: "Local scanner owner and API service.")
    @Option(help: "Storage root; clients may only write scan batches inside this directory.")
    var storageRoot: String?
    @Option(help: "Require this SMB share at the storage root; never fall back to local storage.")
    var requiredSMBShare: String?
    @Option(help: "Private SQLite database path.")
    var database = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Scanner/state/scanner.sqlite").path
    @Option(help: "Write a LaunchAgent plist to this path and exit without accessing hardware.")
    var launchAgentPlist: String?
    @Option(help: "Absolute Unix socket path (parent directory must be private).")
    var socket = SocketLocation.defaultPath
    @Flag(name: .shortAndLong, help: "Log discovery and scanner session callbacks.")
    var verbose = false
    func storageConfiguration() throws -> StorageConfiguration {
        var result = try StorageConfiguration.load()
        if let storageRoot { result.storageRoot = storageRoot; result.requiredSMBShare = requiredSMBShare }
        else if let requiredSMBShare { result.requiredSMBShare = requiredSMBShare }
        return result
    }
}

@main
enum ServiceMain {
    @MainActor static func main() {
        let options = ServiceOptions.parseOrExit()
        umask(0o077)
        let logger = Logger(subsystem: "local.scanner.service", category: "service")
        let log: @Sendable (String) -> Void = { message in
            logger.info("\(message, privacy: .public)")
            let entry: [String: String] = ["timestamp": ISO8601DateFormatter().string(from: Date()), "component": "scanner-service", "message": message]
            if let data = try? JSONSerialization.data(withJSONObject: entry, options: [.sortedKeys]) {
                FileHandle.standardError.write(data + Data([10]))
            }
        }
        do {
            let storage = try options.storageConfiguration()
            guard options.database.hasPrefix("/") else {
                throw ScannerError("invalidSettings", "Service storage and database paths must be absolute.")
            }
            if let path = options.launchAgentPlist {
                try LaunchAgent.write(options: options, to: path)
                return
            }
            try storage.validate()
            let owner = try HardwareOwnerLock()
            let database = try ScannerDatabase(path: options.database)
            let recovered = try database.recoverInterruptedJobs(storageRoot: URL(fileURLWithPath: storage.storageRoot))
            try database.event("service.started", payload: .object(["recoveredJobs": .number(Double(recovered))]))
            log("Recovered \(recovered) interrupted job(s); physical scans are never automatically resumed.")
            let verbose = options.verbose
            let backend = ImageCaptureBackend { message in if verbose { log(message) } }
            let coordinator = try ScannerCoordinator(backend: backend, storageRoot: URL(fileURLWithPath: storage.storageRoot), database: database, validateStorage: storage.validate)
            let router = DiagnosticRouter(coordinator: coordinator)
            let server = UnixServer(path: options.socket, log: log, handler: router.respond)
            let lifetime = ServiceLifetime(server: server, coordinator: coordinator, log: log)
            try server.start()
            lifetime.start()
            log("Listening on \(options.socket); scan storage: \(storage.storageRoot); database: \(options.database).")
            // ICC delegate callbacks require the main run loop, even with no GUI running.
            withExtendedLifetime(owner) {
                while !lifetime.finished { RunLoop.main.run(until: Date().addingTimeInterval(0.1)) }
            }
            try database.event("service.stopped", payload: .null)
        } catch {
            log("Startup or shutdown failed: \(error.localizedDescription)")
            exit(1)
        }
    }
}
