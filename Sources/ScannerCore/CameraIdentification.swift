import Foundation
import ScannerAPI
import ScannerImageProcessing
import ScannerRecognition

extension ScannerCoordinator {
    /// No scan record, files, database writes, inventory mutation or hardware access.
    public func identifyCamera(base64: String, language: CardLanguage) async throws -> CameraIdentification {
        guard !stopping, !paused, activeScanID == nil, !automaticallyIdentifyingCards,
              cameraIdentifications == 0 else { throw ScannerError("busy", "Camera identification is busy. Try again shortly.") }
        guard base64.utf8.count <= (CameraIdentification.maximumImageBytes + 2) / 3 * 4,
              let data = Data(base64Encoded: base64), data.count <= CameraIdentification.maximumImageBytes else {
            throw ScannerError("invalidParams", "Camera image must be base64 JPEG up to 512 KiB.")
        }
        cameraIdentifications += 1; defer { cameraIdentifications -= 1 }
        let pokemon = cardRecognizer, yugioh = yugiohRecognizer, starwars = starWarsRecognizer
        return try await withThrowingTaskGroup(of: CameraIdentification.self) { group in
            group.addTask {
                let preparation = Task.detached(priority: .utility) { () throws -> (Data, String?)? in
                    guard let card = try CameraCard.extract(data) else { return nil }
                    let rotation = try CardProcessor.uprightRotation(card, language: language)
                    return (card, rotation.basis)
                }
                guard let (card, basis) = try await withTaskCancellationHandler(operation: { try await preparation.value }, onCancel: { preparation.cancel() }) else {
                    return CameraIdentification(state: .noCard)
                }
                if basis?.hasSuffix("-back-pattern") == true { return CameraIdentification(state: .cardBack) }
                if basis == "pokemon-code-text" || basis == "pokemon-code-url" { return CameraIdentification(state: .codeCard) }
                let evidence = try await pokemon.recognize(card, language: language)
                if !evidence.candidates.isEmpty || evidence.energySymbol != nil || evidence.uncataloguedCard != nil {
                    return Self.cameraResult(evidence, game: .pokemon)
                }
                if language == .english {
                    let other = try await yugioh.recognize(card, language: language)
                    if !other.candidates.isEmpty { return Self.cameraResult(other, game: .yugioh) }
                    let swu = try await starwars.recognize(card, language: language)
                    if !swu.candidates.isEmpty || swu.uncataloguedCard != nil { return Self.cameraResult(swu, game: .starwarsUnlimited) }
                }
                return CameraIdentification(state: .noMatch)
            }
            group.addTask { try await Task.sleep(for: .seconds(60)); throw ScannerError("timeout", "Camera identification timed out. Try a clearer view.") }
            defer { group.cancelAll() }
            return try await group.next()!
        }
    }
    private static func cameraResult(_ evidence: RecognitionEvidence, game: CardGame) -> CameraIdentification {
        CameraIdentification(state: evidence.candidates.isEmpty ? .noMatch : evidence.candidates.count == 1 ? .matched : .ambiguous, game: game, evidence: evidence)
    }
}
