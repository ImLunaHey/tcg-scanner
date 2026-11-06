import Foundation
import ImageIO
import ScannerAPI
import Vision

public protocol CardRecognizing: Sendable {
    func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence
}

public actor CardRecognizer: CardRecognizing {
    private let catalogues: [CardLanguage: TCGdexClient] = [
        .english: TCGdexClient(language: .english), .korean: TCGdexClient(language: .korean)
    ]
    public init() {}
    public func recognize(_ data: Data, language: CardLanguage = .english) async throws -> RecognitionEvidence {
        let ocr = Task.detached(priority: .utility) { try CardOCR.read(data, language: language) }
        let orientations = try await withTaskCancellationHandler { try await ocr.value } onCancel: { ocr.cancel() }
        try Task.checkCancellation()
        // Avoid a network lookup for blank pages, card backs or other images with no readable text.
        guard orientations.contains(where: { !$0.lines.isEmpty }) else {
            return RecognitionEvidence(lines: [], candidates: [], warnings: ["No readable card text. Select the front image."])
        }
        let energyTask = Task.detached(priority: .utility) { () throws -> (CardOCR.Reading, EnergySymbolEvidence)? in
            for reading in orientations where EnergySymbolDetector.hasEnergyHeading(reading.lines) {
                if let symbol = try EnergySymbolDetector.analyze(data, reading: reading) { return (reading, symbol) }
            }
            return nil
        }
        let energy = try await withTaskCancellationHandler { try await energyTask.value } onCancel: { energyTask.cancel() }
        if let (reading, symbol) = energy, !Self.hasPrintedIdentifier(reading.lines) {
            // Unnumbered basic Energy is useful identity evidence on its own.
            // Do not require a network lookup or invent a catalogue printing.
            var result = RecognitionEvidence(lines: reading.lines, candidates: [], warnings: [
                "Energy type recognized from the top-right symbol. Exact printing is unknown; the symbol does not establish set, price or finish."
            ], orientation: reading.rotation)
            result.energySymbol = symbol
            return result
        }
        guard let catalogue = catalogues[language] else { throw ScannerError("unsupportedLanguage", "No catalogue client is configured for this card language.") }
        let cards = try await catalogue.catalogue()
        let ranked = orientations.map { ($0, CardMatcher.rank(lines: $0.lines, catalogue: cards, language: language)) }
        var best = energy.flatMap { symbol in ranked.first { $0.0.rotation == symbol.0.rotation } }
            ?? ranked.max { ($0.1.first?.score ?? 0) < ($1.1.first?.score ?? 0) }!
        var candidates = try await candidates(for: best.1, catalogue: catalogue)
        var warnings = ["Suggestions require manual confirmation. A base-card catalogue match alone does not establish finish, stamped edition or condition."]
        if language == .korean {
            warnings.append("Korean catalogue coverage is limited. Only Korean names and numbered Korean catalogue entries are considered; no English printing is substituted.")
        }
        if candidates.isEmpty, !best.1.isEmpty {
            // A rarity symbol can merge with a set total in whole-card OCR.
            // Re-read the footer pixels; never trim digits to fit the catalogue.
            let reading = best.0
            let retry = Task.detached(priority: .utility) { try CardOCR.refineFooter(data, reading: reading, language: language) }
            let refined = try await withTaskCancellationHandler { try await retry.value } onCancel: { retry.cancel() }
            let previousIDs = Set(best.1.map(\.card.id))
            let refinedRank = CardMatcher.rank(lines: refined.lines, catalogue: cards, language: language).filter {
                previousIDs.contains($0.card.id) && !$0.totals.isEmpty
            }
            let refinedCandidates = try await self.candidates(for: refinedRank, catalogue: catalogue)
            if !refinedCandidates.isEmpty {
                best = (refined, refinedRank)
                candidates = refinedCandidates
                warnings.append("Collector number and set total re-read from the footer after whole-card OCR failed catalogue validation.")
            }
        }
        var raritySymbol: RaritySymbolEvidence?
        let rotation = best.0.rotation
        let symbolTask = Task.detached(priority: .utility) {
            try RaritySymbolDetector.analyze(data, rotation: rotation, language: language)
        }
        do {
            let symbol = try await withTaskCancellationHandler { try await symbolTask.value } onCancel: { symbolTask.cancel() }
            raritySymbol = symbol
            candidates = RarityEvidence.apply(symbol, to: candidates)
            if candidates.contains(where: { $0.evidence.contains(where: { $0.hasPrefix("Rarity symbol conflict:") }) }) {
                warnings.append("Printed rarity and catalogue metadata disagree for a candidate. Review the identity; rarity does not establish foil finish.")
            }
        } catch is CancellationError { throw CancellationError() }
        catch { warnings.append("Rarity symbol could not be checked; text-based identification is retained.") }
        if best.1.count > 5 { warnings.append("Showing up to five candidates. Other printings may match; do not choose one unless its set and number agree.") }
        if candidates.isEmpty {
            warnings.append(energy == nil ? "No supported match. Check the front orientation, text clarity and catalogue coverage; originals are preserved." : "Energy type recognized from the top-right symbol; exact printing is unknown.")
        }
        var result = RecognitionEvidence(lines: best.0.lines, candidates: candidates, warnings: warnings, orientation: best.0.rotation)
        result.raritySymbol = raritySymbol
        if let energy, energy.0.rotation == best.0.rotation { result.energySymbol = energy.1 }
        return result
    }

    private static func hasPrintedIdentifier(_ lines: [RecognizedLine]) -> Bool {
        lines.contains { line in
            guard line.y < 0.3 else { return false }
            return line.text.range(of: #"(?i)(\d\s*/\s*\d|\b(?:SVE|SWSH|SVP|SM|XY)\s*\d|^\s*\d{1,4}\s*$)"#, options: .regularExpression) != nil
        }
    }

    private func candidates(for ranked: [CardMatcher.Ranked], catalogue: TCGdexClient) async throws -> [CardCandidate] {
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
            var candidate = CardCandidate(id: detail.id, name: detail.name, localID: detail.localId,
                setID: detail.set.id, setName: detail.set.name, officialSetCount: total, evidence: evidence, availableFinishes: finishes, category: detail.category, types: detail.types)
            candidate.printingVariants = detail.printingVariants
            candidate.rarities = detail.rarity.map { [$0] }
            candidates.append(candidate)
            if candidates.count == 5 { break }
        }
        return candidates
    }
}

