import CoreGraphics
import Foundation
import ImageIO
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

private func backPixels() -> [UInt8] {
    var pixels = [UInt8](repeating: 255, count: 700 * 1000 * 4)
    for i in 0..<(700 * 1000) { pixels[i * 4] = 25; pixels[i * 4 + 1] = 45; pixels[i * 4 + 2] = 120 }
    return pixels
}
private func paint(_ pixels: inout [UInt8], x: Int, y: Int, width: Int, height: Int) {
    for yy in y..<(y + height) { for xx in x..<(x + width) {
        let i = (yy * 700 + xx) * 4
        pixels[i] = 225; pixels[i + 1] = 225; pixels[i + 2] = 225
    } }
}
@Test func conditionMarksExcludeArtworkNoiseAndPaperRim() {
    var pixels = backPixels()
    paint(&pixels, x: 80, y: 100, width: 500, height: 800) // White printed artwork is excluded.
    paint(&pixels, x: 0, y: 0, width: 3, height: 1000) // Normal outermost paper rim.
    paint(&pixels, x: 10, y: 400, width: 1, height: 1) // Single-pixel noise.
    let clear = ConditionInspector.inspect(pixels, width: 700, height: 1000)
    #expect(clear.state == .checked && clear.findings.isEmpty)
    paint(&pixels, x: 12, y: 60, width: 7, height: 7)
    paint(&pixels, x: 680, y: 500, width: 7, height: 7)
    paint(&pixels, x: 280, y: 14, width: 45, height: 3)
    let result = ConditionInspector.inspect(pixels, width: 700, height: 1000)
    #expect(Set(result.findings.map(\.kind)) == [.cornerWhitening, .edgeWhitening, .linearMark])
    let corner = result.findings.first { $0.kind == .cornerWhitening }!
    #expect(corner.region.x < 0.03 && corner.region.y < 0.08)
    #expect(result.findings.allSatisfy { $0.region.x >= 0 && $0.region.y >= 0 && $0.region.x + $0.region.width <= 1 && $0.region.y + $0.region.height <= 1 })
}
@Test func conditionCoordinatesMatchEncodedImageAndRejectUnsupportedSources() throws {
    var pixels = backPixels(); paint(&pixels, x: 12, y: 60, width: 8, height: 8)
    let data = try #require(CGDataProvider(data: Data(pixels) as CFData))
    let image = try #require(CGImage(width: 700, height: 1000, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: 2800,
        space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: data, decode: nil, shouldInterpolate: false, intent: .defaultIntent))
    let bytes = NSMutableData(); let destination = try #require(CGImageDestinationCreateWithData(bytes, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, image, nil); #expect(CGImageDestinationFinalize(destination))
    var preparation = CardPreparation(crop: ImageCrop(), quarterTurns: 0, orientationVerified: true, warnings: [], orientationBasis: "pokemon-back-pattern")
    let result = try ConditionInspector.analyze(bytes as Data, preparation: preparation)
    let finding = try #require(result.findings.first)
    #expect(result.state == .checked && finding.region.x < 0.03 && finding.region.y < 0.08)
    #expect(result.limitations.contains { $0.contains("does not mean Near Mint") })
    preparation.orientationVerified = false
    #expect(try ConditionInspector.analyze(bytes as Data, preparation: preparation).state == .unsupported)
    preparation.orientationVerified = true; preparation.orientationBasis = "text"
    #expect(try ConditionInspector.analyze(bytes as Data, preparation: preparation).state == .unsupported)
    preparation.orientationBasis = "pokemon-back-pattern"
    #expect(try ConditionInspector.analyze(Data(), preparation: preparation).state == .inconclusive)
    #expect(ConditionInspector.inspect(Array(repeating: 255, count: 700 * 1000 * 4), width: 700, height: 1000).state == .inconclusive)
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURE"] != nil))
func inspectSavedConditionRegionsWithoutAssigningGrades() throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURE"])
    let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    var checked = 0
    for edit in record.processedImages ?? [] {
        let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(edit.image.relativePath))
        let result = try ConditionInspector.analyze(data, preparation: edit.cardPreparation)
        if edit.cardPreparation?.orientationBasis == "pokemon-back-pattern" {
            #expect(result.state == .checked); checked += 1
        } else { #expect(result.state == .unsupported) }
        print("CONDITION", edit.request.sequence, String(data: try Wire.encoder().encode(result), encoding: .utf8)!)
    }
    #expect(checked > 0)
}
