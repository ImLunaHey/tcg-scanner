import CoreImage
import Foundation
import ImageIO
import ScannerAPI

/// Local printed-variant inference for supported modern English cards. It does
/// not measure reflectivity or confirm finish, and never writes a reviewed field.
public enum FinishEstimator {
    public static func estimate(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation? = nil) -> FinishEstimate {
        do { return try analyze(data, evidence: evidence, preparation: preparation) }
        catch { return FinishEstimate(state: .uncertain, candidateID: evidence.candidates.count == 1 ? evidence.candidates.first?.id : nil,
            evidence: ["Local finish analysis could not read the image evidence."], limitations: ["Inspect the physical card; no finish has been confirmed."], uncertaintyReason: .analysisFailed) }
    }

    public static func analyze(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation? = nil) throws -> FinishEstimate {
        let marker = try analyzeMarker(data, evidence: evidence)
        // Original scanner frames and unverified/backs must never supply border evidence.
        guard preparation?.orientationVerified == true, preparation?.orientationBasis == "text",
              evidence.candidates.count == 1, let card = evidence.candidates.first,
              card.category == "Pokemon", marker.markerPolarity == "dark",
              marker.state == .estimated || marker.uncertaintyReason == .multipleRegularFinishes else { return marker }
        let border: BorderFinishEvidence
        do {
            let measured = try BorderFoilAnalyzer.analyze(data, orientation: evidence.orientation)
            border = BorderFinishEvidence(state: BorderFinishEvidence.State(rawValue: measured.state.rawValue)!,
                threshold: measured.threshold, baseline: measured.baseline, inset: measured.inset, downsampled: measured.downsampled)
        } catch is CancellationError { throw CancellationError() }
        catch {
            border = BorderFinishEvidence(state: .failed, threshold: BorderFoilMeasurement.calibratedThreshold)
        }
        return combine(marker, card: card, border: border)
    }

    /// Reverse-holo markers remain independent; absence of border texture never means normal.
    static func combine(_ marker: FinishEstimate, card: CardCandidate, border: BorderFinishEvidence) -> FinishEstimate {
        guard marker.candidateID == card.id, marker.markerPolarity == "dark",
              marker.state == .estimated || marker.uncertaintyReason == .multipleRegularFinishes else { return marker }
        var result = marker
        result.borderEvidence = border
        result.limitations.append("Border texture was calibrated on a small set of modern cards. Printed patterns can resemble foil; absent texture does not establish non-holo or rule out reverse holo.")
        switch border.state {
        case .present:
            if card.availableFinishes.contains(.holo) {
                result.state = .estimated; result.suggestedFinish = .holo; result.uncertaintyReason = nil
                result.evidence.append("Independent brightness texture on both borders exceeds the fixed foil threshold in all three measurement variants; the dark set code and catalogue support holo.")
            } else {
                result.state = .uncertain; result.suggestedFinish = nil; result.uncertaintyReason = .conflictingFinishEvidence
                result.evidence.append("Border texture suggests foil but the catalogue does not list a regular holo printing. The conflict needs review.")
            }
        case .absent:
            result.evidence.append("No stable border foil signal was found; this does not prove non-holo. The printed-marker/catalogue inference is retained.")
        case .unstable:
            result.evidence.append("Border measurements disagree across strip widths or resolution. They cannot resolve the finish; the printed-marker/catalogue inference is retained.")
        case .unsupported:
            result.evidence.append("The prepared image is outside the border check's size or shape limits; only printed-marker/catalogue evidence was used.")
        case .failed:
            result.evidence.append("Border analysis was unavailable; only printed-marker/catalogue evidence was used.")
        }
        return result
    }

