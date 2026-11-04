import CoreGraphics
import Foundation
import ImageIO
import ScannerAPI
import Vision

/// Conservative bounding crop for individual cards against the selected scanner background.
/// Connected foreground includes the card border, unlike an artwork-only rectangle detector.
public enum CardProcessor {
    public static func analyze(_ data: Data, background: ScanBackground = .black, language: CardLanguage = .english) throws -> CardPreparation {
        guard data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let fullWidth = properties[kCGImagePropertyPixelWidth] as? Int,
              let fullHeight = properties[kCGImagePropertyPixelHeight] as? Int,
              fullWidth > 0, fullHeight > 0, fullWidth <= 20000, fullHeight <= 20000, fullWidth * fullHeight <= 48_000_000,
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 1200] as CFDictionary) else {
            throw ScannerError("image", "Card preparation requires one image up to 32 MiB and 48 megapixels.")
        }
        let width = image.width, height = image.height
        var rgba = [UInt8](repeating: 0, count: width * height * 4)
        guard let context = CGContext(data: &rgba, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue) else {
            throw ScannerError("image", "Cannot inspect card boundaries.")
        }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let boundary = try foregroundBoundary(rgba: rgba, width: width, height: height, background: background)
        let geometry = CardDeskew.analyze(points: boundary.points, width: width, height: height)
        let crop = geometry.crop
        let request = ImageEditRequest(jobID: UUID(), sequence: 1, crop: crop, deskewDegrees: geometry.degrees)
        let cropped = try ImageProcessor.render(data, request: request)
        let orientation = try uprightRotation(cropped.data, language: language)
        var warnings = orientation.verified ? [] : ["Orientation is uncertain (unrecognized or ambiguous card back). Inspect the prepared image and rotate manually if needed."]
        if boundary.touchesEdge {
            warnings.append("Card is close to a scan boundary. Check that the original captured the entire edge; cropping cannot restore missing pixels.")
        }
        return CardPreparation(crop: crop, quarterTurns: orientation.turns, orientationVerified: orientation.verified, warnings: warnings + geometry.warnings, deskewDegrees: geometry.degrees, orientationBasis: orientation.basis,
            sourceCrop: CardDeskew.bounds(points: boundary.points, width: width, height: height, degrees: 0))
    }

    struct Component { var count: Int; var minX: Int; var maxX: Int; var minY: Int; var maxY: Int }
    static func foregroundCrop(rgba: [UInt8], width: Int, height: Int, background: ScanBackground = .black) throws -> ImageCrop {
        let boundary = try foregroundBoundary(rgba: rgba, width: width, height: height, background: background)
        return CardDeskew.bounds(points: boundary.points, width: width, height: height, degrees: 0)
    }

    private static func foregroundBoundary(rgba: [UInt8], width: Int, height: Int, background: ScanBackground) throws -> (points: [CGPoint], touchesEdge: Bool) {
        var bestPixels: [Int] = []
        var mask = [UInt8](repeating: 0, count: width * height)
        let backing: [Double] = background == .white ? (0..<height).map { y in
            let margin = max(1, width / 10)
            let values = (Array(0..<margin) + Array((width - margin)..<width)).map { x in
                let i = (y * width + x) * 4
                return Double(Int(rgba[i]) + Int(rgba[i + 1]) + Int(rgba[i + 2])) / 3
            }.sorted()
            return values[values.count / 2]
        } : []
        for pixel in mask.indices {
            let i = pixel * 4
            let red = Int(rgba[i]), green = Int(rgba[i + 1]), blue = Int(rgba[i + 2])
            let maximum = max(red, green, blue), minimum = min(red, green, blue)
            if background == .black {
                if maximum > 50 && red + green + blue > 70 { mask[pixel] = 1 }
            } else {
                // Estimate backing brightness per row from both outside margins.
                // This follows the real driver's grey backing / white padding transition.
                let reference = backing[pixel / width]
                let brightness = Double(red + green + blue) / 3
                if reference - brightness > 45 || maximum - minimum > 45 { mask[pixel] = 1 }
            }
        }
        var components: [Component] = []
        var queue: [Int] = []; queue.reserveCapacity(width * height / 4)
        for start in mask.indices where mask[start] == 1 {
            queue.removeAll(keepingCapacity: true); queue.append(start); mask[start] = 0
            var c = Component(count: 0, minX: width, maxX: 0, minY: height, maxY: 0)
            var head = 0
            while head < queue.count {
                let p = queue[head]; head += 1
                let x = p % width, y = p / width
                c.count += 1; c.minX = min(c.minX, x); c.maxX = max(c.maxX, x); c.minY = min(c.minY, y); c.maxY = max(c.maxY, y)
                for dy in -1...1 { for dx in -1...1 {
                    let nx = x + dx, ny = y + dy
                    guard nx >= 0, nx < width, ny >= 0, ny < height else { continue }
                    let n = ny * width + nx
                    if mask[n] == 1 { mask[n] = 0; queue.append(n) }
                } }
            }
            if c.count > width * height / 500 { components.append(c) }
            if c.count > bestPixels.count { bestPixels = queue }
        }
        guard let card = components.max(by: { $0.count < $1.count }) else { throw ScannerError("cardNotFound", "No clear card boundary found against the selected background. Original retained.") }
        let w = Double(card.maxX - card.minX + 1), h = Double(card.maxY - card.minY + 1)
        let ratio = min(w, h) / max(w, h)
        guard (0.60...0.83).contains(ratio), w * h >= Double(width * height) * 0.015, w * h < Double(width * height) * 0.85 else {
            throw ScannerError("cardNotFound", "Detected shape does not have a supported card aspect ratio. Use manual cropping; original retained.")
        }
        for other in components where other.count > card.count / 5 {
            if other.maxX < card.minX || other.minX > card.maxX || other.maxY < card.minY || other.minY > card.maxY {
                throw ScannerError("cardNotFound", "More than one substantial object found. Crop manually to avoid losing a card.")
            }
        }
        // Row endpoints enclose the component; column endpoints also sample the
        // long top/bottom edges when a card was fed sideways for fine deskew.
        var rows: [Int: (Int, Int)] = [:]
        var columns: [Int: (Int, Int)] = [:]
        for pixel in bestPixels {
            let x = pixel % width, y = pixel / width
            let previous = rows[y] ?? (x, x); rows[y] = (min(previous.0, x), max(previous.1, x))
            let column = columns[x] ?? (y, y); columns[x] = (min(column.0, y), max(column.1, y))
        }
        var points = rows.flatMap { y, range in [CGPoint(x: range.0, y: y), CGPoint(x: range.1 + 1, y: y), CGPoint(x: range.0, y: y + 1), CGPoint(x: range.1 + 1, y: y + 1)] }
        points += columns.flatMap { x, range in [CGPoint(x: x, y: range.0), CGPoint(x: x, y: range.1 + 1), CGPoint(x: x + 1, y: range.0), CGPoint(x: x + 1, y: range.1 + 1)] }
        return (points, card.minX == 0 || card.minY == 0 || card.maxX == width - 1 || card.maxY == height - 1)
    }

    static func uprightRotation(_ data: Data, language: CardLanguage = .english) throws -> (turns: Int, verified: Bool, basis: String) {
        if let turns = try YugiohBackOrientation.rotation(data) { return (turns, true, "yugioh-back-pattern") }
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        let supported = try request.supportedRecognitionLanguages()
        guard language.recognitionLanguages.allSatisfy(supported.contains) else {
            throw ScannerError("languageUnsupported", "This Mac cannot read the selected card language for automatic orientation.")
        }
        request.recognitionLanguages = language.recognitionLanguages; request.usesLanguageCorrection = false
        try VNImageRequestHandler(data: data).perform([request])
        var votes = [Double](repeating: 0, count: 4)
        var lines = [Int](repeating: 0, count: 4)
        for observation in request.results ?? [] {
            guard let candidate = observation.topCandidates(1).first, candidate.confidence >= 0.5,
                  candidate.string.count >= 4 else { continue }
            let dx = observation.topRight.x - observation.topLeft.x, dy = observation.topRight.y - observation.topLeft.y
            let angle = atan2(dy, dx)
            let turns = (Int((angle / (.pi / 2)).rounded()) + 4) % 4
            votes[turns] += Double(candidate.confidence) * Double(min(candidate.string.count, 24)); lines[turns] += 1
        }
        let best = (0..<4).max { votes[$0] < votes[$1] }!
        let total = votes.reduce(0, +)
        if lines[best] >= 4 && votes[best] >= 20 && votes[best] >= total * 0.8 { return (best, true, "text") }
        let words = (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
        if let turns = try PokemonBackOrientation.rotation(data, words: words) {
            return (turns, true, "pokemon-back-pattern")
        }
        let source = CGImageSourceCreateWithData(data as CFData, nil)
        let properties = source.flatMap { CGImageSourceCopyPropertiesAtIndex($0, 0, nil) as? [CFString: Any] }
        let width = properties?[kCGImagePropertyPixelWidth] as? Int ?? 0
        let height = properties?[kCGImagePropertyPixelHeight] as? Int ?? 0
        return (width > height ? 1 : 0, false, "unknown")
    }
}
