import Foundation
import Testing
import ScannerAPI
import ScannerPersistence
@testable import ScannerCore

@Test func batchLabelsStayStableAndPaddingNeverWraps() {
    #expect(BatchNaming.batchFolder(1) == "B-001")
    #expect(BatchNaming.batchFolder(1000) == "B-1000")
    #expect(BatchNaming.originalName(sequence: 23, format: .png) == "C-023.png")
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(), outputDirectory: "/Volumes/scans/B-023")
    record.batchNumber = 23
    #expect(BatchNaming.exportDirectory(record) == "B-023")
    #expect(!BatchNaming.exportDirectory(record).contains("["))
}
@Test func batchReservationsAreConcurrentIdempotentAndSurviveRestart() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let path = root.appendingPathComponent("state.sqlite").path
    let id = UUID()
    do {
        let database = try ScannerDatabase(path: path)
        let values = try await withThrowingTaskGroup(of: Int.self) { group in
            for _ in 0..<20 { group.addTask { try database.reserveBatchNumber(for: id) } }
            var results: [Int] = []
            for try await number in group { results.append(number) }
            return results
        }
        #expect(Set(values) == [1])
        #expect(try database.reserveBatchNumber(for: UUID()) == 2) // Retained even without a job.
    }
    let reopened = try ScannerDatabase(path: path)
    #expect(try reopened.reserveBatchNumber(for: id) == 1)
    #expect(try reopened.reserveBatchNumber(for: UUID()) == 3)
}
