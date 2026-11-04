import CoreGraphics
import Foundation
import ImageIO
import Vision

/// Standard English TCG back: dark vortex, warm surround, and two spatially
/// separated printed brands. Brown colour or KONAMI text alone is insufficient.
enum YugiohBackOrientation {
    static func rotation(_ data: Data) throws -> Int? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceThumbnailMaxPixelSize: 256] as CFDictionary),
              layoutMatches(image) else { return nil }
        let orientations: [CGImagePropertyOrientation] = [.up, .right, .down, .left]
        var matches: [Int] = []
        for turns in (image.width < image.height ? [0, 2] : [1, 3]) {
            try Task.checkCancellation()
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.recognitionLanguages = ["en-US"]; request.usesLanguageCorrection = false
            try VNImageRequestHandler(data: data, orientation: orientations[turns]).perform([request])
            let observations = request.results ?? []
            let konami = observations.contains { observation in
                guard let text = observation.topCandidates(1).first, text.confidence >= 0.4 else { return false }
                return normalized(text.string).contains("konami") && observation.boundingBox.midY > 0.75 && observation.boundingBox.midX < 0.55
            }
            let logo = observations.contains { observation in
                guard let text = observation.topCandidates(1).first, text.confidence >= 0.3 else { return false }
                let value = normalized(text.string)
                return (resemblesTradingLogo(value) || value.contains("yugioh")) && observation.boundingBox.midY < 0.25
            }
            if konami && logo { matches.append(turns) }
        }
        return matches.count == 1 ? matches[0] : nil
    }

    static func resemblesTradingLogo(_ value: String) -> Bool {
        let text = Array(normalized(value)), expected = Array("tradingcardgame")
        guard abs(text.count - expected.count) <= 1 else { return false }
        var previous = Array(0...expected.count)
        for (row, letter) in text.enumerated() {
            var next = [row + 1]
            for (column, target) in expected.enumerated() {
                next.append(min(previous[column + 1] + 1, next[column] + 1,
                                previous[column] + (letter == target ? 0 : 1)))
            }
            previous = next
        }
        return previous[expected.count] <= 1
    }

    private static func normalized(_ value: String) -> String {
        value.lowercased().filter { $0.isLetter }
    }

    static func layoutMatches(_ image: CGImage) -> Bool {
        let width = image.width, height = image.height
        guard (0.63...0.77).contains(Double(min(width, height)) / Double(max(width, height))) else { return false }
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        guard let context = CGContext(data: &pixels, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        var center = 0, dark = 0, ring = 0, warm = 0
        for y in 0..<height { for x in 0..<width {
            let nx = Double(x) / Double(width), ny = Double(y) / Double(height)
            let i = (y * width + x) * 4
            let r = Double(pixels[i]), g = Double(pixels[i + 1]), b = Double(pixels[i + 2])
            if abs(nx - 0.5) < 0.12 && abs(ny - 0.5) < 0.12 {
                center += 1; if max(r, g, b) < 70 { dark += 1 }
            } else if nx > 0.035 && nx < 0.965 && ny > 0.035 && ny < 0.965 {
                ring += 1
                if r > 65 && r > g * 1.18 && g > b * 1.15 { warm += 1 }
            }
        } }
        return Double(dark) / Double(max(1, center)) > 0.65 && Double(warm) / Double(max(1, ring)) > 0.15
    }
}
