import Foundation
import Hummingbird
import HummingbirdTesting
import HTTPTypes
import Testing
import ScannerAPI
@testable import ScannerHTTPSupport

@Test func authenticationOriginInputAndUnavailableBoundaries() async throws {
    var configuration = try GatewayConfiguration(); configuration.authorities.insert("localhost") // Router test harness authority.
    let gateway = try ScannerGateway(socketPath: "/tmp/missing-http-test-\(UUID())", token: httpTestToken, configuration: configuration)
    let app = Application(responder: gateway)
    try await app.test(.router) { client in
        #expect(try await client.execute(uri: "/api/v1/status", method: .get).status.code == 401)
        var evilOrigin = httpHeaders(); evilOrigin[.origin] = "https://evil.example"
        #expect(try await client.execute(uri: "/api/v1/status", method: .get, headers: evilOrigin).status.code == 403)
        var cookies = httpHeaders(); cookies[.cookie] = "session=x"
        #expect(try await client.execute(uri: "/api/v1/jobs", method: .post, headers: cookies).status.code == 400)
        for route in ["/api/v1/jobs?limit=101", "/api/v1/jobs?limit=1&limit=2", "/api/v1/devices?token=x", "/api/v1/images/..%2Fsecret", "/api/v1/images/not-a-job:1", "/api/v1/jobs/invalid", "/api/v1/events?seconds=100"] {
            #expect(try await client.execute(uri: route, method: .get, headers: httpHeaders()).status.code == 400)
        }
        for params: JSONValue in [.object(["shell": .string("id")]), .object(["outputDirectory": .string("/tmp")]), .object([:])] {
            #expect(try await client.execute(uri: "/api/v1/jobs", method: .post, headers: httpHeaders(), body: buffer(params)).status.code == 400)
        }
        var wrongType = httpHeaders(); wrongType[.contentType] = "text/plain"
        #expect(try await client.execute(uri: "/api/v1/jobs", method: .post, headers: wrongType, body: ByteBuffer(string: "{}" )).status.code == 415)
        #expect(try await client.execute(uri: "/api/v1/jobs", method: .post, headers: httpHeaders(), body: ByteBuffer(string: String(repeating: "a", count: 62 * 1024))).status.code == 413)
        let offline = try await client.execute(uri: "/api/v1/status", method: .get, headers: httpHeaders())
        #expect(offline.status.code == 503 && offline.headers[.cacheControl] == "no-store")
        let reset = try await client.execute(uri: "/api/v1/session", method: .delete, headers: httpHeaders())
        #expect(reset.status.code == 400)
    }
}

@Test func credentialsSessionsAndLimits() async throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("http-token-\(UUID())")
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("token").path
    try GatewayCredential.create(at: file)
    let token = try GatewayCredential.load(at: file)
    #expect(token.count == 64)
    #expect(throws: ScannerError.self) { try GatewayCredential.create(at: file) }
    try FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: file)
    #expect(throws: ScannerError.self) { try GatewayCredential.load(at: file) }
    let access = try GatewayAccess(token: token)
    #expect(try await access.authenticate("Bearer " + token) == "root")
    let (session, _) = try await access.issue(for: "root")
    let principal = try await access.authenticate("Bearer " + session)
    await #expect(throws: GatewayError.self) { try await access.issue(for: principal) }
    try await access.revoke(principal)
    await #expect(throws: GatewayError.self) { try await access.authenticate("Bearer " + session) }
    let expired = try GatewayAccess(token: token, sessionLifetime: -1)
    let (oldToken, _) = try await expired.issue(for: "root")
    await #expect(throws: GatewayError.self) { try await expired.authenticate("Bearer " + oldToken) }
    for _ in 0..<4 { try await access.acquire("events") }
    await #expect(throws: GatewayError.self) { try await access.acquire("events") }
    await access.release("events"); try await access.acquire("events")
    for _ in 0..<2 { try await access.acquire("image") }
    await #expect(throws: GatewayError.self) { try await access.acquire("image") }
    for _ in 0..<30 { _ = try? await access.authenticate(nil) }
    do { _ = try await access.authenticate(nil); Issue.record("Expected rate limit") }
    catch let error as GatewayError { #expect(error.status == 429) }
}

@Test func remoteBindingMustBeExplicitAndPrivate() throws {
    #expect(try GatewayConfiguration().host == "127.0.0.1")
    #expect(throws: ScannerError.self) { try GatewayConfiguration(host: "0.0.0.0", allowRemote: true) }
    #expect(throws: ScannerError.self) { try GatewayConfiguration(host: "192.168.1.10", allowRemote: true) }
    #expect(throws: ScannerError.self) { try GatewayConfiguration(host: "100.100.1.2") }
    #expect(try GatewayConfiguration(host: "100.100.1.2", allowRemote: true).origin == "http://100.100.1.2:8170")
    for origin in ["http://evil.example", "https://user:secret@example.com", "https://example.com/path", "https://example.com?token=x"] {
        #expect(throws: ScannerError.self) { try GatewayConfiguration(publicOrigin: origin) }
    }
    #expect(try GatewayConfiguration(publicOrigin: "https://scanner.example.ts.net").authorities.contains("scanner.example.ts.net"))
}

@Test func admissionIsReleasedOnceEvenWhenResponseIsDropped() async throws {
    let access = try GatewayAccess(token: httpTestToken)
    try await access.acquire("image")
    var lease: RequestLease? = RequestLease(access: access, kind: "image")
    await lease?.finish(); await lease?.finish(); lease = nil
    try await access.acquire("image"); try await access.acquire("image")
    await #expect(throws: GatewayError.self) { try await access.acquire("image") }
    await access.release("image"); await access.release("image")
    // Simulates a responder body dropped before the framework invokes its write closure.
    try await access.acquire("events")
    var dropped: RequestLease? = RequestLease(access: access, kind: "events")
    #expect(dropped != nil); dropped = nil
    // Deinit enqueues actor cleanup; allow it to run without blocking the executor.
    try await Task.sleep(for: .milliseconds(20))
    for _ in 0..<4 { try await access.acquire("events") }
    await #expect(throws: GatewayError.self) { try await access.acquire("events") }
}

@Test func imageDownloadsDoNotBlockIdentityOrMetadataRequests() async throws {
    var configuration = try GatewayConfiguration(); configuration.authorities.insert("localhost")
    let gateway = try ScannerGateway(socketPath: "/tmp/missing-http-test-\(UUID())", token: httpTestToken, configuration: configuration)
    try await gateway.access.acquire("image"); try await gateway.access.acquire("image")
    let app = Application(responder: gateway)
    try await app.test(.router) { client async throws in
        let path = "/api/v1/images/\(UUID()):1"
        for suffix in ["", "/thumbnail"] {
            #expect(try await client.execute(uri: path + suffix, method: .get, headers: httpHeaders()).status.code == 429)
        }
        // Reaching the deliberately unavailable service proves these requests
        // passed admission despite both binary download slots being occupied.
        for suffix in ["/metadata", "/identification"] {
            #expect(try await client.execute(uri: path + suffix, method: .get, headers: httpHeaders()).status.code == 503)
        }
        #expect(try await client.execute(uri: path + "/identification", method: .post, headers: httpHeaders(), body: buffer(.object(["id": .string(UUID().uuidString)]))).status.code == 503)
    }
    await gateway.access.release("image"); await gateway.access.release("image")
}