enum CardOCR {
    struct Reading: Sendable { var rotation: Int; var lines: [RecognizedLine] }
    static func request(language: CardLanguage) throws -> VNRecognizeTextRequest {
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        let supported = try request.supportedRecognitionLanguages()
        guard language.recognitionLanguages.allSatisfy({ supported.contains($0) }) else {
            throw ScannerError("unsupportedLanguage", "This Mac's Vision framework does not support \(language.displayName) OCR.")
        }
        request.recognitionLanguages = language.recognitionLanguages
        request.automaticallyDetectsLanguage = false
        request.usesLanguageCorrection = false // Preserve collector numbers and Pokémon spelling.
        return request
    }
    static func image(_ data: Data) throws -> CGImage {
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
    static func read(_ data: Data, language: CardLanguage = .english) throws -> [Reading] {
        let image = try image(data)
        return try [CGImagePropertyOrientation.up, .right, .down, .left].enumerated().map { rotation, orientation in
            try Task.checkCancellation()
            let request = try Self.request(language: language)
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
    static func englishSetCode(_ data: Data, rotation: Int, magnify: Bool = false) throws -> [RecognizedLine] {
        let orientations: [CGImagePropertyOrientation] = [.up, .right, .down, .left]
        guard orientations.indices.contains(rotation) else { return [] }
        var input = try image(data)
        if magnify {
            // Small printed codes can lose a character at capture resolution.
            // Re-read enlarged pixels; never invent letters to fit a card/set.
            let scale = min(2, 2400 / Double(max(input.width, input.height)))
            guard scale > 1.1 else { return [] }
            let width = Int(Double(input.width) * scale), height = Int(Double(input.height) * scale)
            guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
                throw ScannerError("recognition", "Cannot enlarge set-code evidence.")
            }
            context.interpolationQuality = .high
            context.draw(input, in: CGRect(x: 0, y: 0, width: width, height: height))
            guard let enlarged = context.makeImage() else { throw ScannerError("recognition", "Cannot read enlarged set-code evidence.") }
            input = enlarged
        }
        let request = try Self.request(language: .english)
        request.minimumTextHeight = 0
        let region = CGRect(x: 0.03, y: 0.015, width: 0.25, height: 0.075)
        request.regionOfInterest = region
        try VNImageRequestHandler(cgImage: input, orientation: orientations[rotation]).perform([request])
        let pattern = try NSRegularExpression(pattern: #"(?i)([A-Z0-9]{3}\s*EN)\s*$"#)
        return try (request.results ?? []).compactMap { observation in
            guard let text = observation.topCandidates(1).first, text.confidence >= 0.5,
                  let match = pattern.firstMatch(in: text.string, range: NSRange(text.string.startIndex..., in: text.string)),
                  let range = Range(match.range(at: 1), in: text.string),
                  let rectangle = try text.boundingBox(for: range) else { return nil }
            // Character-range boxes separate the regulation mark from a merged code.
            let box = rectangle.boundingBox
            return RecognizedLine(text: String(text.string[range]), confidence: text.confidence,
                x: region.minX + box.minX * region.width, y: region.minY + box.minY * region.height,
                width: box.width * region.width, height: box.height * region.height)
        }
    }

    static func refineFooter(_ data: Data, reading: Reading, language: CardLanguage = .english) throws -> Reading {
        try Task.checkCancellation()
        let orientations: [CGImagePropertyOrientation] = [.up, .right, .down, .left]
        guard orientations.indices.contains(reading.rotation) else { throw ScannerError("recognition", "Invalid OCR orientation.") }
        let request = try Self.request(language: language)
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
