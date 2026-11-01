import Foundation
import Hummingbird
import HummingbirdTesting
import HTTPTypes
import Testing
import ScannerAPI
@testable import ScannerHTTPSupport

@Test func publicShellNeverExposesScannerOrOtherFiles() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    try Data("<html>Sign in</html>".utf8).write(to: root.appendingPathComponent("index.html"))
    try Data("console.log('fixture')".utf8).write(to: root.appendingPathComponent("assets/main.js"))
    var config = try GatewayConfiguration(); config.authorities.insert("localhost")
    let gateway = try ScannerGateway(socketPath: "/tmp/no-scanner-web-test", token: httpTestToken, configuration: config, webAssets: WebAssets(directory: root.path))
    let app = Application(responder: gateway)
    try await app.test(.router) { client in
        for route in ["/", "/index.html", "/assets/main.js", "/library", "/profiles", "/scanner",
                      "/batches/5F2213C2-7E6E-41E1-9A9B-C756613A1A15"] {
            let response = try await client.execute(uri: route, method: .get)
            #expect(response.status.code == 200)
            #expect(response.headers[HTTPField.Name("Content-Security-Policy")!]?.contains("script-src 'self'") == true)
            let policy = response.headers[HTTPField.Name("Content-Security-Policy")!] ?? ""
            #expect(policy.split(separator: ";").map { $0.trimmingCharacters(in: .whitespaces) }.contains("script-src 'self'"))
            #expect(!policy.contains("unsafe-eval"))
            #expect(response.headers[.cacheControl] == "no-store")
        }
        #expect(try await client.execute(uri: "/", method: .post).status.code == 405)
        let batch = "/batches/5F2213C2-7E6E-41E1-9A9B-C756613A1A15"
        let deepLink = try await client.execute(uri: batch, method: .get)
        #expect(String(buffer: deepLink.body) == "<html>Sign in</html>")
        #expect(try await client.execute(uri: batch, method: .head).body.readableBytes == 0)
        #expect(try await client.execute(uri: batch, method: .post).status.code == 405)
        #expect(try await client.execute(uri: batch + "?token=secret", method: .get).status.code == 400)
        #expect(try await client.execute(uri: "/", method: .get, headers: [.contentLength: "2"], body: ByteBuffer(string: "{}" )).status.code == 400)
        #expect(try await client.execute(uri: "/?token=secret", method: .get).status.code == 400)
        for route in ["/api/v1/jobs", "/api/v1/devices", "/token", "/manifest.json", "/assets/missing.js", "/batches/invalid", batch + "/originals", "/unknown"] {
            #expect(try await client.execute(uri: route, method: .get).status.code == 401)
        }
        #expect(try await client.execute(uri: "/assets/%2e%2e/token", method: .get).status.code == 400)
        var badOrigin = httpHeaders(); badOrigin[.origin] = "https://evil.example"
        #expect(try await client.execute(uri: "/", method: .get, headers: badOrigin).status.code == 403)
    }
    try FileManager.default.createSymbolicLink(at: root.appendingPathComponent("assets/leak.js"), withDestinationURL: root.appendingPathComponent("index.html"))
    #expect(throws: ScannerError.self) { try WebAssets(directory: root.path) }
}
