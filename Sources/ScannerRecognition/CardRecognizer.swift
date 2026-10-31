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
        var best = ranked.max { ($0.1.first?.score ?? 0) < ($1.1.first?.score ?? 0) }!
        var candidates = try await candidates(for: best.1)
        var warnings = ["Suggestions require manual confirmation. Foil, reverse holo, edition and condition are not identified."]
        if candidates.isEmpty, !best.1.isEmpty {
            // A rarity symbol can merge with a set total in whole-card OCR.
            // Re-read the footer pixels; never trim digits to fit the catalogue.
            let reading = best.0
            let retry = Task.detached(priority: .utility) { try CardOCR.refineFooter(data, reading: reading) }
            let refined = try await withTaskCancellationHandler { try await retry.value } onCancel: { retry.cancel() }
            let previousIDs = Set(best.1.map(\.card.id))
            let refinedRank = CardMatcher.rank(lines: refined.lines, catalogue: cards).filter {
                previousIDs.contains($0.card.id) && !$0.totals.isEmpty
            }
            let refinedCandidates = try await self.candidates(for: refinedRank)
            if !refinedCandidates.isEmpty {
                best = (refined, refinedRank)
                candidates = refinedCandidates
                warnings.append("Collector number and set total re-read from the footer after whole-card OCR failed catalogue validation.")
            }
        }
        if best.1.count > 5 { warnings.append("Showing up to five candidates. Other printings may match; do not choose one unless its set and number agree.") }
        if candidates.isEmpty { warnings.append("No supported match. Check the front orientation, text clarity and catalogue coverage; originals are preserved.") }
        return RecognitionEvidence(lines: best.0.lines, candidates: candidates, warnings: warnings, orientation: best.0.rotation)
    }

    private func candidates(for ranked: [CardMatcher.Ranked]) async throws -> [CardCandidate] {
        var candidates: [CardCandidate] = []
        for match in ranked.prefix(8) {
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
        return candidates
    }
}

enum CardOCR {
    struct Reading: Sendable { var rotation: Int; var lines: [RecognizedLine] }
    private static func image(_ data: Data) throws -> CGImage {
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
        return image
    }
    static func read(_ data: Data) throws -> [Reading] {
        let image = try image(data)
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
    static func refineFooter(_ data: Data, reading: Reading) throws -> Reading {
        try Task.checkCancellation()
        let orientations: [CGImagePropertyOrientation] = [.up, .right, .down, .left]
        guard orientations.indices.contains(reading.rotation) else { throw ScannerError("recognition", "Invalid OCR orientation.") }
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.recognitionLanguages = ["en-US"]
        request.usesLanguageCorrection = false
        request.minimumTextHeight = 0
        let numbered = reading.lines.filter { $0.y < 0.15 && $0.height < 0.1 && $0.text.range(of: #"[0-9]\s*/\s*[A-Za-z]{0,5}[0-9]"#, options: .regularExpression) != nil }
        guard numbered.count == 1, let number = numbered.first, number.width <= 0.45 else { return reading }
        // Isolate the printed number from adjacent flavour text. Keep the whole
        // observation and its nearby rarity mark inside the region.
        let x = number.x + number.width <= 0.45 ? 0 : min(0.55, max(0, number.x + number.width / 2 - 0.225))
        request.regionOfInterest = CGRect(x: x, y: 0, width: 0.45, height: 0.15)
        try VNImageRequestHandler(cgImage: image(data), orientation: orientations[reading.rotation]).perform([request])
        let footer = (request.results ?? []).prefix(32).compactMap { observation -> RecognizedLine? in
            guard let text = observation.topCandidates(1).first, text.confidence >= 0.35 else { return nil }
            // Vision normalizes these observations to regionOfInterest. Map
            // them back to whole-card coordinates before matching title/footer.
            let relative = observation.boundingBox
            let region = request.regionOfInterest
            let box = CGRect(x: region.minX + relative.minX * region.width,
                             y: region.minY + relative.minY * region.height,
                             width: relative.width * region.width, height: relative.height * region.height)
            return RecognizedLine(text: String(text.string.prefix(160)), confidence: text.confidence,
                x: box.minX, y: box.minY, width: box.width, height: box.height)
        }
        // Keep the independently read title. A footer cannot invent the card name.
        return Reading(rotation: reading.rotation, lines: reading.lines.filter { $0.y >= 0.15 } + footer)
    }

}
