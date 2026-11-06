import CoreImage
import Foundation
import ScannerAPI
import Vision

/// Local footer evidence only. No finish inference and no catalogue-trained labels.
enum RaritySymbolDetector {
    enum Shape: String, CaseIterable { case circle, diamond, star }
    typealias Reading = RaritySymbolEvidence
    static func analyze(_ data: Data, rotation: Int, language: CardLanguage) throws -> Reading? {
        try Task.checkCancellation()
        // Other language layouts and metallic/outlined symbols need separate validation.
        guard language == .english, (0..<4).contains(rotation) else { return nil }
        let source = try CardOCR.image(data)
        let upright = CIImage(cgImage: source).oriented(forExifOrientation: [1, 6, 3, 8][rotation])
        guard let image = CIContext().createCGImage(upright, from: upright.extent), image.height >= 1000 else { return nil }
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.recognitionLanguages = ["en-US"]
        request.usesLanguageCorrection = false
        try VNImageRequestHandler(cgImage: image).perform([request])
        let pattern = try NSRegularExpression(pattern: #"(?<![A-Za-z0-9])([0-9]{1,4})\s*/\s*([0-9]{1,4})(?![A-Za-z0-9])"#)
        var anchors: [(CGRect, String, String)] = []
        for observation in request.results ?? [] where observation.boundingBox.minY < 0.15 {
            guard let text = observation.topCandidates(1).first, text.confidence >= 0.7 else { continue }
            for match in pattern.matches(in: text.string, range: NSRange(text.string.startIndex..., in: text.string)) {
                guard let range = Range(match.range, in: text.string), let box = try text.boundingBox(for: range),
                      let local = Range(match.range(at: 1), in: text.string), let total = Range(match.range(at: 2), in: text.string) else { continue }
                anchors.append((box.boundingBox, String(text.string[local]), String(text.string[total])))
            }
        }
        guard anchors.count == 1, let (number, local, total) = anchors.first,
              number.height > 0.003, number.height < 0.025, number.maxX < 0.95 else { return nil }
        // Character-range bounds exclude any symbol Vision merged into the number.
        let rect = CGRect(x: number.maxX + 0.001, y: number.minY - number.height * 0.35,
                          width: min(0.065, 1 - number.maxX - 0.001), height: number.height * 1.7)
        let pixels = CGRect(x: rect.minX * Double(image.width), y: (1 - rect.maxY) * Double(image.height),
                            width: rect.width * Double(image.width), height: rect.height * Double(image.height)).integral
        guard let crop = image.cropping(to: pixels), crop.width * crop.height < 100_000 else { return nil }
        var rgba = [UInt8](repeating: 255, count: crop.width * crop.height * 4)
        let rendered = rgba.withUnsafeMutableBytes { bytes -> Bool in
            guard let context = CGContext(data: bytes.baseAddress, width: crop.width, height: crop.height,
                bitsPerComponent: 8, bytesPerRow: crop.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            context.draw(crop, in: CGRect(x: 0, y: 0, width: crop.width, height: crop.height))
            return true
        }
        try Task.checkCancellation()
        guard rendered else { return nil }
        let light = (0..<crop.width * crop.height).filter { i in
            (0..<3).reduce(0) { $0 + Int(rgba[i * 4 + $1]) } > 270
        }.count
        guard Double(light) / Double(crop.width * crop.height) > 0.55 else { return nil }
        let a = classify(rgba: rgba, width: crop.width, height: crop.height, threshold: 90)
        let b = classify(rgba: rgba, width: crop.width, height: crop.height, threshold: 135)
        guard let a, let b, a.symbol == b.symbol else { return nil }
        return Reading(symbol: a.symbol, similarity: min(a.score, b.score),
            region: ImageCrop(x: rect.minX, y: 1 - rect.maxY, width: rect.width, height: rect.height),
            collectorNumber: local, setTotal: total)
    }

    /// Shape agreement at two ink thresholds; similarity is not a probability.
    static func classify(rgba: [UInt8], width: Int, height: Int, threshold: Int) -> (symbol: String, score: Double)? {
        guard width > 0, height > 0, width * height <= 100_000, rgba.count == width * height * 4 else { return nil }
        let mask = (0..<width * height).map { i in
            let rgb = (0..<3).map { Int(rgba[i * 4 + $0]) }
            return rgb.max()! < threshold && rgb.max()! - rgb.min()! < 60
        }
        var visited = Set<Int>()
        var components: [(x: Int, shape: Shape, score: Double)] = []
        for start in mask.indices where mask[start] && !visited.contains(start) {
            var points = [start]; visited.insert(start); var cursor = 0
            while cursor < points.count {
                let p = points[cursor]; cursor += 1
                for dy in -1...1 { for dx in -1...1 {
                    let x = p % width + dx, y = p / width + dy
                    guard x >= 0, x < width, y >= 0, y < height else { continue }
                    let q = y * width + x
                    if mask[q] && visited.insert(q).inserted { points.append(q) }
                } }
            }
            guard points.count >= 8 else { continue }
            let xs = points.map { $0 % width }, ys = points.map { $0 / width }
            let left = xs.min()!, right = xs.max()!, top = ys.min()!, bottom = ys.max()!
            let w = right - left + 1, h = bottom - top + 1
            // Reject clipped components, digits, footer text and background blocks.
            guard left > 0, top > 0, right < width - 1, bottom < height - 1,
                  h >= 6, Double(h) / Double(height) >= 0.18,
                  (0.75...1.3).contains(Double(w) / Double(h)) else { continue }
            let ink = Set(points)
            var scores: [(Shape, Double)] = []
            for shape in Shape.allCases {
                var intersection = 0, union = 0
                for y in 0..<h { for x in 0..<w {
                    let expected = template(shape, x: (Double(x) + 0.5) / Double(w), y: (Double(y) + 0.5) / Double(h))
                    let actual = ink.contains((top + y) * width + left + x)
                    if expected && actual { intersection += 1 }
                    if expected || actual { union += 1 }
                } }
                scores.append((shape, Double(intersection) / Double(max(1, union))))
            }
            scores.sort { $0.1 > $1.1 }
            guard scores[0].1 >= 0.82, scores[0].1 - scores[1].1 >= 0.08 else { continue }
            components.append((left, scores[0].0, scores[0].1))
        }
        components.sort { $0.x < $1.x }
        // Require the mark beside the fraction, not an unrelated mark further right.
        guard let first = components.first, first.x < height else { return nil }
        if components.count == 1 { return (first.shape.rawValue, first.score) }
        if components.count == 2, components.allSatisfy({ $0.shape == .star }),
           components[1].x - first.x < Int(Double(height) * 1.5) {
            return ("doubleStar", components.map(\.score).min()!)
        }
        return nil
    }
    static func template(_ shape: Shape, x: Double, y: Double) -> Bool {
        switch shape {
        case .circle: return pow(x - 0.5, 2) + pow(y - 0.5, 2) <= 0.25
        case .diamond: return abs(x - 0.5) + abs(y - 0.5) <= 0.5
        case .star:
            let vertices = (0..<10).map { i -> (Double, Double) in
                let a = Double(i) * .pi / 5 - .pi / 2, r = i % 2 == 0 ? 0.5 : 0.28
                return (0.5 + cos(a) * r, 0.5 + sin(a) * r)
            }
            let minX = vertices.map { $0.0 }.min()!, maxX = vertices.map { $0.0 }.max()!
            let minY = vertices.map { $0.1 }.min()!, maxY = vertices.map { $0.1 }.max()!
            let normalized = vertices.map { (($0.0 - minX) / (maxX - minX), ($0.1 - minY) / (maxY - minY)) }
            var inside = false
            for i in normalized.indices {
                let a = normalized[i], b = normalized[(i + 1) % normalized.count]
                if (a.1 > y) != (b.1 > y), x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 { inside.toggle() }
            }
            return inside
        }
    }
}
