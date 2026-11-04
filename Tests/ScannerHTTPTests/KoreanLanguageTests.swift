import Foundation
import Hummingbird
import HummingbirdTesting
import Testing
import ScannerAPI
import ScannerCore
import ScannerRecognition
import ScannerServiceSupport
import ScannerTransport
@testable import ScannerHTTPSupport

private actor KoreanHTTPRecognition: CardRecognizing {
    private(set) var languages: [CardLanguage] = []
    func recognize(_ data: Data, language: CardLanguage) -> RecognitionEvidence {
        languages.append(language)
        return RecognitionEvidence(lines: [], candidates: [])
    }
}

@Test @MainActor func koreanScanLanguageSurvivesHTTPRPCAndIdentificationRetry() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent("ko-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = HTTPMock(), recognition = KoreanHTTPRecognition()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, cardRecognizer: recognition)
    let socket = root.appendingPathComponent("c.sock").path
    let server = UnixServer(path: socket, handler: DiagnosticRouter(coordinator: coordinator).respond)
    try server.start(); defer { server.stopAccepting() }
    let app = Application(responder: try ScannerGateway(socketPath: socket, token: httpTestToken, configuration: GatewayConfiguration()))
    try await app.test(.ahc()) { client in
        let id = UUID()
        let body = try buffer(.object(["requestID": .string(id.uuidString), "prepareCards": .bool(true), "cardLanguage": .string("ko")]))
        let start = try await client.execute(uri: "/api/v1/jobs", method: .post, headers: httpHeaders(), body: body)
        #expect(start.status.code == 202)
        #expect(try decode(start, as: ScanRecord.self).request.cardLanguage == .korean)
        let record = try await httpPoll(client, id: id, headers: httpHeaders())
        #expect(record.request.cardLanguage == .korean)
        #expect(record.cardIdentifications == nil) // Unknown mock frame must not enter Pokémon recognition.
        #expect(await recognition.languages.isEmpty)
        let retry = try await client.execute(uri: "/api/v1/jobs", method: .post, headers: httpHeaders(), body: body)
        #expect(try decode(retry, as: ScanRecord.self).id == id)
        let path = "/api/v1/images/\(id):1/identification"
        let wrong = try await client.execute(uri: path, method: .post, headers: httpHeaders(),
            body: buffer(.object(["id": .string(UUID().uuidString), "language": .string("en")])))
        #expect(wrong.status.code == 422)
        let inferred = try await client.execute(uri: path, method: .post, headers: httpHeaders(),
            body: buffer(.object(["id": .string(UUID().uuidString)])))
        #expect(inferred.status.code == 202)
        #expect(try decode(inferred, as: CardIdentification.self).request.language == "ko")
        let unsupported = try await client.execute(uri: "/api/v1/jobs", method: .post, headers: httpHeaders(),
            body: buffer(.object(["requestID": .string(UUID().uuidString), "cardLanguage": .string("ja")])))
        #expect(unsupported.status.code == 400)
    }
    #expect(await backend.captures == 1)
    await coordinator.shutdownScans()
}
