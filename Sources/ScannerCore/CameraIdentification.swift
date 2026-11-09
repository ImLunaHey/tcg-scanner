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
        let pokemon = cardRecognizer, yugioh = yugiohRecognizer, starwars = starWarsRecognizer, finish = finishAnalyzer
        return try await withThrowingTaskGroup(of: CameraIdentification.self) { group in
            group.addTask {
                let preparation = Task.detached(priority: .utility) { () throws -> (Data, Int?, String)? in
                    guard let card = try CameraCard.extract(data) else { return nil }
                    let rotation = try CardProcessor.uprightRotation(card, language: language)
                    return (card, rotation.verified ? rotation.turns : nil, rotation.basis)
                }
                guard let (card, orientation, basis) = try await withTaskCancellationHandler(operation: { try await preparation.value }, onCancel: { preparation.cancel() }) else {
                    return CameraIdentification(state: .noCard)
                }
                if basis.hasSuffix("-back-pattern") { return CameraIdentification(state: .cardBack) }
                if basis == "pokemon-code-text" || basis == "pokemon-code-url" { return CameraIdentification(state: .codeCard) }
                if language == .english, basis.hasPrefix("starwars-") {
                    return Self.cameraResult(try await starwars.recognize(card, language: language), game: .starwarsUnlimited)
                }
                let evidence = try await pokemon.recognize(card, language: language, preferredOrientation: orientation)
                if !evidence.candidates.isEmpty || evidence.energySymbol != nil || evidence.uncataloguedCard != nil {
                    var result = Self.cameraResult(evidence, game: .pokemon)
                    let preparation = CardPreparation(crop: ImageCrop(), quarterTurns: 0, orientationVerified: orientation != nil,
                        warnings: [], orientationBasis: basis)
                    var estimate = await finish.estimate(card, evidence: evidence, preparation: preparation, language: language, camera: true)
                    try Task.checkCancellation()
                    estimate.limitations.append("Camera estimate: lighting, glare and exposure can alter printed markers. Scanner foil-texture thresholds are not used. Check the physical card under light.")
                    result.finishEstimate = estimate
                    return result
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
