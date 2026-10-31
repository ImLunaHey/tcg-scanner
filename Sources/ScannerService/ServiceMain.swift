import ArgumentParser
import Darwin
import Foundation
import ScannerAPI
import ScannerCore
import ScannerServiceSupport
import ScannerTransport

struct ServiceOptions: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scanner-service", abstract: "Local scanner owner and API service.")
    @Option(help: "Storage root; clients may only write scan batches inside this directory.")
    var storageRoot = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Scans").path
    @Option(help: "Absolute Unix socket path (parent directory must be private).")
    var socket = SocketLocation.defaultPath
    @Flag(name: .shortAndLong, help: "Log discovery and scanner session callbacks.")
    var verbose = false
}

@main
enum ServiceMain {
    @MainActor static func main() {
        let options = ServiceOptions.parseOrExit()
        let log: @Sendable (String) -> Void = { message in
            FileHandle.standardError.write(Data("[scanner-service] \(message)\n".utf8))
        }
        let verbose = options.verbose
        let backend = ImageCaptureBackend { message in
            if verbose { log(message) }
        }
        let coordinator = ScannerCoordinator(backend: backend, storageRoot: URL(fileURLWithPath: options.storageRoot))
        let router = DiagnosticRouter(coordinator: coordinator)
        let server = UnixServer(path: options.socket, log: log, handler: router.respond)
        let lifetime = ServiceLifetime(server: server, coordinator: coordinator, log: log)
        do {
            try server.start()
            lifetime.installSignalHandlers()
            log("Listening on \(options.socket); scan storage: \(options.storageRoot).")
            // ICC delegate callbacks require the main run loop, including when no GUI is running.
            while !lifetime.finished {
                RunLoop.main.run(until: Date().addingTimeInterval(0.1))
            }
        } catch {
            log("Startup failed: \(error.localizedDescription)")
            exit(1)
        }
    }
}
