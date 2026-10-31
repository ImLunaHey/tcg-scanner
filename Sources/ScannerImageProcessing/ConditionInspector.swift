import CoreGraphics
import Foundation
import ImageIO
import ScannerAPI

/// Conservative candidate-mark detection on an upright, prepared standard blue
/// back. Requires independently established back orientation; never grades cards.
public enum ConditionInspector {
    public static func check(_ data: Data, preparation: CardPreparation?) -> ConditionCheck {
        do { return try analyze(data, preparation: preparation) }
        catch { return ConditionCheck(state: .inconclusive, limitations: ["The saved image could not be analyzed. Inspect the physical card; no grade was assigned."]) }
    }

    public static func analyze(_ data: Data, preparation: CardPreparation?) throws -> ConditionCheck {
        let limits = ["Experimental candidate marks, not confirmed damage or a condition grade.",
            "Checks only the blue outer border of a prepared standard back. Fronts, artwork, the outermost rim and hidden surfaces are not assessed.",
            "Dust, print variation and scan artifacts can resemble wear. Fine scratches, dents and creases may be invisible; no flagged marks does not mean Near Mint."]
        guard preparation?.orientationVerified == true, preparation?.orientationBasis == "pokemon-back-pattern" else {
            return ConditionCheck(state: .unsupported, limitations: ["This check requires an upright, automatically prepared standard blue Pokémon back."] + limits)
        }
        try Task.checkCancellation()
        guard data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, nil), CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width >= 500, height >= 700, width <= 20_000, height <= 20_000, width * height <= 40_000_000,
              (0.65...0.80).contains(Double(width) / Double(height)),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 1200] as CFDictionary) else {
            return ConditionCheck(state: .inconclusive, limitations: ["Needs one upright card image of at least 500 × 700 pixels, up to 32 MiB and 40 megapixels."] + limits)
        }
        let w = image.width, h = image.height
        var pixels = [UInt8](repeating: 0, count: w * h * 4)
        guard let context = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue) else {
            throw ScannerError("image", "Cannot read condition-check pixels.")
        }
        context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        var result = inspect(pixels, width: w, height: h)
        result.limitations += limits
        return result
    }

    static func inspect(_ pixels: [UInt8], width w: Int, height h: Int) -> ConditionCheck {
        var mask = [UInt8](repeating: 0, count: w * h)
        var border = 0, blue = 0
        for y in 0..<h { for x in 0..<w {
            let nx = Double(x) / Double(w), ny = Double(y) / Double(h)
            // Exclude the very outer rim (normal paper edge/antialiasing) and all
            // interior printing. Rounded black corners never pass the light test.
            guard nx >= 0.008, nx <= 0.992, ny >= 0.006, ny <= 0.994,
                  nx < 0.045 || nx > 0.955 || ny < 0.032 || ny > 0.968 else { continue }
            let i = (y * w + x) * 4
            let r = Int(pixels[i]), g = Int(pixels[i + 1]), b = Int(pixels[i + 2])
            border += 1
            if b > 35 && Double(b) > Double(r) * 1.3 && Double(b) > Double(g) * 1.15 { blue += 1 }
            if min(r, g, b) > 145 && max(r, g, b) - min(r, g, b) < 55 { mask[y * w + x] = 1 }
        } }
        guard Double(blue) / Double(max(1, border)) > 0.55 else {
            return ConditionCheck(state: .inconclusive, limitations: ["The border is not consistently blue. Check cropping, lighting and card-back design manually."])
        }
        var found: [(area: Int, finding: ConditionCheck.Finding)] = []
        var queue: [Int] = []
        for start in mask.indices where mask[start] == 1 {
            queue.removeAll(keepingCapacity: true); queue.append(start); mask[start] = 0
            var head = 0, left = w, right = 0, top = h, bottom = 0
            while head < queue.count {
                let p = queue[head]; head += 1; let x = p % w, y = p / w
                left = min(left, x); right = max(right, x); top = min(top, y); bottom = max(bottom, y)
                for dy in -1...1 { for dx in -1...1 {
                    let xx = x + dx, yy = y + dy
                    guard xx >= 0, xx < w, yy >= 0, yy < h else { continue }
                    let next = yy * w + xx
                    if mask[next] == 1 { mask[next] = 0; queue.append(next) }
                } }
            }
            let bw = right - left + 1, bh = bottom - top + 1
            guard queue.count >= max(8, w * h / 90_000), bw >= 2, bh >= 2 else { continue }
            let cx = Double(left + right) / Double(2 * w), cy = Double(top + bottom) / Double(2 * h)
            let corner = (cx < 0.12 || cx > 0.88) && (cy < 0.10 || cy > 0.90)
            let linear = max(bw, bh) >= max(24, w / 25) && max(bw, bh) >= min(bw, bh) * 6
            let kind: ConditionCheck.Finding.Kind = linear ? .linearMark : corner ? .cornerWhitening : .edgeWhitening
            let explanation = linear ? "Elongated light mark in the blue border. Inspect for a scratch or crease; a scan artifact is also possible." :
                corner ? "Light patch near a corner. Inspect for whitening, chipped ink or a scan artifact." :
                "Light patch in the blue edge border. Inspect for whitening, chipped ink or a scan artifact."
            let x = max(0, left - 3), y = max(0, top - 3)
            let region = ImageCrop(x: Double(x) / Double(w), y: Double(y) / Double(h),
                width: Double(min(w, right + 4) - x) / Double(w), height: Double(min(h, bottom + 4) - y) / Double(h))
            found.append((queue.count, .init(kind: kind, region: region, explanation: explanation)))
        }
        found.sort { $0.area != $1.area ? $0.area > $1.area :
            $0.finding.region.y != $1.finding.region.y ? $0.finding.region.y < $1.finding.region.y : $0.finding.region.x < $1.finding.region.x }
        var result = ConditionCheck(state: .checked, findings: found.prefix(24).map(\.finding),
            limitations: found.count > 24 ? ["Showing the 24 largest candidate marks; inspect the entire card."] : [])
        result.analyzedWidth = w; result.analyzedHeight = h
        return result
    }
}
