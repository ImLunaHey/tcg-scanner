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
        } else if abs(degrees) < 0.15 || best.width * best.height > initial.width * initial.height * 0.995 {
            degrees = 0 // Avoid interpolating an effectively straight scan.
        }
        return Result(crop: bounds(points: points, width: width, height: height, degrees: degrees), degrees: degrees, warnings: warnings)
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
