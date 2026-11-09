import CoreGraphics
import Foundation
import Hummingbird
import HummingbirdTesting
import HTTPTypes
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerImageProcessing
import ScannerPersistence
import ScannerRecognition
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerHTTPSupport

func cameraFixture(card: Bool = true, width: Int = 1000, height: Int = 1333) throws -> Data {
    let context = try #require(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
    context.setFillColor(CGColor(gray: 0.1, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    if card {
        context.setFillColor(CGColor(gray: 0.95, alpha: 1)); context.fill(CGRect(x: 120, y: 130, width: 760, height: 1060))
        context.setFillColor(CGColor(red: 0.4, green: 0.6, blue: 0.8, alpha: 1)); context.fill(CGRect(x: 160, y: 650, width: 680, height: 470))
    }
    let data = NSMutableData()
    let output = try #require(CGImageDestinationCreateWithData(data, "public.jpeg" as CFString, 1, nil))
    CGImageDestinationAddImage(output, try #require(context.makeImage()), nil)
    #expect(CGImageDestinationFinalize(output))
    return data as Data
}

@Test func cameraRejectsInvalidInputAndExtractsWholeCard() throws {
    #expect(throws: ScannerError.self) { try CameraCard.extract(Data("not JPEG".utf8)) }
    #expect(throws: ScannerError.self) { try CameraCard.extract(Data(repeating: 0, count: 512 * 1024 + 1)) }
    #expect(throws: ScannerError.self) { try CameraCard.extract(cameraFixture(width: 100, height: 100)) }
    #expect(try CameraCard.extract(cameraFixture(card: false)) == nil)
    let card = try #require(try CameraCard.extract(cameraFixture()))
    let source = try #require(CGImageSourceCreateWithData(card as CFData, nil))
    let image = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
    #expect((740...780).contains(image.width))
    #expect((1040...1080).contains(image.height))
}

@Test @MainActor func cameraHTTPIsPublicEphemeralAndBounded() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("cam-" + String(UUID().uuidString.prefix(10)))
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = HTTPMock(), database = try ScannerDatabase()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root.appendingPathComponent("must-not-exist"), database: database, validateStorage: { throw ScannerError("storageUnavailable", "Fixture share offline") }, cardRecognizer: HTTPRecognitionMock())
    let before = try Wire.encoder().encode(database.events(after: 0))
    let socket = root.appendingPathComponent("c.sock").path
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: coordinator).respond)
    try server.start(); defer { server.stopAccepting() }
    let app = Application(responder: try ScannerGateway(socketPath: socket, token: httpTestToken, configuration: GatewayConfiguration()))
    var frame = try cameraFixture()
    frame.append(Data(repeating: 0, count: 80 * 1024)) // Exercise the larger camera HTTP and IPC budgets.
    let body = try buffer(.object(["base64": .string(frame.base64EncodedString()), "language": .string("en")]))
    try await app.test(.ahc()) { client in
        let path = "/api/v1/camera/identify"
        #expect(try await client.execute(uri: path, method: .post, headers: httpHeaders(), body: buffer(.object(["base64": .string("invalid")]))).status.code == 422)
        let anonymous: HTTPFields = [.contentType: "application/json", HTTPField.Name("Host")!: "127.0.0.1:8170"]
        let lookup = try await client.execute(uri: path, method: .post, headers: anonymous, body: body)
        #expect(lookup.status.code == 200)
        #expect(try decode(lookup, as: CameraIdentification.self).state == .matched)
        for protected in ["/api/v1/jobs", "/api/v1/inventory", "/api/v1/profiles", "/api/v1/session", "/api/v1/camera/other"] {
            #expect(try await client.execute(uri: protected, method: .post, headers: anonymous, body: buffer(.object([:]))).status.code == 401)
        }
        #expect(try await client.execute(uri: path, method: .get, headers: anonymous).status.code == 401)
        #expect(try await client.execute(uri: path + "?path=/tmp/a", method: .post, headers: anonymous, body: body).status.code == 400)
        var crossOrigin = anonymous; crossOrigin[.origin] = "https://evil.example"
        #expect(try await client.execute(uri: path, method: .post, headers: crossOrigin, body: body).status.code == 403)
        let response = try await client.execute(uri: path, method: .post, headers: httpHeaders(), body: body)
        #expect(response.status.code == 200)
        #expect(response.headers[.cacheControl] == "no-store")
        let result = try decode(response, as: CameraIdentification.self)
        #expect(result.state == .matched && result.candidates.first?.name == "Furret")
        #expect(result.game == .pokemon)
        #expect(!String(buffer: response.body).contains("jobID"))
        for invalid: JSONValue in [.object(["base64": .string(""), "language": .string("xx")]), .object(["base64": .string(""), "path": .string("/tmp/foo")])] {
            #expect(try await client.execute(uri: path, method: .post, headers: httpHeaders(), body: buffer(invalid)).status.code == 400)
        }
        #expect(try await client.execute(uri: path, method: .post, headers: httpHeaders(), body: ByteBuffer(string: String(repeating: "x", count: CameraIdentification.maximumRequestBytes + 1))).status.code == 413)
    }
    #expect(try database.jobs().jobs.isEmpty)
    #expect(try Wire.encoder().encode(database.events(after: 0)) == before)
    #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("must-not-exist").path))
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest()) }
    #expect(await backend.captures == 0)
    await coordinator.shutdownScans()
}

private actor CameraWaitingRecognizer: CardRecognizing {
    var entered = false
    var wait = true
    func release() { wait = false }
    func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence {
        entered = true
        while wait { try await Task.sleep(for: .milliseconds(5)) }
        return try await HTTPRecognitionMock().recognize(data, language: language)
    }
}

@Test func cameraConcurrentRequestsAreBoundedAndCancellationReleasesSlot() async throws {
    let recognizer = CameraWaitingRecognizer()
    let coordinator = try ScannerCoordinator(backend: HTTPMock(), cardRecognizer: recognizer)
    let base64 = try cameraFixture().base64EncodedString()
    let first = Task { try await coordinator.identifyCamera(base64: base64, language: .english) }
    defer { first.cancel() }
    let deadline = ContinuousClock.now + .seconds(5)
    while !(await recognizer.entered), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await recognizer.entered)
    do { _ = try await coordinator.identifyCamera(base64: base64, language: .english); Issue.record("Expected busy") }
    catch let error as ScannerError { #expect(error.kind == "busy") }
    first.cancel()
    if case .success = await first.result { Issue.record("Expected cancelled lookup") }
    await recognizer.release()
    #expect(try await coordinator.identifyCamera(base64: base64, language: .english).state == .matched)
    await coordinator.shutdownScans()
}


@Test func publicCameraLimitsDoNotConsumeControlOrAuthenticationQuota() async throws {
    let access = try GatewayAccess(token: httpTestToken)
    for _ in 0..<60 { try await access.rateLimitCamera() }
    do { try await access.rateLimitCamera(); Issue.record("Expected camera rate limit") }
    catch let error as GatewayError { #expect(error.status == 429) }
    #expect(try await access.authenticate("Bearer " + httpTestToken) == "root")
    try await access.acquire("camera")
    await #expect(throws: GatewayError.self) { try await access.acquire("camera") }
    try await access.acquire("request")
    await access.release("request"); await access.release("camera")
    try await access.acquire("camera"); await access.release("camera")
}
