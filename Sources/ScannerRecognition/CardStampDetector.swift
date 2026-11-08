import CoreGraphics
import Foundation
import ImageIO
import ScannerAPI

/// Detects the orange Pikachu-pumpkin mark, not its release year or finish.
/// Callers must establish an upright, prepared card front before using this cue.
public enum CardStampDetector {
    public static func analyze(_ data: Data, orientation: Int) throws -> CardStampEvidence {
        try Task.checkCancellation()
        guard (0..<4).contains(orientation) else {
            throw ScannerError("invalidParams", "Stamp analysis requires a valid quarter-turn orientation.")
        }
        guard data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, nil), CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 20_000, height <= 20_000, width * height <= 40_000_000 else {
            throw ScannerError("resourceLimit", "Stamp analysis requires one image up to 32 MiB and 40 megapixels.")
        }
        let full = try raster(source, maximumSize: 1260).rotated(orientation)
        guard full.height >= 600, (0.65...0.80).contains(Double(full.width) / Double(full.height)) else {
            return CardStampEvidence(state: .unsupported, sourceQuarterTurns: orientation,
                evidence: ["Pumpkin-mark analysis requires a prepared portrait card at a readable resolution."])
        }
        let baseline = try full.bestMark()
        let smaller = try raster(source, maximumSize: 630).rotated(orientation)
        let downsampled = try smaller.bestMark()
        let a = baseline?.similarity ?? 0, b = downsampled?.similarity ?? 0
        let sameMark = baseline.map { first in downsampled.map { first.overlaps($0) } ?? false } ?? false
        let present = a >= 0.88 && b >= 0.88 && sameMark && baseline?.darkFace == true && downsampled?.darkFace == true
        let state: CardStampEvidence.State = present ? .present : max(a, b) >= 0.75 ? .uncertain : .absent
        let message: String
        switch state {
        case .present:
            message = "Orange pumpkin silhouette, paired ears and dark eye/mouth cutouts agree at two resolutions in the expected illustration-edge region."
        case .uncertain:
            message = "A mark resembles the pumpkin shape, but its dark facial details or measurements do not agree strongly enough."
        default:
            message = "No matching orange Pikachu-pumpkin mark was found. This does not rule out other stamp designs."
        }
        return CardStampEvidence(state: state, region: state == .absent ? nil : baseline?.region ?? downsampled?.region,
            sourceQuarterTurns: orientation, similarity: min(a, b), baselineSimilarity: a, downsampledSimilarity: b,
            evidence: [message, "Shape similarity is not a probability, edition year, authenticity check or finish measurement."])
    }

    static func raster(_ source: CGImageSource, maximumSize: Int) throws -> Raster {
        try Task.checkCancellation()
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maximumSize
        ] as CFDictionary), let space = CGColorSpace(name: CGColorSpace.sRGB) else {
            throw ScannerError("image", "Could not decode the prepared card for stamp analysis.")
        }
        var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
        try pixels.withUnsafeMutableBytes { bytes in
            guard let context = CGContext(data: bytes.baseAddress, width: image.width, height: image.height,
                bitsPerComponent: 8, bytesPerRow: image.width * 4, space: space,
                bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue) else {
                throw ScannerError("image", "Could not render the prepared card for stamp analysis.")
            }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        return Raster(width: image.width, height: image.height, pixels: pixels)
    }

    struct Mark {
        let region: ImageCrop
        let similarity: Double
        let darkFace: Bool
        func overlaps(_ other: Self) -> Bool {
            let x = max(region.x, other.region.x), y = max(region.y, other.region.y)
            let right = min(region.x + region.width, other.region.x + other.region.width)
            let bottom = min(region.y + region.height, other.region.y + other.region.height)
            let intersection = max(0, right - x) * max(0, bottom - y)
            let union = region.width * region.height + other.region.width * other.region.height - intersection
            return union > 0 && intersection / union >= 0.8
        }
    }

    struct Raster {
        let width: Int, height: Int
        let pixels: [UInt8]

        func rotated(_ turns: Int) throws -> Self {
            var current = self
            for _ in 0..<turns {
                try Task.checkCancellation()
                var output = [UInt8](repeating: 0, count: current.pixels.count)
                for y in 0..<current.height {
                    if y % 64 == 0 { try Task.checkCancellation() }
                    for x in 0..<current.width {
                        let source = (y * current.width + x) * 4
                        let destination = (x * current.height + current.height - 1 - y) * 4
                        for channel in 0..<4 { output[destination + channel] = current.pixels[source + channel] }
                    }
                }
                current = Self(width: current.height, height: current.width, pixels: output)
            }
            return current
        }

        func orange(_ index: Int) -> Bool {
            let i = index * 4, r = Double(pixels[i]) / 255, g = Double(pixels[i + 1]) / 255, b = Double(pixels[i + 2]) / 255
            return r > 0.18 && r - g > 0.07 && g - b > 0.05 && g / r > 0.25 && g / r < 0.83 && b / r < 0.65
        }

        func bestMark() throws -> Mark? {
            let x0 = Int(Double(width) * 0.65), x1 = Int(Double(width) * 0.98)
            let y0 = Int(Double(height) * 0.28), y1 = Int(Double(height) * 0.60)
            var seen = [Bool](repeating: false, count: width * height)
            var best: Mark?
            for y in y0..<y1 {
                try Task.checkCancellation()
                for x in x0..<x1 {
                    let first = y * width + x
                    if seen[first] || !orange(first) { continue }
                    seen[first] = true
                    var queue = [first], head = 0, minX = x, maxX = x, minY = y, maxY = y
                    while head < queue.count {
                        if head % 2048 == 0 { try Task.checkCancellation() }
                        let p = queue[head]; head += 1
                        let px = p % width, py = p / width
                        minX = min(minX, px); maxX = max(maxX, px); minY = min(minY, py); maxY = max(maxY, py)
                        for dy in -1...1 { for dx in -1...1 {
                            let nx = px + dx, ny = py + dy
                            if nx < x0 || nx >= x1 || ny < y0 || ny >= y1 { continue }
                            let index = ny * width + nx
                            if !seen[index] && orange(index) { seen[index] = true; queue.append(index) }
                        } }
                    }
                    let w = maxX - minX + 1, h = maxY - minY + 1
                    guard (0.14...0.27).contains(Double(w) / Double(width)),
                          (0.09...0.18).contains(Double(h) / Double(height)),
                          (0.70...0.95).contains(Double(minX + w / 2) / Double(width)),
                          (0.34...0.57).contains(Double(minY + h / 2) / Double(height)) else { continue }
                    var shape = [Double](repeating: 0, count: 1024), totals = shape
                    // Only pixels in this connected component contribute. Nearby orange
                    // artwork cannot fill the mark's eyes, mouth or ear separation.
                    for index in queue {
                        let bin = min(31, (index / width - minY) * 32 / h) * 32 + min(31, (index % width - minX) * 32 / w)
                        shape[bin] += 1
                    }
                    for cy in 0..<h { for cx in 0..<w { totals[min(31, cy * 32 / h) * 32 + min(31, cx * 32 / w)] += 1 } }
                    var error = 0.0
                    for i in shape.indices { error += abs(shape[i] / max(1, totals[i]) - PumpkinStampShape.mask[i]) }
                    let similarity = 1 - error / 1024
                    let darkFace = [(0.27, 0.35, 0.62, 0.67), (0.67, 0.75, 0.62, 0.67), (0.45, 0.55, 0.77, 0.84)].allSatisfy { a, b, c, d in
                        var dark = 0, total = 0
                        for yy in (minY + Int(c * Double(h)))..<(minY + Int(d * Double(h))) {
                            for xx in (minX + Int(a * Double(w)))..<(minX + Int(b * Double(w))) {
                                let i = (yy * width + xx) * 4
                                let luminance = (0.2126 * Double(pixels[i]) + 0.7152 * Double(pixels[i + 1]) + 0.0722 * Double(pixels[i + 2])) / 255
                                total += 1; if luminance < 0.25 { dark += 1 }
                            }
                        }
                        return total > 0 && Double(dark) / Double(total) >= 0.65
                    }
                    let mark = Mark(region: ImageCrop(x: Double(minX) / Double(width), y: Double(minY) / Double(height),
                        width: Double(w) / Double(width), height: Double(h) / Double(height)), similarity: similarity, darkFace: darkFace)
                    if best == nil || similarity > best!.similarity { best = mark }
                }
            }
            return best
        }
    }
}

