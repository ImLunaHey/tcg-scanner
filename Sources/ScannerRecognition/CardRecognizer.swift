import Foundation
import ImageIO
import ScannerAPI
import Vision

public protocol CardRecognizing: Sendable {
    func recognize(_ data: Data) async throws -> RecognitionEvidence
}

public actor CardRecognizer: CardRecognizing {
    private let catalogue = TCGdexClient()
    public init() {}
    public func recognize(_ data: Data) async throws -> RecognitionEvidence {
        let ocr = Task.detached(priority: .utility) { try CardOCR.read(data) }
        let orientations = try await withTaskCancellationHandler { try await ocr.value } onCancel: { ocr.cancel() }
        try Task.checkCancellation()
        // Avoid a network lookup for blank pages, card backs or other images with no readable text.
        guard orientations.contains(where: { !$0.lines.isEmpty }) else {
            return RecognitionEvidence(lines: [], candidates: [], warnings: ["No readable card text. Select the front image."])
        }
        let cards = try await catalogue.catalogue()
        let ranked = orientations.map { ($0, CardMatcher.rank(lines: $0.lines, catalogue: cards)) }
        let best = ranked.max { ($0.1.first?.score ?? 0) < ($1.1.first?.score ?? 0) }!
        var candidates: [CardCandidate] = []
        for match in best.1.prefix(8) {
            try Task.checkCancellation()
            let detail = try await catalogue.detail(match.card.id)
            guard detail.id == match.card.id else { throw ScannerError("catalogue", "TCGdex card identifier did not match the requested card.") }
            let total = detail.set.cardCount?.official
            if !match.totals.isEmpty, let total, !match.totals.contains(total) { continue }
            var evidence = match.evidence
            if let total, match.totals.contains(total) { evidence.append("Printed set total " + String(total)) }
            let finishes: [CardFinish] = [("normal", CardFinish.normal), ("holo", .holo), ("reverse", .reverseHolo)]
                .filter { detail.variants?[$0.0] == true }.map(\.1)
            candidates.append(CardCandidate(id: detail.id, name: detail.name, localID: detail.localId,
                setID: detail.set.id, setName: detail.set.name, officialSetCount: total, evidence: evidence, availableFinishes: finishes, category: detail.category, types: detail.types))
            if candidates.count == 5 { break }
        }
        var warnings = ["Suggestions require manual confirmation. Foil, reverse holo, edition and condition are not identified."]
        if best.1.count > 5 { warnings.append("Showing up to five candidates. Other printings may match; do not choose one unless its set and number agree.") }
        if candidates.isEmpty { warnings.append("No supported match. Check the front orientation, text clarity and catalogue coverage; originals are preserved.") }
        return RecognitionEvidence(lines: best.0.lines, candidates: candidates, warnings: warnings, orientation: best.0.rotation)
    }
}

enum CardOCR {
    struct Reading: Sendable { var rotation: Int; var lines: [RecognizedLine] }
    static func read(_ data: Data) throws -> [Reading] {
        guard data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 20_000, height <= 20_000, width * height <= 100_000_000,
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 2400] as CFDictionary) else {
            throw ScannerError("recognition", "OCR requires a single image up to 32 MiB and 100 megapixels.")
        }
        return try [CGImagePropertyOrientation.up, .right, .down, .left].enumerated().map { rotation, orientation in
            try Task.checkCancellation()
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.recognitionLanguages = ["en-US"]
            request.usesLanguageCorrection = false // Preserve collector numbers and Pokémon spelling.
            try VNImageRequestHandler(cgImage: image, orientation: orientation).perform([request])
            let lines = (request.results ?? []).prefix(48).compactMap { observation -> RecognizedLine? in
                guard let text = observation.topCandidates(1).first, text.confidence >= 0.35 else { return nil }
                let box = observation.boundingBox
                return RecognizedLine(text: String(text.string.prefix(160)), confidence: text.confidence,
                    x: box.minX, y: box.minY, width: box.width, height: box.height)
            }
            return Reading(rotation: rotation, lines: lines)
        }
    }
}
