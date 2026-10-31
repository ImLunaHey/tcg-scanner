import Foundation
import Testing
import ScannerAPI
import ScannerPersistence

@Test func durableJobsProfilesIdempotencyAndEvents() throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let path = root.appendingPathComponent("state.sqlite").path
    let request = ScanRequest()
    var record = ScanRecord(device: Device(id: "test", name: "Test"), request: request, outputDirectory: root.appendingPathComponent("batch").path)
    record.state = .queued
    let profile = ScanProfile(name: "Documents")
    do {
        let db = try ScannerDatabase(path: path)
        try db.insert(record, submission: request)
        #expect(throws: (any Error).self) { try db.insert(record, submission: request) }
        try db.saveProfile(profile)
        record.state = .completed
        try db.save(record)
    }
    let reopened = try ScannerDatabase(path: path)
    #expect(try reopened.job(record.id)?.state == .completed)
    #expect(try reopened.submission(record.id) == request)
    #expect(try reopened.profiles() == [profile])
    #expect(try reopened.jobs(limit: 1).jobs.count == 1)
    let page = try reopened.events(after: 0)
    #expect(page.events.count == 3)
    #expect(try reopened.events(after: page.nextCursor).events.isEmpty)
    #expect(try reopened.events(after: page.nextCursor + 1).resetRequired)
    #expect(try reopened.recoverInterruptedJobs(storageRoot: root) == 0)
}

@Test func recoveryReconcilesManifestAndNeverResumesQueuedOrActiveJobs() throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let db = try ScannerDatabase()
    for state in [ScanState.queued, .scanning, .completed] {
        let request = ScanRequest()
        let directory = root.appendingPathComponent(request.requestID.uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: request, outputDirectory: directory.path)
        record.state = state == .completed ? .processing : state
        try db.insert(record, submission: request)
        record.state = state
        record.images = [OriginalImage(sequence: 1, relativePath: "originals/page.tiff", bytes: 1, sha256: "test", width: 1, height: 1, imageCount: 1, uniformType: "public.tiff")]
        try Wire.encoder().encode(record).write(to: directory.appendingPathComponent("manifest.json"))
    }
    #expect(try db.recoverInterruptedJobs(storageRoot: root) == 3)
    #expect(try db.unfinished().isEmpty)
    let jobs = try db.jobs().jobs
    #expect(jobs.filter { $0.state == .failed }.count == 2)
    #expect(jobs.filter { $0.state == .completed }.count == 1)
    #expect(jobs.allSatisfy { $0.progress.imagesSaved == 1 })
    #expect(try db.recoverInterruptedJobs(storageRoot: root) == 0)
}

@Test func deviceTransitionsAndInspectionCachePersist() throws {
    let db = try ScannerDatabase()
    let device = Device(id: "mock", name: "Mock")
    try db.observeDevices([device]); try db.observeDevices([device]); try db.observeDevices([])
    try db.observeDevices([device])
    #expect(try db.events(after: 0).events.map(\.kind) == ["device.connected", "device.disconnected", "device.connected"])
    try db.saveInspection(Inspection(device: device))
    #expect(try db.cachedInspection("mock")?.device.id == "mock")
    #expect(throws: ScannerError.self) { try db.saveProfile(ScanProfile(name: "")) }
    #expect(throws: ScannerError.self) { try db.jobs(limit: 101) }
}

@Test func persistenceRejectsUnsafePathsAndRecoveryLeavesSymlinksUntouched() throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    defer { try? FileManager.default.removeItem(at: root) }
    let sentinel = root.appendingPathComponent("sentinel")
    try Data("preserve me".utf8).write(to: sentinel)
    let link = root.appendingPathComponent("linked.sqlite")
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: sentinel)
    #expect(throws: ScannerError.self) { try ScannerDatabase(path: link.path) }
    let manifest = root.appendingPathComponent("manifest.json")
    try FileManager.default.createSymbolicLink(at: manifest, withDestinationURL: sentinel)
    let database = try ScannerDatabase()
    let request = ScanRequest()
    let record = ScanRecord(device: Device(id: "test", name: "Test"), request: request, outputDirectory: root.path)
    try database.insert(record, submission: request)
    try database.recoverInterruptedJobs(storageRoot: root.deletingLastPathComponent())
    #expect(try String(contentsOf: sentinel, encoding: .utf8) == "preserve me")
    #expect(try FileManager.default.destinationOfSymbolicLink(atPath: manifest.path) == sentinel.path)
    #expect(try database.job(record.id)?.state == .failed)
}
