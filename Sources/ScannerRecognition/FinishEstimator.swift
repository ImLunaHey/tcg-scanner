import CoreImage
import Foundation
import ImageIO
import ScannerAPI

/// Local printed-variant inference for ordinary modern English cards. It does
/// not measure reflectivity or confirm finish, and never writes a reviewed field.
public enum FinishEstimator {
    public static func estimate(_ data: Data, evidence: RecognitionEvidence) -> FinishEstimate {
        do { return try analyze(data, evidence: evidence) }
        catch { return FinishEstimate(state: .uncertain, candidateID: evidence.candidates.count == 1 ? evidence.candidates.first?.id : nil,
            evidence: ["Local finish analysis could not read the set-code box."], limitations: ["Inspect the physical card; no finish has been confirmed."]) }
    }

    public static func analyze(_ data: Data, evidence: RecognitionEvidence) throws -> FinishEstimate {
        try Task.checkCancellation()
        let limitations = ["Experimental estimate: inspect the physical card before confirming finish.",
            "Uses printed set-code contrast and catalogue variants, not measured reflectivity. Special/promotional printings may differ."]
        guard evidence.candidates.count == 1, let card = evidence.candidates.first,
              card.evidence.contains(where: { $0.hasPrefix("Collector number ") }) else {
            return FinishEstimate(state: .uncertain, evidence: ["A single printing with a readable collector number is required."], limitations: limitations)
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
            return FinishEstimate(state: .uncertain, candidateID: card.id, evidence: ["The English set-code box could not be located unambiguously."], limitations: limitations)
        }
        // Include the narrow padding inside the printed box, so dense white glyphs
        // do not turn a black-background code into a mid-gray average.
        let region = ImageCrop(x: marker.x - 0.002, y: 1 - marker.y - marker.height - 0.0015,
                               width: marker.width + 0.004, height: marker.height + 0.003)
        let patch = try pixels(data, region: region, turns: evidence.orientation)
        let luminance = patch.values
        let median = luminance.sorted()[luminance.count / 2]
        let dark = Double(luminance.filter { $0 < 0.30 }.count) / Double(luminance.count)
        let light = Double(luminance.filter { $0 > 0.70 }.count) / Double(luminance.count)
        let background = MarkerBackground.measure(luminance, width: patch.width, height: patch.height)
        var result = decision(card: card, median: median, dark: dark, light: light, background: background)
        result.limitations = limitations; result.region = region; result.sourceQuarterTurns = evidence.orientation
        result.medianLuminance = median
        result.darkBackgroundAreaFraction = background.darkArea
        result.lightBackgroundAreaFraction = background.lightArea
        if enlargedReading { result.evidence.append("Set-code box located by re-reading enlarged pixels locally.") }
        return result
    }

    static func decision(card: CardCandidate, median: Double, dark: Double, light: Double,
                         background: MarkerBackground? = nil) -> FinishEstimate {
        var result = FinishEstimate(state: .uncertain, candidateID: card.id, evidence: ["Set-code contrast is inconclusive."])
        guard dark > 0.08 && light > 0.08 else { return result }
        let overall: String? = median > 0.65 && light > 0.40 ? "light" :
            median < 0.45 && dark > 0.30 ? "dark" : nil
        if let overall, let spatial = background?.polarity, overall != spatial {
            result.evidence = ["Set-code background and overall contrast disagree."]
            return result
        }
        let polarity = background?.polarity ?? overall
        if polarity == "light" {
            result.state = .estimated; result.suggestedFinish = .reverseHolo; result.markerPolarity = "light"
            result.evidence = ["Light set-code box with dark lettering suggests a reverse-holo printing."]
        } else if polarity == "dark" {
            result.markerPolarity = "dark"
            let regular = card.availableFinishes.filter { $0 == .normal || $0 == .holo }
            if Set(regular).count == 1, let finish = regular.first {
                result.state = .estimated; result.suggestedFinish = finish
                result.evidence = ["Dark set-code box suggests the regular printing.", "The catalogue lists one regular finish for this printing."]
            } else {
                result.evidence = ["Dark set-code box suggests the regular printing, but it does not distinguish holo from non-holo."]
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
