import CoreImage
import Foundation
import ScannerAPI

/// Reads only the small top-right symbol. Colour and the central artwork never
/// determine the type. Ambiguous shapes are left unresolved.
enum EnergySymbolDetector {
    struct Shape {
        let mask: [Bool]
        let ratio: Double
        let box: CGRect
    }
    static let templates = EnergySymbolTemplates.values.map { value in
        (name: value.name, ratio: value.ratio, mask: value.rows.flatMap { $0.map { $0 == "#" } })
    }

    static func hasEnergyHeading(_ lines: [RecognizedLine]) -> Bool {
        let headings = lines.filter { $0.confidence >= 0.7 && $0.y > 0.8 && $0.x < 0.7 }
        let title = headings.contains {
            let text = $0.text.uppercased().filter { $0.isLetter }
            return text == "ENERGY" || text == "BASICENERGY" || text == "기본에너지" || text == "에너지"
        }
        // Rules text in the body belongs to Special Energy or a playable Pokémon,
        // neither of which can be identified as a basic Energy from one type icon.
        let body = lines.contains { $0.confidence >= 0.6 && $0.y > 0.12 && $0.y < 0.8 && $0.text.filter(\.isLetter).count > 3 }
        return title && !body
    }

    static func analyze(_ data: Data, reading: CardOCR.Reading) throws -> EnergySymbolEvidence? {
        guard hasEnergyHeading(reading.lines), (0..<4).contains(reading.rotation) else { return nil }
        try Task.checkCancellation()
        let source = try CardOCR.image(data)
        let upright = CIImage(cgImage: source).oriented(forExifOrientation: [1, 6, 3, 8][reading.rotation])
        guard let image = CIContext().createCGImage(upright, from: upright.extent),
              image.width >= 400, (0.60...0.80).contains(Double(image.width) / Double(image.height)) else { return nil }
        var readings: [(type: String, score: Double, box: CGRect)] = []
        for threshold in [45, 70, 95, 120, 145] {
            try Task.checkCancellation()
            let matches = shapes(image, threshold: threshold).compactMap { shape -> (String, Double, CGRect)? in
                guard let result = classify(shape) else { return nil }
                return (result.type, result.score, shape.box)
            }
            // Two plausible different symbols in the corner is not a basic Energy.
            guard Set(matches.map { $0.0 }).count <= 1 else { return nil }
            if let best = matches.max(by: { $0.1 < $1.1 }) { readings.append(best) }
        }
        guard readings.count >= 2, Set(readings.map(\.type)).count == 1,
              let best = readings.max(by: { $0.score < $1.score }),
              readings.allSatisfy({ abs($0.box.midX - best.box.midX) < Double(image.width) * 0.015 &&
                  abs($0.box.midY - best.box.midY) < Double(image.height) * 0.015 }) else { return nil }
        return EnergySymbolEvidence(type: best.type, similarity: readings.map(\.score).min()!,
            region: ImageCrop(x: best.box.minX / Double(image.width), y: best.box.minY / Double(image.height),
                              width: best.box.width / Double(image.width), height: best.box.height / Double(image.height)))
    }

    static func classify(_ shape: Shape) -> (type: String, score: Double)? {
        guard shape.mask.count == 576 else { return nil }
        let ranked = templates.compactMap { template -> (String, Double)? in
            guard abs(log(shape.ratio / template.ratio)) < 0.28 else { return nil }
            var best = 0.0
            // One sample of alignment tolerance handles scan antialiasing; no
            // rotation or reflection can turn an unrelated mark into a match.
            for dy in -1...1 { for dx in -1...1 {
                var intersection = 0, union = 0
                for y in 0..<24 { for x in 0..<24 {
                    let tx = x + dx, ty = y + dy
                    let expected = (0..<24).contains(tx) && (0..<24).contains(ty) && template.mask[ty * 24 + tx]
                    let actual = shape.mask[y * 24 + x]
                    if expected && actual { intersection += 1 }
                    if expected || actual { union += 1 }
                } }
                best = max(best, Double(intersection) / Double(max(1, union)))
            } }
            return (template.name, best)
        }.sorted { $0.1 > $1.1 }
        guard let best = ranked.first, best.1 >= 0.70,
              best.1 - (ranked.dropFirst().first?.1 ?? 0) >= 0.10 else { return nil }
        return best
    }

    static func shapes(_ image: CGImage, threshold: Int) -> [Shape] {
        let area = CGRect(x: Double(image.width) * 0.82, y: 0,
            width: Double(image.width) * 0.175, height: Double(image.height) * 0.18).integral
        guard let crop = image.cropping(to: area), crop.width * crop.height <= 300_000 else { return [] }
        let w = crop.width, h = crop.height
        var rgba = [UInt8](repeating: 255, count: w * h * 4)
        let rendered = rgba.withUnsafeMutableBytes { bytes -> Bool in
            guard let context = CGContext(data: bytes.baseAddress, width: w, height: h,
                bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            context.draw(crop, in: CGRect(x: 0, y: 0, width: w, height: h)); return true
        }
        guard rendered else { return [] }
        let mask = (0..<w*h).map { i in
            let r = Int(rgba[i*4]), g = Int(rgba[i*4+1]), b = Int(rgba[i*4+2])
            return max(r, g, b) < threshold && max(r, g, b) - min(r, g, b) < 65
        }
        // Group separated ink strokes (for example a fist's fingers), without
        // filling their gaps in the shape used for comparison.
        let radius = max(1, Int(ceil(Double(image.width) * 0.0025)))
        var seen = [Bool](repeating: false, count: w * h), result: [Shape] = []
        for start in mask.indices where mask[start] && !seen[start] {
            seen[start] = true
            var points = [start], cursor = 0
            while cursor < points.count {
                let p = points[cursor]; cursor += 1
                for dy in -radius...radius { for dx in -radius...radius {
                    let x = p % w + dx, y = p / w + dy
                    guard x >= 0, x < w, y >= 0, y < h else { continue }
                    let q = y * w + x
                    if mask[q] && !seen[q] { seen[q] = true; points.append(q) }
                } }
            }
            let xs = points.map { $0 % w }, ys = points.map { $0 / w }
            let x = xs.min()!, y = ys.min()!, cw = xs.max()! - x + 1, ch = ys.max()! - y + 1
            guard points.count >= 10, x > 0, y > 0, x + cw < w, y + ch < h,
                  (0.02...0.115).contains(Double(cw) / Double(image.width)),
                  (0.012...0.09).contains(Double(ch) / Double(image.height)) else { continue }
            let ink = Set(points)
            let normalized = (0..<576).map { i -> Bool in
                let px = x + min(cw - 1, Int((Double(i % 24) + 0.5) * Double(cw) / 24))
                let py = y + min(ch - 1, Int((Double(i / 24) + 0.5) * Double(ch) / 24))
                return ink.contains(py * w + px)
            }
            result.append(Shape(mask: normalized, ratio: Double(cw) / Double(ch),
                box: CGRect(x: Double(x) + area.minX, y: Double(y) + area.minY, width: Double(cw), height: Double(ch))))
        }
        return result
    }
}
