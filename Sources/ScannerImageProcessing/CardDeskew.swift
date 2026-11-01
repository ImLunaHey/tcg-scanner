import CoreGraphics
import Foundation
import ScannerAPI

/// Minimum-area enclosing box of the outer foreground, not the printed artwork.
/// Coordinates are top-left pixels; positive angles rotate clockwise.
enum CardDeskew {
    struct Result {
        var crop: ImageCrop
        var degrees: Double
        var warnings: [String]
    }

    static func analyze(points: [CGPoint], width: Int, height: Int) -> Result {
        let initial = enclosure(points, degrees: 0)
        var best = initial, degrees = 0.0
        for step in -120...120 {
            let angle = Double(step) / 10
            let rect = enclosure(points, degrees: angle)
            if rect.width * rect.height < best.width * best.height { best = rect; degrees = angle }
        }
        var warnings: [String] = []
        if abs(degrees) >= 11.9 {
            degrees = 0
            warnings.append("Card tilt exceeds the supported deskew range. Original retained; review manually.")
        } else if let correction = edgeCorrection(points, degrees: degrees), abs(correction) <= 0.6,
                  abs(degrees + correction) < 11.9 {
            // Rounded corners and raster steps can make the minimum-area box
            // flat at zero despite tilted straight edges. Require two long,
            // parallel outer edges before trusting a finer correction.
            let refined = degrees + correction
            degrees = abs(refined) >= 0.15 ? refined : 0
        } else if abs(degrees) < 0.15 || best.width * best.height > initial.width * initial.height * 0.999 {
            // Keep a small noise floor, but retain real sub-degree corrections.
            // Rasterized rounded corners can reduce area gain well below 0.5%.
            degrees = 0
        }
        return Result(crop: bounds(points: points, width: width, height: height, degrees: degrees), degrees: degrees, warnings: warnings)
    }

    private static func edgeCorrection(_ points: [CGPoint], degrees: Double) -> Double? {
        let radians = degrees * .pi / 180
        var aligned = points.map { CGPoint(x: $0.x * cos(radians) - $0.y * sin(radians),
                                           y: $0.x * sin(radians) + $0.y * cos(radians)) }
        var box = enclosure(aligned, degrees: 0)
        let landscape = box.width > box.height
        if landscape {
            aligned = aligned.map { CGPoint(x: $0.y, y: $0.x) }
            box = enclosure(aligned, degrees: 0)
        }
        let middle = aligned.filter { $0.y > box.minY + box.height * 0.1 && $0.y < box.maxY - box.height * 0.1 }
        guard let left = edgeSlope(middle.filter { $0.x < box.midX }, span: box.height),
              let right = edgeSlope(middle.filter { $0.x > box.midX }, span: box.height),
              abs(atan(left) - atan(right)) * 180 / .pi <= 0.15 else { return nil }
        return atan((left + right) / 2) * 180 / .pi * (landscape ? -1 : 1)
    }

    /// Fit x against y, discarding isolated border noise before measuring tilt.
    private static func edgeSlope(_ points: [CGPoint], span: Double) -> Double? {
        guard points.count >= 24, span >= 80 else { return nil }
        func fit(_ samples: [CGPoint]) -> (slope: Double, intercept: Double)? {
            let meanX = samples.reduce(0) { $0 + $1.x } / Double(samples.count)
            let meanY = samples.reduce(0) { $0 + $1.y } / Double(samples.count)
            let variance = samples.reduce(0) { $0 + pow($1.y - meanY, 2) }
            guard variance > 0 else { return nil }
            let slope = samples.reduce(0) { $0 + ($1.x - meanX) * ($1.y - meanY) } / variance
            return (slope, meanX - slope * meanY)
        }
        guard let initial = fit(points) else { return nil }
        let residuals = points.map { abs($0.x - initial.slope * $0.y - initial.intercept) }.sorted()
        let tolerance = max(0.75, residuals[residuals.count / 2] * 3)
        let inliers = points.filter { abs($0.x - initial.slope * $0.y - initial.intercept) <= tolerance }
        guard inliers.count >= points.count * 4 / 5,
              let low = inliers.map(\.y).min(), let high = inliers.map(\.y).max(), high - low >= span * 0.5,
              let final = fit(inliers) else { return nil }
        let rms = sqrt(inliers.reduce(0) { $0 + pow($1.x - final.slope * $1.y - final.intercept, 2) } / Double(inliers.count))
        guard rms <= 0.65 else { return nil }
        return final.slope
    }

    static func bounds(points: [CGPoint], width: Int, height: Int, degrees: Double) -> ImageCrop {
        let canvas = enclosure([.zero, CGPoint(x: width, y: 0), CGPoint(x: 0, y: height), CGPoint(x: width, y: height)], degrees: degrees)
        // One analysis pixel protects all observed edges and interpolation rounding.
        let rect = enclosure(points, degrees: degrees).insetBy(dx: -1, dy: -1).intersection(canvas)
        return ImageCrop(x: (rect.minX - canvas.minX) / canvas.width, y: (rect.minY - canvas.minY) / canvas.height,
                         width: rect.width / canvas.width, height: rect.height / canvas.height)
    }

    private static func enclosure(_ points: [CGPoint], degrees: Double) -> CGRect {
        let angle = degrees * .pi / 180, c = cos(angle), s = sin(angle)
        var minX = Double.infinity, minY = Double.infinity, maxX = -Double.infinity, maxY = -Double.infinity
        for point in points {
            let x = point.x * c - point.y * s, y = point.x * s + point.y * c
            minX = min(minX, x); maxX = max(maxX, x); minY = min(minY, y); maxY = max(maxY, y)
        }
        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }
}
