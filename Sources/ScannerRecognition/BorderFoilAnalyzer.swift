import CoreGraphics
import Foundation
import ImageIO
import ScannerAPI

/// Frozen local image cue, not a reflectivity measurement or a finish label.
struct BorderFoilMeasurement: Sendable, Equatable {
    enum State: String, Sendable { case present, absent, unstable, unsupported }
    static let calibratedThreshold = 0.008213026997467985
    let state: State
    let baseline: Double?
    let inset: Double?
    let downsampled: Double?
    let threshold = calibratedThreshold

    static let unsupported = Self(state: .unsupported, baseline: nil, inset: nil, downsampled: nil)

    static func consensus(baseline: Double, inset: Double, downsampled: Double) -> Self {
        let values = [baseline, inset, downsampled]
        guard values.allSatisfy({ $0.isFinite && $0 >= 0 }) else { return .unsupported }
        let above = values.filter { $0 > calibratedThreshold }.count
        return Self(state: above == 3 ? .present : above == 0 ? .absent : .unstable,
                    baseline: baseline, inset: inset, downsampled: downsampled)
    }
}

/// Measures only prepared, upright, ordinary card borders. The caller must also
/// verify preparation and layout provenance: aspect ratio alone cannot do that.
/// Absence of this cue does not establish non-holo; reverse holos can lack it.
enum BorderFoilAnalyzer {
    static func analyze(_ data: Data, orientation: Int) throws -> BorderFoilMeasurement {
        try Task.checkCancellation()
        guard (0..<4).contains(orientation) else {
            throw ScannerError("invalidParams", "Border analysis requires a valid quarter-turn orientation.")
        }
        guard data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, nil),
              CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 20_000, height <= 20_000,
              width * height <= 40_000_000 else {
            throw ScannerError("resourceLimit", "Border analysis requires one image up to 32 MiB and 40 megapixels.")
        }
        let full = try raster(source, maximumSize: 1260).rotated(quarterTurns: orientation)
        let aspect = Double(full.width) / Double(full.height)
        guard full.height >= 600, (0.65...0.80).contains(aspect) else { return .unsupported }
        let baseline = try full.minimumBand(inset: false)
        let inset = try full.minimumBand(inset: true)
        try Task.checkCancellation()
        let smaller = try raster(source, maximumSize: 630).rotated(quarterTurns: orientation)
        let downsampled = try smaller.minimumBand(inset: false)
        return .consensus(baseline: baseline, inset: inset, downsampled: downsampled)
    }

    private static func raster(_ source: CGImageSource, maximumSize: Int) throws -> Raster {
        try Task.checkCancellation()
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maximumSize
        ] as CFDictionary), let colourSpace = CGColorSpace(name: CGColorSpace.sRGB) else {
            throw ScannerError("image", "Could not decode the prepared card for border analysis.")
        }
        let width = image.width, height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        try pixels.withUnsafeMutableBytes { bytes in
            guard let context = CGContext(data: bytes.baseAddress, width: width, height: height,
                bitsPerComponent: 8, bytesPerRow: width * 4, space: colourSpace,
                bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue) else {
                throw ScannerError("image", "Could not render the prepared card for border analysis.")
            }
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        }
        return Raster(pixels: pixels, width: width, height: height)
    }

    private struct Raster {
        let pixels: [UInt8]
        let width: Int
        let height: Int

        /// Integer pixel rotation avoids extra interpolation or colour conversion.
        /// One turn is clockwise, matching CIImage.oriented(.right).
        func rotated(quarterTurns: Int) throws -> Self {
            guard quarterTurns != 0 else { return self }
            var current = self
            for _ in 0..<quarterTurns {
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
                current = Self(pixels: output, width: current.height, height: current.width)
            }
            return current
        }

        func minimumBand(inset: Bool) throws -> Double {
            let x0 = inset ? 0.02 : 0.015, x1 = inset ? 0.035 : 0.04
            return min(try band(x0, x1), try band(1 - x1, 1 - x0))
        }

        private func band(_ x0: Double, _ x1: Double) throws -> Double {
            var rows: [Double] = []
            let startX = Int(x0 * Double(width)), endX = Int(x1 * Double(width))
            for y in Int(0.12 * Double(height))..<Int(0.86 * Double(height)) {
                if y % 64 == 0 { try Task.checkCancellation() }
                var sum = 0.0
                for x in startX..<endX {
                    let i = (y * width + x) * 4
                    let r = Double(pixels[i]) / 255, g = Double(pixels[i + 1]) / 255, b = Double(pixels[i + 2]) / 255
                    sum += 0.2126 * r + 0.7152 * g + 0.0722 * b
                }
                rows.append(sum / Double(max(1, endX - startX)))
            }
            // Match the frozen experiment, including edge windows and rounding.
            let radius = max(1, Int((5 * Double(height) / 1260).rounded()))
            var energy = 0.0
            for i in rows.indices {
                let lo = max(0, i - radius), hi = min(rows.count - 1, i + radius)
                let residual = rows[i] - rows[lo...hi].reduce(0, +) / Double(hi - lo + 1)
                energy += residual * residual
            }
            return sqrt(energy / Double(max(1, rows.count)))
        }
    }
}
