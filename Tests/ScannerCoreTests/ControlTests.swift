import Foundation
import Testing
import ScannerAPI
import ScannerPersistence
@testable import ScannerCore

private actor ControlBackend: ScannerBackend {
    var captures = 0
    var lastRequest: ScanRequest?
    var hold = true
    func devices() -> [Device] { [Device(id: "control", name: "Control mock")] }
    func inspect(deviceID: String) -> Inspection {
        var inspection = Inspection(device: Device(id: deviceID, name: "Control mock"))
        inspection.sessionOpened = true; inspection.sessionClosed = true
        var unit = UnitSnapshot(); unit.type = 3
        unit.settings["resolutionDPI"] = Setting(current: .number(300), allowed: .array([.number(300), .number(600)]), readOnly: false)
        unit.settings["supportsDuplexScanning"] = Setting(current: .bool(true), readOnly: true)
        unit.settings["documentLoaded"] = Setting(current: .bool(true), readOnly: true)
        inspection.units = [unit]
        return inspection
    }
    func release() { hold = false }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        captures += 1; lastRequest = request
        try await store.markScanning(settings: [:])
        let image = store.incomingDirectory.appendingPathComponent("test.png")
        try makeTestPNG(at: image); try await store.preserve(image, sequence: 1)
        do { while hold { try await Task.sleep(for: .milliseconds(5)) } }
        catch is CancellationError { return CaptureOutcome(cancelled: true) }
        return CaptureOutcome()
    }
}
@Test func configurationReadinessAndImageRetrievalStayServiceOwned() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = ControlBackend(); let db = try ScannerDatabase()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    #expect(try await service.deviceStatus(nil).ready == true)
    #expect(await backend.captures == 0)
    await #expect(throws: ScannerError.self) { try await service.setSettings(deviceID: nil, settings: ScanSettings(dpi: 301)) }
    _ = try await service.setSettings(deviceID: nil, settings: ScanSettings(dpi: 600, duplex: true, format: .png))
    #expect(try db.deviceSettings("control")?.dpi == 600)
    let request = ScanRequest(useDeviceSettings: true)
    let record = try await service.startScan(request)
    let deadline = ContinuousClock.now + .seconds(3)
    while (try await service.scanStatus(record.id)).images.isEmpty && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(5)) }
    #expect(await backend.lastRequest?.dpi == 600)
    #expect(try await service.deviceStatus(nil).ready == false)
    await #expect(throws: ScannerError.self) { try await service.setSettings(deviceID: nil, settings: ScanSettings()) }
    let metadata = try await service.imageMetadata(jobID: record.id, sequence: 1)
    var bytes = Data(); var offset = 0
    repeat {
        let chunk = try await service.readImage(jobID: record.id, sequence: 1, offset: offset, length: 50)
        bytes.append(try #require(Data(base64Encoded: chunk.base64))); offset = chunk.nextOffset
        if chunk.eof { break }
    } while offset < metadata.image.bytes
    let file = URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(metadata.image.relativePath)
    #expect(try bytes == Data(contentsOf: file))
    await #expect(throws: ScannerError.self) { try await service.readImage(jobID: record.id, sequence: 1, offset: 0, length: 262145) }
    let thumbnail = try await service.thumbnail(jobID: record.id, sequence: 1)
    #expect(thumbnail.width <= 512 && thumbnail.height <= 512)
    #expect(Data(base64Encoded: thumbnail.base64)?.starts(with: [137, 80, 78, 71]) == true)
    #expect(try Data(contentsOf: file) == bytes)
    _ = try await service.cancelScan(record.id)
    await service.shutdownScans()
    // Replace only a temporary fixture with a link; arbitrary paths cannot be read.
    try FileManager.default.removeItem(at: file)
    try FileManager.default.createSymbolicLink(at: file, withDestinationURL: URL(fileURLWithPath: "/etc/passwd"))
    await #expect(throws: ScannerError.self) { try await service.readImage(jobID: record.id, sequence: 1, offset: 0, length: 50) }
    await #expect(throws: ScannerError.self) { try await service.thumbnail(jobID: record.id, sequence: 1) }
    let restarted = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    _ = try await restarted.setSettings(deviceID: nil, settings: ScanSettings(dpi: 300, format: .png))
    #expect(try await restarted.startScan(request).request.dpi == 600)
    #expect(await backend.captures == 1)
}
@Test func profileNamesPartialUpdatesAndCreateConflicts() async throws {
    let service = try ScannerCoordinator(backend: ControlBackend())
    let original = ScanProfile(name: "pokemon", dpi: 600, duplex: true, format: .png)
    #expect(try await service.createProfile(original) == original)
    #expect(try await service.createProfile(original) == original)
    await #expect(throws: ScannerError.self) { try await service.createProfile(ScanProfile(name: "POKEMON")) }
    await #expect(throws: ScannerError.self) { try await service.saveProfile(ScanProfile(name: "pokemon")) }
    let updated = try await service.updateProfile("Pokemon", patch: ProfilePatch(name: "Paper"))
    #expect(updated.dpi == 600 && updated.duplex && updated.format == .png)
    #expect(try await service.profile("Paper").id == original.id)
    await #expect(throws: ScannerError.self) { try await service.updateProfile("Paper", patch: ProfilePatch()) }
    try await service.removeProfile("Paper")
    await #expect(throws: ScannerError.self) { try await service.profile(original.id.uuidString) }
}
