import Foundation
import CoreGraphics
import ImageIO
import Vision

/// Standard black SWU logo back. Text, layout and absence of card rules must agree.
enum StarWarsBackOrientation {
    static func rotation(_ data: Data) throws -> Int? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceThumbnailMaxPixelSize: 200] as CFDictionary), layoutMatches(image) else { return nil }
        let orientations: [CGImagePropertyOrientation] = [.up, .right, .down, .left]
        var matches: [Int] = []
        for turns in (image.width < image.height ? [0, 2] : [1, 3]) {
            try Task.checkCancellation()
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate; request.recognitionLanguages = ["en-US"]; request.usesLanguageCorrection = false
            try VNImageRequestHandler(data: data, orientation: orientations[turns]).perform([request])
            let words = (request.results ?? []).filter { ($0.topCandidates(1).first?.confidence ?? 0) >= 0.5 }
            if words.count <= 4 && words.contains(where: { o in
                o.topCandidates(1).first?.string.uppercased().filter(\.isLetter) == "UNLIMITED" &&
                (0.24...0.44).contains(o.boundingBox.midY) && (0.35...0.65).contains(o.boundingBox.midX) && o.boundingBox.width > 0.5
            }) { matches.append(turns) }
        }
        return matches.count == 1 ? matches[0] : nil
    }
    static func layoutMatches(_ image: CGImage) -> Bool {
        let w = image.width, h = image.height
        guard (0.62...0.79).contains(Double(min(w,h)) / Double(max(w,h))) else { return false }
        var pixels = [UInt8](repeating: 0, count: w*h*4)
        guard let c = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w*4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        c.draw(image, in: CGRect(x: 0,y: 0,width: w,height: h))
        var dark = 0, white = 0, center = 0
        for y in 0..<h { for x in 0..<w {
            let i = (y*w+x)*4, values = [pixels[i],pixels[i+1],pixels[i+2]]
            if values.max()! < 55 { dark += 1 }
            if abs(Double(x)/Double(w)-0.5)<0.3 && abs(Double(y)/Double(h)-0.5)<0.15 {
                center += 1; if values.min()! > 180 { white += 1 }
            }
        } }
        return Double(dark)/Double(w*h)>0.65 && Double(white)/Double(max(1,center))>0.25
    }
}
