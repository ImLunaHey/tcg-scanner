import ArgumentParser
import Darwin
import Foundation
import ScannerAPI
import ScannerCore
import ScannerServiceSupport
import ScannerTransport

struct ServiceOptions: ParsableArguments {
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
        let router = DiagnosticRouter(coordinator: ScannerCoordinator(backend: backend))
        let server = UnixServer(path: options.socket, log: log, handler: router.respond)
        let lifetime = ServiceLifetime(server: server, log: log)
        do {
            try server.start()
            lifetime.installSignalHandlers()
            log("Listening on \(options.socket); diagnostics only, scanning is not implemented.")
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
