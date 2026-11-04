import Foundation
import Hummingbird
import HummingbirdTesting
import HTTPTypes
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerRecognition
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerHTTPSupport

struct HTTPRecognitionMock: CardRecognizing {
    func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence {
        try await Task.sleep(for: .milliseconds(200))
        return RecognitionEvidence(lines: [RecognizedLine(text: "Furret", y: 0.9), RecognizedLine(text: "136/189", y: 0.01)], candidates: [
            CardCandidate(id: "swsh3-136", name: "Furret", localID: "136", setID: "swsh3", setName: "Darkness Ablaze", officialSetCount: 189,
                          evidence: ["Fixture name", "Fixture collector number"], availableFinishes: [.normal, .reverseHolo], category: "Pokemon", types: ["Colorless"])])
    }
}

@Test @MainActor func authenticatedIdentificationAndReviewPreserveScan() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("ri-" + String(UUID().uuidString.prefix(12)))
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = HTTPMock()
    let database = try ScannerDatabase()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, database: database, cardRecognizer: HTTPRecognitionMock())
    var record = try await coordinator.startScan(ScanRequest(duplex: true))
    let deadline = ContinuousClock.now + .seconds(5)
    while !record.state.terminal, ContinuousClock.now < deadline {
        try await Task.sleep(for: .milliseconds(10)); record = try await coordinator.scanStatus(record.id)
    }
    let back = try #require(record.images.last)
    var preparedBack = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: 2), sourceSHA256: back.sha256, image: back)
    preparedBack.image.relativePath = "edit-\(preparedBack.id.uuidString.lowercased()).png"
    try FileManager.default.copyItem(atPath: record.outputDirectory + "/" + back.relativePath, toPath: record.outputDirectory + "/" + preparedBack.image.relativePath)
    preparedBack.cardPreparation = CardPreparation(crop: ImageCrop(), quarterTurns: 0, orientationVerified: true, warnings: [], orientationBasis: "pokemon-back-pattern")
    record.processedImages = [preparedBack]
    try database.save(record)
    let job = record
    let socket = root.appendingPathComponent("c.sock").path
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: coordinator).respond)
    try server.start(); defer { server.stopAccepting() }
    let app = Application(responder: try ScannerGateway(socketPath: socket, token: httpTestToken, configuration: GatewayConfiguration()))
    try await app.test(.ahc()) { client in
        let path = "/api/v1/images/\(job.id):1/identification"
        let id = UUID()
        let body = try buffer(.object(["id": .string(id.uuidString), "language": .string("en")]))
        #expect(try await client.execute(uri: path, method: .post, headers: [HTTPField.Name("Host")!: "127.0.0.1:8170"], body: body).status.code == 401)
        let response = try await client.execute(uri: path, method: .post, headers: httpHeaders(), body: body)
        #expect(response.status.code == 202)
        #expect(try decode(response, as: CardIdentification.self).state == .processing)
        #expect(try await client.execute(uri: path, method: .post, headers: httpHeaders(), body: body).status.code == 202)
        var value: CardIdentification?
        let timeout = ContinuousClock.now + .seconds(5)
        struct Envelope: Decodable { var identification: CardIdentification? }
        while ContinuousClock.now < timeout {
            value = try decode(await client.execute(uri: path, method: .get, headers: httpHeaders()), as: Envelope.self).identification
            if value?.state == .reviewRequired { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(value?.state == .reviewRequired)
        let review = try buffer(.object(["identificationID": .string(id.uuidString), "cardID": .string("swsh3-136"), "finish": .string("reverseHolo"), "condition": .string("nearMint")]))
        let reviewed = try await client.execute(uri: path, method: .patch, headers: httpHeaders(), body: review)
        #expect(reviewed.status.code == 200)
        #expect(try decode(reviewed, as: CardIdentification.self).state == .confirmed)
        let finishPath = "/api/v1/images/\(job.id):1/finish-estimate"
        let finishBody = try buffer(.object(["identificationID": .string(id.uuidString)]))
        #expect(try await client.execute(uri: finishPath, method: .post, headers: [HTTPField.Name("Host")!: "127.0.0.1:8170"], body: finishBody).status.code == 401)
        let estimated = try decode(await client.execute(uri: finishPath, method: .post, headers: httpHeaders(), body: finishBody), as: CardIdentification.self)
        #expect(estimated.finishEstimate != nil && estimated.finish == .reverseHolo && estimated.condition == .nearMint)
        #expect(estimated.reviewedAt == (try decode(reviewed, as: CardIdentification.self)).reviewedAt)
        #expect(try await client.execute(uri: finishPath, method: .post, headers: httpHeaders(), body: buffer(.object(["identificationID": .string(id.uuidString), "path": .string("/etc/passwd")]))).status.code == 400)

        let conditionPath = "/api/v1/images/\(job.id):1/condition-check"
        #expect(try await client.execute(uri: conditionPath, method: .post, headers: [HTTPField.Name("Host")!: "127.0.0.1:8170"], body: finishBody).status.code == 401)
        let checked = try decode(await client.execute(uri: conditionPath, method: .post, headers: httpHeaders(), body: finishBody), as: CardIdentification.self)
        #expect(checked.conditionCheck?.state == .unsupported && checked.condition == .nearMint && checked.finish == .reverseHolo)
        #expect(checked.reviewedAt == estimated.reviewedAt)
        #expect(try await client.execute(uri: conditionPath, method: .post, headers: httpHeaders(), body: buffer(.object(["identificationID": .string(id.uuidString), "path": .string("/etc/passwd")]))).status.code == 400)
        #expect(try await client.execute(uri: path, method: .patch, headers: httpHeaders(), body: buffer(.object(["identificationID": .string(id.uuidString), "finish": .string("invented")]))).status.code == 400)
        #expect(try await client.execute(uri: path + "?path=/etc/passwd", method: .get, headers: httpHeaders()).status.code == 400)
        let manifest = try decode(await client.execute(uri: "/api/v1/batches/\(job.id)/manifest", method: .get, headers: httpHeaders()), as: ScanRecord.self)
        #expect(manifest.cardIdentifications?.first?.condition == .nearMint)
        #expect(manifest.images.first?.sha256 == job.images.first?.sha256)
        #expect(manifest.state == .completed)
        let preparePath = "/api/v1/images/\(job.id):1/prepare-card"
        // A blank mock frame is deliberately rejected, with a client-actionable error.
        let preparation = try await client.execute(uri: preparePath, method: .post, headers: httpHeaders(), body: buffer(.object(["id": .string(UUID().uuidString)])))
        #expect(preparation.status.code == 422)
        #expect(try await client.execute(uri: preparePath, method: .post, headers: httpHeaders(), body: buffer(.object(["id": .string(UUID().uuidString), "path": .string("/etc/passwd")]))).status.code == 400)
    }
    #expect(await backend.captures == 1)
    await coordinator.shutdownScans()
}