/// Frozen 32×32 orange silhouette measured from B-027/C-001's prepared scan.
/// The mark, including ear separation and facial cutouts, is the reference;
/// no card names, collector numbers, finish labels or edition years participate.
/// Development comparison: other five B-027 stamps and 217 saved control fronts
/// at two resolutions. This limited fixture set is not a universal accuracy claim.
enum PumpkinStampShape {
    static let rows = [
        "##............................##",
        "####........................####",
        "#####......................#####",
        "######....................######",
        ".######..................######.",
        ".######..................######.",
        ".#######................#######.",
        "..#######..............#######..",
        "..#######..............#######..",
        "...#######.....##.#...#######...",
        "...##########################...",
        "....########################....",
        ".....######################.....",
        "......####################......",
        "......####################......",
        "......####################......",
        ".....######################.....",
        ".....######################.....",
        "....####.##.########.##.###.....",
        "....####....########....####....",
        "....####....########....####....",
        "....#####..####.#####..#####....",
        "....#..##################..#....",
        "....#...################...#....",
        "....#...####........####..##....",
        ".....#..#####......#####..#.....",
        ".....#..#####......#####..#.....",
        "......#.######....######.#......",
        "......####################......",
        "........################........",
        "..........############..........",
        "............########............"
    ]
    static let mask: [Double] = rows.flatMap { $0.map { $0 == "#" ? 1.0 : 0.0 } }
}