    private static func analyzeMarker(_ data: Data, evidence: RecognitionEvidence) throws -> FinishEstimate {
        try Task.checkCancellation()
        let limitations = ["Experimental estimate: inspect the physical card before confirming finish.",
            "Uses printed set-code contrast, catalogue variants and eligible prepared-card border texture, not measured reflectivity. Special/promotional printings may differ."]
        guard evidence.candidates.count == 1, let card = evidence.candidates.first,
              card.evidence.contains(where: { $0.hasPrefix("Collector number ") }) else {
            return FinishEstimate(state: .uncertain, evidence: ["A single printing with a readable collector number is required."], limitations: limitations, uncertaintyReason: .ambiguousIdentity)
        }
        if let catalogue = modernExCatalogueEstimate(card) { return catalogue }
        if card.setID == "sve" {
            return FinishEstimate(state: .unsupported, candidateID: card.id,
                evidence: ["Basic Energy foil variants need validation. A dark SVE set-code box does not yet establish non-holo."],
                limitations: ["The catalogue groups several special Energy foil treatments under broad variant flags. A missing holo flag does not prove a non-holo card.",
                    "Compare known normal and foil examples of the same SVE collector number before enabling automatic Energy finish estimates."])
        }
        guard card.setID.range(of: #"^(sv|me)[0-9]{2}(\.[0-9][a-z]?)?$"#, options: .regularExpression) != nil,
              let total = card.officialSetCount, let number = Int(card.localID), number <= total,
              card.availableFinishes.contains(.reverseHolo) else {
            return FinishEstimate(state: .unsupported, candidateID: card.id, evidence: ["This printing is outside the supported modern set-code layout."], limitations: limitations)
        }
        func locate(_ lines: [RecognizedLine]) -> [RecognizedLine] { lines.filter { line in
            let code = line.text.uppercased().filter { !$0.isWhitespace }
            return line.confidence >= 0.5 && line.x >= 0.04 && line.x <= 0.18 && line.y >= 0.015 && line.y < 0.08 &&
                (0.03...0.13).contains(line.width) && (0.008...0.035).contains(line.height) &&
                code.range(of: #"^[A-Z0-9]{3}EN$"#, options: .regularExpression) != nil
        }
        }
        var markers = locate(evidence.lines)
        if markers.isEmpty { markers = locate(try CardOCR.englishSetCode(data, rotation: evidence.orientation)) }
        var enlargedReading = false
        if markers.isEmpty {
            try Task.checkCancellation()
            markers = locate(try CardOCR.englishSetCode(data, rotation: evidence.orientation, magnify: true))
            enlargedReading = markers.count == 1
        }
        guard markers.count == 1, let marker = markers.first else {
            return FinishEstimate(state: .uncertain, candidateID: card.id, evidence: ["The English set-code box could not be located unambiguously."], limitations: limitations, uncertaintyReason: .markerNotFound)
        }
        var result = try measure(data, marker: marker, card: card, orientation: evidence.orientation, limitations: limitations)
        if enlargedReading { result.evidence.append("Set-code box located by re-reading enlarged pixels locally.") }
        // A recognized word can still have an oversized Vision character box that
        // includes the neighbouring regulation symbol. Retry only unresolved contrast,
        // never conflicting evidence or a catalogue ambiguity. Require the same code
        // and a substantially tighter, contained rectangle before accepting a retry.
        if result.uncertaintyReason == .markerContrast && !enlargedReading {
            try Task.checkCancellation()
            let refined = locate(try CardOCR.englishSetCode(data, rotation: evidence.orientation, magnify: true))
            if refined.count == 1, let tighter = refined.first, isTighterRead(tighter, than: marker) {
                var retry = try measure(data, marker: tighter, card: card, orientation: evidence.orientation, limitations: limitations)
                if retry.markerPolarity != nil {
                    retry.evidence.append("An enlarged local OCR pass isolated the same set code in a tighter box, excluding neighbouring print.")
                    result = retry
                }
            }
        }
        return result
    }

    /// Modern English Pokémon ex have documented foil printings. Infer only when
    /// the exact catalogue printing lists holo alone; never generalise to older
    /// ex, promos, or mixed variants. Category adds no identity evidence here:
    /// recognition already uses the Pokémon catalogue and the exact SV namespace.
    private static func modernExCatalogueEstimate(_ card: CardCandidate) -> FinishEstimate? {
        guard card.setID.range(of: #"^sv[0-9]{2}(\.[0-9][a-z]?)?$"#, options: .regularExpression) != nil,
              card.id == card.setID + "-" + card.localID,
              card.localID.range(of: #"^[0-9]+$"#, options: .regularExpression) != nil,
              let number = Int(card.localID), number > 0,
              let total = card.officialSetCount, total > 0,
              card.evidence.contains("Collector number " + card.localID),
              card.name.hasSuffix(" ex"), Set(card.availableFinishes) == [.holo] else { return nil }
        var result = FinishEstimate(state: .estimated, suggestedFinish: .holo, candidateID: card.id,
            evidence: ["The exact Scarlet & Violet Pokémon ex printing \(card.id) lists only holo in the catalogue.",
                "Modern Pokémon ex foil printing rules support this catalogue-based estimate, including collector numbers above the ordinary set total."],
            limitations: ["Catalogue-based inference, not measured reflectivity. Incorrect identification, missing catalogue variants or special printings can invalidate it.",
                "Inspect the physical card before confirming finish; this does not confirm authenticity or a particular foil texture."])
        result.method = "modern-ex-catalogue-v1"
        return result
    }

    static func isTighterRead(_ refined: RecognizedLine, than original: RecognizedLine) -> Bool {
        func code(_ line: RecognizedLine) -> String { line.text.uppercased().filter { !$0.isWhitespace } }
        let areaRatio = refined.width * refined.height / (original.width * original.height)
        return code(refined) == code(original) && (0.3...0.8).contains(areaRatio) &&
            refined.x >= original.x - 0.004 && refined.y >= original.y - 0.004 &&
            refined.x + refined.width <= original.x + original.width + 0.004 &&
            refined.y + refined.height <= original.y + original.height + 0.004
    }

    private static func measure(_ data: Data, marker: RecognizedLine, card: CardCandidate,
                                orientation: Int, limitations: [String]) throws -> FinishEstimate {
        // Include the narrow padding inside the printed box, so dense white glyphs
        // do not turn a black-background code into a mid-gray average.
        let region = ImageCrop(x: marker.x - 0.002, y: 1 - marker.y - marker.height - 0.0015,
                               width: marker.width + 0.004, height: marker.height + 0.003)
        let patch = try pixels(data, region: region, turns: orientation)
        let luminance = patch.values
        let median = luminance.sorted()[luminance.count / 2]
        let dark = Double(luminance.filter { $0 < 0.30 }.count) / Double(luminance.count)
        let light = Double(luminance.filter { $0 > 0.70 }.count) / Double(luminance.count)
        let background = MarkerBackground.measure(luminance, width: patch.width, height: patch.height)
        var result = decision(card: card, median: median, dark: dark, light: light, background: background)
        result.limitations = limitations; result.region = region; result.sourceQuarterTurns = orientation
        result.medianLuminance = median
        result.darkBackgroundAreaFraction = background.darkArea
        result.lightBackgroundAreaFraction = background.lightArea
        return result
    }

    static func decision(card: CardCandidate, median: Double, dark: Double, light: Double,
                         background: MarkerBackground? = nil) -> FinishEstimate {
        var result = FinishEstimate(state: .uncertain, candidateID: card.id, evidence: ["Set-code contrast is inconclusive."], uncertaintyReason: .markerContrast)
        guard dark > 0.08 && light > 0.08 else { return result }
        let overall: String? = median > 0.65 && light > 0.40 ? "light" :
            median < 0.45 && dark > 0.30 ? "dark" : nil
        if let overall, let spatial = background?.polarity, overall != spatial {
            result.evidence = ["Set-code background and overall contrast disagree."]
            result.uncertaintyReason = .conflictingContrast
            return result
        }
        let polarity = background?.polarity ?? overall
        if polarity == "light" {
            result.uncertaintyReason = nil; result.state = .estimated; result.suggestedFinish = .reverseHolo; result.markerPolarity = "light"
            result.evidence = ["Light set-code box with dark lettering suggests a reverse-holo printing."]
        } else if polarity == "dark" {
            result.markerPolarity = "dark"
            let regular = card.availableFinishes.filter { $0 == .normal || $0 == .holo }
            if Set(regular).count == 1, let finish = regular.first {
                result.uncertaintyReason = nil; result.state = .estimated; result.suggestedFinish = finish
                result.evidence = ["Dark set-code box suggests the regular printing.", "The catalogue lists one regular finish for this printing."]
            } else if regular.isEmpty {
                result.evidence = ["Dark set-code box suggests the regular printing, but the catalogue does not list its finish."]
                result.uncertaintyReason = .missingRegularFinish
            } else {
                result.evidence = ["Dark set-code box suggests the regular printing, but it does not distinguish holo from non-holo."]
                result.uncertaintyReason = .multipleRegularFinishes
            }
        }
        if background?.polarity != nil {
            result.evidence.append("Broad clear background patches were measured separately from the set-code lettering.")
        }
        return result
    }

    private static func pixels(_ data: Data, region: ImageCrop, turns: Int) throws -> (values: [Double], width: Int, height: Int) {
        guard data.count <= 32 * 1024 * 1024, (0..<4).contains(turns),
              let source = CGImageSourceCreateWithData(data as CFData, nil), CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 20000, height <= 20000, width * height <= 40_000_000,
              var image = CIImage(data: data, options: [.applyOrientationProperty: true]) else {
            throw ScannerError("resourceLimit", "Finish analysis requires one image up to 32 MiB and 40 megapixels.")
        }
        for _ in 0..<turns { image = image.oriented(.right) }
        let e = image.extent
        let rect = CGRect(x: e.minX + region.x * e.width, y: e.minY + (1 - region.y - region.height) * e.height,
                          width: region.width * e.width, height: region.height * e.height).integral.intersection(e)
        guard rect.width >= 12, rect.height >= 6, rect.width * rect.height <= 100_000 else {
            throw ScannerError("image", "Set-code box is too small or exceeds analysis limits.")
        }
        let w = Int(rect.width), h = Int(rect.height)
        var rgba = [UInt8](repeating: 0, count: w * h * 4)
        CIContext(options: [.cacheIntermediates: false]).render(image, toBitmap: &rgba, rowBytes: w * 4, bounds: rect, format: .RGBA8, colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!)
        var values: [Double] = []; values.reserveCapacity(w * h)
        for i in 0..<(w * h) {
            let red = 0.2126 * Double(rgba[i * 4])
            let green = 0.7152 * Double(rgba[i * 4 + 1])
            let blue = 0.0722 * Double(rgba[i * 4 + 2])
            values.append((red + green + blue) / 255)
        }
        return (values, w, h)
    }
}
