import Foundation
import CoreImage
import ImageIO
import ScannerAPI

public enum ImageProcessor {
    public static func render(_ data: Data, request: ImageEditRequest) throws -> (data: Data, width: Int, height: Int) {
        try request.validate()
        guard data.count <= 32 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 20000, height <= 20000, width * height <= 48_000_000,
              let original = CIImage(data: data, options: [.applyOrientationProperty: true]) else {
            throw ScannerError("resourceLimit", "Editing supports source images up to 32 MiB and 48 megapixels.")
        }
        let deskewed = original.transformed(by: CGAffineTransform(rotationAngle: -(request.deskewDegrees ?? 0) * .pi / 180))
        let e = deskewed.extent; let c = request.crop
        let rect = CGRect(x: e.minX + c.x * e.width, y: e.minY + (1 - c.y - c.height) * e.height,
                          width: c.width * e.width, height: c.height * e.height).integral.intersection(e)
        guard rect.width >= 1, rect.height >= 1 else { throw ScannerError("invalidParams", "Crop is smaller than one pixel.") }
        var image = deskewed.cropped(to: rect).transformed(by: CGAffineTransform(translationX: -rect.minX, y: -rect.minY))
        for _ in 0..<request.quarterTurns { image = image.oriented(.right) }
        let size = image.extent.integral.size
        guard size.width <= 20000, size.height <= 20000, size.width * size.height <= 40_000_000 else {
            throw ScannerError("resourceLimit", "Edited image exceeds 40 megapixels or 20,000 pixels per side.")
        }
        let context = CIContext(options: [.cacheIntermediates: false])
        guard let rendered = context.createCGImage(image, from: image.extent, format: .RGBA8,
            colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!) else { throw ScannerError("image", "Could not render image edit.") }
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, "public.png" as CFString, 1, nil) else { throw ScannerError("image", "Could not create PNG encoder.") }
        CGImageDestinationAddImage(destination, rendered, nil)
        guard CGImageDestinationFinalize(destination), output.length <= 64 * 1024 * 1024 else { throw ScannerError("image", "Edited PNG exceeds output limit or could not be encoded.") }
        return (output as Data, rendered.width, rendered.height)
    }
}
