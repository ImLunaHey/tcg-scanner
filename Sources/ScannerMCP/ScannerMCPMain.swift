import ArgumentParser
import Foundation
import ScannerMCPSupport
import ScannerTransport

@main
struct ScannerMCPMain: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scanner-mcp", abstract: "Swift MCP server over stdio; ScannerService must already be running.", version: "0.4.0")
    @Option(help: "Private ScannerService Unix socket.") var socket = SocketLocation.defaultPath
    @Flag(help: "Advertise and allow only observation/retrieval tools.") var readOnly = false
    func run() async throws {
        let server = ScannerMCPServer(socketPath: socket, readOnly: readOnly)
        // stdout is reserved for MCP frames. ArgumentParser errors/logs use stderr.
        try await server.runStdio()
    }
}
