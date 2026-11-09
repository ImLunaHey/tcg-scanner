import CoreImage
import Foundation
import ImageIO
import ScannerAPI
import Vision

/// Camera input stays in memory. Only the perspective-corrected card reaches OCR.
public enum CameraCard {
    public static func extract(_ data: Data) throws -> Data? {
        guard !data.isEmpty, data.count <= CameraIdentification.maximumImageBytes,
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetType(source) as String? == "public.jpeg", CGImageSourceGetCount(source) == 1,
              let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = props[kCGImagePropertyPixelWidth] as? Int,
              let height = props[kCGImagePropertyPixelHeight] as? Int,
              (240...4096).contains(width), (240...4096).contains(height), width * height <= 8_000_000 else {
            throw ScannerError("invalidParams", "Provide one JPEG up to 512 KiB, 240–4096 pixels per side and 8 megapixels.")
        }
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: 1600
        ] as CFDictionary) else { throw ScannerError("invalidParams", "Camera JPEG could not be decoded.") }
        try Task.checkCancellation()
        let rectangles = VNDetectRectanglesRequest()
        rectangles.minimumConfidence = 0.6
        rectangles.minimumAspectRatio = 0.5; rectangles.maximumAspectRatio = 0.9
        rectangles.minimumSize = 0.35; rectangles.maximumObservations = 6
        rectangles.quadratureTolerance = 25
        try VNImageRequestHandler(cgImage: image).perform([rectangles])
        let candidates = (rectangles.results ?? []).filter {
            let box = $0.boundingBox
            let w = box.width * Double(image.width), h = box.height * Double(image.height)
            let ratio = min(w, h) / max(w, h)
            return box.width * box.height >= 0.35 && (0.52...0.88).contains(ratio)
                && box.minX > 0.01 && box.minY > 0.01 && box.maxX < 0.99 && box.maxY < 0.99
        }
        guard let card = candidates.max(by: { $0.boundingBox.width * $0.boundingBox.height < $1.boundingBox.width * $1.boundingBox.height }) else { return nil }
        func point(_ p: CGPoint) -> CIVector { CIVector(x: p.x * CGFloat(image.width), y: p.y * CGFloat(image.height)) }
        let input = CIImage(cgImage: image)
        let corrected = input.applyingFilter("CIPerspectiveCorrection", parameters: [
            "inputTopLeft": point(card.topLeft), "inputTopRight": point(card.topRight),
            "inputBottomLeft": point(card.bottomLeft), "inputBottomRight": point(card.bottomRight)
        ])
        try Task.checkCancellation()
        guard let rendered = CIContext(options: [.cacheIntermediates: false]).createCGImage(corrected, from: corrected.extent) else {
            throw ScannerError("image", "Camera perspective correction failed.")
        }
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, "public.jpeg" as CFString, 1, nil) else { throw ScannerError("image", "Camera JPEG encoding failed.") }
        CGImageDestinationAddImage(destination, rendered, [kCGImageDestinationLossyCompressionQuality: 0.92] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { throw ScannerError("image", "Camera JPEG encoding failed.") }
        return output as Data
    }
}
