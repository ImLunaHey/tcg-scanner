import ArgumentParser
import Foundation
import Hummingbird
import HummingbirdCore
import ScannerHTTPSupport
import ScannerTransport

@main
struct HTTPMain: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scanner-http", abstract: "Authenticated Swift HTTP gateway to ScannerService.", version: "0.5.0")
    @Option var socket = SocketLocation.defaultPath
    @Option var host = "127.0.0.1"
    @Option var port = 8170
    @Option var tokenFile = GatewayCredential.defaultPath
    @Option(help: "Absolute path to built web assets; exposes only the login shell and static assets without authentication.") var webRoot: String?
    @Option(help: "Exact HTTPS origin when using Tailscale Serve. Never trust forwarded headers.") var publicOrigin: String?
    @Flag(help: "Permit binding to a specific Tailscale IPv4 address; authentication remains required.") var allowRemote = false
    @Option(help: "Write a LaunchAgent plist without starting the gateway.") var launchAgentPlist: String?
    @Flag(help: "Create a private credential file and exit; never prints the secret.") var initToken = false
    func run() async throws {
        if initToken { try GatewayCredential.create(at: tokenFile); return }
        let config = try GatewayConfiguration(host: host, port: port, allowRemote: allowRemote, publicOrigin: publicOrigin)
        if let launchAgentPlist {
            _ = try GatewayCredential.load(at: tokenFile)
            try writeLaunchAgent(to: launchAgentPlist); return
        }
        let assets = try webRoot.map { try WebAssets(directory: $0) }
        let responder = try ScannerGateway(socketPath: socket, token: GatewayCredential.load(at: tokenFile), configuration: config, webAssets: assets)
        let app = Application(responder: responder, server: .http1(configuration: .init(idleTimeout: .seconds(30),
            httpDecoderConfiguration: .init(maxHeaderFieldSize: 8192, maxHeaderListSize: 16384, maxHeaderFieldCount: 32))),
            configuration: .init(address: .hostname(host, port: port), serverName: "ScannerHTTP", backlog: 64,
                                 availableConnectionsDelegate: MaximumAvailableConnections(32)))
        try await app.runService()
    }
}
