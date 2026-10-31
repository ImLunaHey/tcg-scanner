import CoreGraphics
import CryptoKit
import Foundation
import Hummingbird
import HummingbirdTesting
import HTTPTypes
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerHTTPSupport

let httpTestToken = String(repeating: "a", count: 64)
func httpHeaders(_ token: String = httpTestToken) -> HTTPFields {
    [.authorization: "Bearer " + token, .contentType: "application/json", HTTPField.Name("Host")!: "127.0.0.1:8170"]
}
func buffer(_ value: JSONValue) throws -> ByteBuffer { ByteBuffer(bytes: try Wire.encoder().encode(value)) }
func decode<T: Decodable>(_ result: TestResponse, as: T.Type) throws -> T { try Wire.decoder().decode(T.self, from: Data(result.body.readableBytesView)) }

actor HTTPMock: ScannerBackend {
    var captures = 0
    var hold = false
    func holdCaptures() { hold = true }
    func devices() -> [Device] { [Device(id: "http-mock", name: "HTTP mock")] }
    func inspect(deviceID: String) -> Inspection {
        var value = Inspection(device: Device(id: deviceID, name: "HTTP mock"))
        value.sessionOpened = true; value.sessionClosed = true
        var unit = UnitSnapshot(); unit.type = 3
        unit.settings["resolutionDPI"] = Setting(current: .number(300), allowed: .array([.number(300), .number(600)]), readOnly: false)
        unit.settings["supportsDuplexScanning"] = Setting(current: .bool(true), readOnly: true)
        unit.settings["documentLoaded"] = Setting(current: .bool(true), readOnly: true)
        value.units = [unit]; return value
    }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        captures += 1; try await store.markScanning(settings: [:])
        for sequence in 1...(request.duplex ? 2 : 1) {
            let context = try #require(CGContext(data: nil, width: 800, height: 600, bitsPerComponent: 8, bytesPerRow: 3200,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
            let file = store.incomingDirectory.appendingPathComponent("page-\(sequence).png")
            let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
            CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
            #expect(CGImageDestinationFinalize(destination)); try await store.preserve(file, sequence: sequence)
        }
        do { while hold { try await Task.sleep(for: .milliseconds(5)) } }
        catch is CancellationError { return CaptureOutcome(cancelled: true) }
        return CaptureOutcome()
    }
}
func httpPoll(_ client: any TestClientProtocol, id: UUID, headers: HTTPFields, terminal: Bool = true) async throws -> ScanRecord {
    let deadline = ContinuousClock.now + .seconds(5)
    while ContinuousClock.now < deadline {
        let response = try await client.execute(uri: "/api/v1/jobs/\(id)", method: .get, headers: headers)
        let job = try decode(response, as: ScanRecord.self)
        if terminal ? job.state.terminal : !job.images.isEmpty { return job }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("test", "HTTP job timed out.")
}

@Test @MainActor func realHTTPWorkflowThroughServiceSocket() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("h5-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = HTTPMock()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    let socket = root.appendingPathComponent("c.sock").path
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: coordinator, backendName: "mock").respond)
    try server.start(); defer { server.stopAccepting() }
    let gateway = try ScannerGateway(socketPath: socket, token: httpTestToken, configuration: GatewayConfiguration())
    let app = Application(responder: gateway)
    try await app.test(.ahc()) { client in
        let anonymous = try await client.execute(uri: "/api/v1/devices", method: .get, headers: [HTTPField.Name("Host")!: "127.0.0.1:8170"])
        #expect(anonymous.status.code == 401)
        let session = try await client.execute(uri: "/api/v1/session", method: .post, headers: httpHeaders())
        #expect(session.status.code == 201)
        let sessionValue = try decode(session, as: JSONValue.self)
        guard case .object(let fields) = sessionValue, case .string(let token) = fields["token"] else { throw ScannerError("test", "Missing session token") }
        let headers = httpHeaders(token)
        for route in ["devices", "devices/http-mock", "devices/http-mock/settings", "devices/http-mock/capabilities", "diagnostics", "status", "profiles", "jobs", "batches"] {
            #expect(try await client.execute(uri: "/api/v1/" + route, method: .get, headers: headers).status.code == 200)
        }
        #expect(await backend.captures == 0)
        let settings = try buffer(.object(["dpi": .number(300), "duplex": .bool(true), "format": .string("png")]))
        #expect(try await client.execute(uri: "/api/v1/devices/http-mock/settings", method: .patch, headers: headers, body: settings).status.code == 200)
        let profileID = UUID()
        let profile = try buffer(.object(["id": .string(profileID.uuidString), "name": .string("Paper"), "duplex": .bool(true)]))
        #expect(try await client.execute(uri: "/api/v1/profiles", method: .post, headers: headers, body: profile).status.code == 201)
        #expect(try await client.execute(uri: "/api/v1/profiles/\(profileID)", method: .patch, headers: headers, body: buffer(.object(["name": .string("Paper updated")]))).status.code == 200)
        let requestID = UUID()
        var jobHeaders = headers; jobHeaders[HTTPField.Name("Idempotency-Key")!] = requestID.uuidString
        let jobBody = try buffer(.object(["profileID": .string(profileID.uuidString)]))
        let stableHeaders = jobHeaders
        // Independent HTTP connections share service idempotency and exclusive hardware ownership.
        try await withThrowingTaskGroup(of: Void.self) { group in
            for _ in 0..<8 { group.addTask {
                let response = try await client.execute(uri: "/api/v1/jobs", method: .post, headers: stableHeaders, body: jobBody)
                #expect(response.status.code == 202)
                #expect(try decode(response, as: ScanRecord.self).id == requestID)
            } }
            try await group.waitForAll()
        }
        let conflict = try await client.execute(uri: "/api/v1/jobs", method: .post, headers: stableHeaders,
            body: buffer(.object(["dpi": .number(600)])))
        #expect(conflict.status.code == 409)
        let job = try await httpPoll(client, id: requestID, headers: headers)
        #expect(job.state == .completed && job.images.count == 2 && job.request.format == .png)
        #expect(await backend.captures == 1)
        for suffix in ["", "/manifest"] {
            let result = try await client.execute(uri: "/api/v1/batches/\(requestID)" + suffix, method: .get, headers: headers)
            #expect(try decode(result, as: ScanRecord.self).images.count == 2)
        }
        for image in job.images {
            let path = "/api/v1/images/\(requestID):\(image.sequence)"
            let original = try await client.execute(uri: path, method: .get, headers: headers)
            #expect(original.status.code == 200 && original.headers[.contentType] == "image/png")
            let bytes = Data(original.body.readableBytesView)
            #expect(SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined() == image.sha256)
            #expect(bytes == (try Data(contentsOf: URL(fileURLWithPath: job.outputDirectory).appendingPathComponent(image.relativePath))))
            let thumbnail = try await client.execute(uri: path + "/thumbnail", method: .get, headers: headers)
            #expect(thumbnail.status.code == 200)
            let source = try #require(CGImageSourceCreateWithData(Data(thumbnail.body.readableBytesView) as CFData, nil))
            let preview = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
            #expect(preview.width == 512 && preview.height == 384)
            #expect(try await client.execute(uri: path + "/metadata", method: .get, headers: headers).status.code == 200)
        }
        var badAuthority = headers; badAuthority[HTTPField.Name("Host")!] = "evil.example"
        badAuthority[HTTPField.Name("X-Forwarded-Host")!] = "127.0.0.1:8170"
        #expect(try await client.execute(uri: "/api/v1/status", method: .get, headers: badAuthority).status.code == 403)
        let eventResponse = try await client.execute(uri: "/api/v1/events?seconds=1", method: .get, headers: headers)
        let events = String(buffer: eventResponse.body)
        #expect(eventResponse.headers[.contentType] == "text/event-stream")
        #expect(events.contains("event: scanner") && events.contains("completed"))
        let page = try await coordinator.events(after: 0, limit: 100, waitSeconds: 0)
        var resumed = headers; resumed[HTTPField.Name("Last-Event-ID")!] = String(page.nextCursor)
        let nextEvents = try await client.execute(uri: "/api/v1/events?seconds=1", method: .get, headers: resumed)
        #expect(!String(buffer: nextEvents.body).contains("event: scanner"))
        let resetEvents = try await client.execute(uri: "/api/v1/events?after=9007199254740991&seconds=1", method: .get, headers: headers)
        #expect(String(buffer: resetEvents.body).contains("event: reset"))
        await backend.holdCaptures()
        let secondID = UUID(); let thirdID = UUID()
        for id in [secondID, thirdID] {
            #expect(try await client.execute(uri: "/api/v1/jobs", method: .post, headers: headers,
                body: buffer(.object(["requestID": .string(id.uuidString), "useDeviceSettings": .bool(true)]))).status.code == 202)
        }
        _ = try await httpPoll(client, id: secondID, headers: headers, terminal: false)
        #expect(try await client.execute(uri: "/api/v1/devices/http-mock/settings", method: .patch, headers: headers, body: settings).status.code == 409)
        for id in [thirdID, secondID] {
            #expect(try await client.execute(uri: "/api/v1/jobs/\(id)/cancel", method: .post, headers: headers).status.code == 200)
            #expect(try await httpPoll(client, id: id, headers: headers).state == .cancelled)
        }
        #expect(await backend.captures == 2)
        #expect(try await client.execute(uri: "/api/v1/profiles/\(profileID)", method: .delete, headers: headers).status.code == 200)
        #expect(try await client.execute(uri: "/api/v1/session", method: .delete, headers: headers).status.code == 200)
        #expect(try await client.execute(uri: "/api/v1/jobs", method: .get, headers: headers).status.code == 401)
    }
    await coordinator.shutdownScans()
}
