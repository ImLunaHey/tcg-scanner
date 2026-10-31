import CoreGraphics
import Foundation
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerServiceSupport
import ScannerTransport

private actor IntegrationBackend: ScannerBackend {
    var captureCount = 0
    var hold = true
    func devices() -> [Device] { [Device(id: "integration", name: "Integration mock")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Integration mock")) }
    func release() { hold = false }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        captureCount += 1
        try await store.markScanning(settings: [:])
        let context = try #require(CGContext(data: nil, width: 8, height: 8, bitsPerComponent: 8, bytesPerRow: 32,
                                            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
        let image = try #require(context.makeImage())
        let file = store.incomingDirectory.appendingPathComponent("page.png")
        let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        #expect(CGImageDestinationFinalize(destination))
        try await store.preserve(file, sequence: 1)
        do { while hold { try await Task.sleep(for: .milliseconds(5)) } }
        catch is CancellationError { return CaptureOutcome(cancelled: true) }
        return CaptureOutcome()
    }
}
private func call(_ path: String, _ method: String, _ params: JSONValue = .object([:])) async throws -> JSONValue {
    var request = RPCRequest(method: "v1." + method); request.params = params
    let response = try await UnixClient(path: path).callAsync(request)
    if let error = response.error { throw error }
    return try #require(response.result)
}
private func waitForImages(_ path: String, id: UUID, terminal: Bool) async throws -> ScanRecord {
    let deadline = ContinuousClock.now + .seconds(5)
    while ContinuousClock.now < deadline {
        let result = try await call(path, "jobs.get", .object(["id": .string(id.uuidString)])).decode(ScanRecord.self)
        if terminal ? result.state.terminal : !result.images.isEmpty { return result }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("testTimeout", "Job did not progress.")
}

@Test @MainActor func multipleSocketClientsQueueCancelAndReadHistoryAfterRestart() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("ss-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let socket = root.appendingPathComponent("s.sock").path
    let databasePath = root.appendingPathComponent("state/scanner.sqlite").path
    let database = try ScannerDatabase(path: databasePath)
    let backend = IntegrationBackend()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, database: database)
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: coordinator).respond)
    try server.start()
    defer { server.stopAccepting() }
    let request = ScanRequest(format: .png)
    let payload = try JSONValue.wrapping(request)
    let ids = try await withThrowingTaskGroup(of: UUID.self) { group in
        for _ in 0..<12 { group.addTask { try await call(socket, "jobs.create", payload).decode(ScanRecord.self).id } }
        var ids: [UUID] = []
        for try await id in group { ids.append(id) }
        return ids
    }
    #expect(Set(ids) == [request.requestID])
    _ = try await waitForImages(socket, id: request.requestID, terminal: false)
    let queued = try await call(socket, "jobs.create", .wrapping(ScanRequest(format: .png))).decode(ScanRecord.self)
    #expect(queued.state == .queued)
    #expect(try await call(socket, "jobs.cancel", .object(["id": .string(queued.id.uuidString)])).decode(ScanRecord.self).state == .cancelled)
    let before = try await call(socket, "events").decode(EventPage.self)
    let subscription = Task { try await call(socket, "events", .object(["after": .number(Double(before.nextCursor)), "waitSeconds": .number(2)])).decode(EventPage.self) }
    _ = try await call(socket, "jobs.cancel", .object(["id": .string(request.requestID.uuidString)]))
    let cancelled = try await waitForImages(socket, id: request.requestID, terminal: true)
    #expect(cancelled.state == .cancelled && cancelled.images.count == 1)
    #expect(try await subscription.value.events.contains { $0.kind == "job.updated" })
    #expect(await backend.captureCount == 1)
    #expect(try await call(socket, "jobs.list", .object(["limit": .number(1)])).decode(JobPage.self).nextOffset == 1)
    await coordinator.shutdownScans()
    while server.activeConnections > 0 { try await Task.sleep(for: .milliseconds(10)) }
    server.stopAccepting()
    let reopened = try ScannerDatabase(path: databasePath)
    #expect(try reopened.recoverInterruptedJobs(storageRoot: root) == 0)
    let restarted = try ScannerCoordinator(backend: backend, storageRoot: root, database: reopened)
    let nextServer = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: restarted).respond)
    try nextServer.start()
    defer { nextServer.stopAccepting() }
    let replay = try await call(socket, "jobs.create", payload).decode(ScanRecord.self)
    #expect(replay.state == .cancelled && replay.images.count == 1)
    #expect(await backend.captureCount == 1)
    #expect(try await call(socket, "batches.get", .object(["id": .string(replay.id.uuidString)])).decode(ScanRecord.self).images[0].sha256 == cancelled.images[0].sha256)
    await backend.release()
    let next = try await call(socket, "jobs.create", .wrapping(ScanRequest(format: .png))).decode(ScanRecord.self)
    #expect(try await waitForImages(socket, id: next.id, terminal: true).state == .completed)
    await restarted.shutdownScans()
    while nextServer.activeConnections > 0 { try await Task.sleep(for: .milliseconds(10)) }
}

@Test func serviceRoutesRejectUnknownKeysAndUnboundedReads() async throws {
    let coordinator = try ScannerCoordinator(backend: IntegrationBackend())
    let router = DiagnosticRouter(coordinator: coordinator)
    for (method, params) in [("jobs.list", JSONValue.object(["limit": .number(101)])),
                              ("events", .object(["waitSeconds": .number(21)])),
                              ("events", .object(["after": .number(-1)])),
                              ("service.status", .object(["shell": .string("not permitted")])),
                              ("jobs.get", .object(["id": .string("invalid")]))] {
        var request = RPCRequest(method: "v1." + method); request.params = params
        #expect(await router.respond(to: try Wire.encoder().encode(request)).error?.code == -32602)
    }
}
