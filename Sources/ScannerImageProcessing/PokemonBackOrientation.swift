import CoreGraphics
import Foundation
import ImageIO

/// Conservative orientation cue for the standard international blue Pokémon back.
/// This is neither card identification nor an authenticity/condition assessment.
enum PokemonBackOrientation {
    static func rotation(_ data: Data, words: [String]) throws -> Int? {
        // Logos alone are rotationally ambiguous. Require both logo evidence and
        // the blue border, yellow end logos, and central red/white ball layout.
        let hasLogo = words.contains(where: resemblesLogo)
        guard hasLogo, let source = CGImageSourceCreateWithData(data as CFData, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceThumbnailMaxPixelSize: 256] as CFDictionary) else { return nil }
        let width = image.width, height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        guard let context = CGContext(data: &pixels, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        var matches: [Int] = []
        for turns in 0..<4 {
            let w = turns % 2 == 0 ? width : height, h = turns % 2 == 0 ? height : width
            guard (0.65...0.80).contains(Double(w) / Double(h)) else { continue }
            var border = 0, blue = 0, top = 0, topYellow = 0, bottom = 0, bottomYellow = 0
            var center = 0, red = 0, redY = 0.0, whiteBelow = 0, lower = 0
            for y in 0..<h { for x in 0..<w {
                let sx: Int, sy: Int
                switch turns {
                case 1: sx = y; sy = height - 1 - x
                case 2: sx = width - 1 - x; sy = height - 1 - y
                case 3: sx = width - 1 - y; sy = x
                default: sx = x; sy = y
                }
                let i = (sy * width + sx) * 4
                let r = Double(pixels[i]), g = Double(pixels[i + 1]), b = Double(pixels[i + 2])
                let nx = Double(x) / Double(w), ny = Double(y) / Double(h)
                if nx < 0.045 || nx > 0.955 || ny < 0.035 || ny > 0.965 {
                    border += 1
                    if b > r * 1.3 && b > g * 1.15 && b > 25 { blue += 1 }
                }
                let yellow = r > 120 && g > 90 && b < min(r, g) * 0.65
                if nx > 0.08 && nx < 0.92 && ny > 0.07 && ny < 0.28 { top += 1; if yellow { topYellow += 1 } }
                if nx > 0.08 && nx < 0.92 && ny > 0.72 && ny < 0.93 { bottom += 1; if yellow { bottomYellow += 1 } }
                if nx > 0.25 && nx < 0.75 && ny > 0.30 && ny < 0.70 {
                    center += 1
                    if r > 95 && r > g * 1.4 && r > b * 1.4 { red += 1; redY += ny }
                }
                if nx > 0.30 && nx < 0.65 && ny > 0.53 && ny < 0.66 {
                    lower += 1
                    if min(r, g, b) > 110 && max(r, g, b) - min(r, g, b) < 65 { whiteBelow += 1 }
                }
            } }
            guard Double(blue) / Double(max(1, border)) > 0.65,
                  Double(topYellow) / Double(max(1, top)) > 0.08,
                  Double(bottomYellow) / Double(max(1, bottom)) > 0.08,
                  Double(red) / Double(max(1, center)) > 0.07,
                  (0.34...0.48).contains(redY / Double(max(1, red))),
                  Double(whiteBelow) / Double(max(1, lower)) > 0.35 else { continue }
            matches.append(turns)
        }
        return matches.count == 1 ? matches[0] : nil
    }
    static func resemblesLogo(_ word: String) -> Bool {
        let text = Array(word.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "en_US_POSIX")).filter { $0.isLetter })
        let expected = Array("pokemon")
        guard (6...8).contains(text.count) else { return false }
        var previous = Array(0...expected.count)
        for (row, letter) in text.enumerated() {
            var next = [row + 1]
            for (column, target) in expected.enumerated() {
                next.append(min(previous[column + 1] + 1, next[column] + 1, previous[column] + (letter == target ? 0 : 1)))
            }
            previous = next
        }
        // Stylized logos produce common OCR substitutions. Colour/layout checks
        // remain mandatory; fuzzy text alone never establishes a card back.
        return previous[expected.count] <= 2
    }

}
