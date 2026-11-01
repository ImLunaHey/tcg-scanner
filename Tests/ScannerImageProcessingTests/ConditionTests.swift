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
@Test func whiteBackingIsNotMistakenForWearButInternalMarksRemain() {
    var pixels = backPixels()
    paint(&pixels, x: 0, y: 0, width: 10, height: 1000)
    paint(&pixels, x: 690, y: 0, width: 10, height: 1000)
    paint(&pixels, x: 0, y: 0, width: 20, height: 18)
    paint(&pixels, x: 680, y: 0, width: 20, height: 18)
    let clean = ConditionInspector.inspect(pixels, width: 700, height: 1000)
    #expect(clean.state == .checked && clean.findings.isEmpty)
    paint(&pixels, x: 15, y: 60, width: 7, height: 7)
    let marked = ConditionInspector.inspect(pixels, width: 700, height: 1000)
    #expect(marked.findings.count == 1 && marked.findings[0].kind == .cornerWhitening)
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
        if edit.cardPreparation?.orientationBasis == "pokemon-back-pattern" || (record.batchNumber == 15 && edit.request.sequence % 2 == 1) {
            #expect(result.state == .checked); checked += 1
        } else { #expect(result.state == .unsupported) }
        print("CONDITION", edit.request.sequence, String(data: try Wire.encoder().encode(result), encoding: .utf8)!)
    }
    #expect(checked > 0)
}


@Test func conditionBorderMarksRemainAlignedAtEveryQuarterTurn() {
    var pixels = backPixels()
    paint(&pixels, x: 12, y: 60, width: 7, height: 7)
    for turns in 0..<4 {
        let w = turns % 2 == 0 ? 700 : 1000, h = turns % 2 == 0 ? 1000 : 700
        var rotated = [UInt8](repeating: 0, count: pixels.count)
        for y in 0..<1000 { for x in 0..<700 {
            let xx: Int, yy: Int
            switch turns {
            case 1: xx = 999 - y; yy = x
            case 2: xx = 699 - x; yy = 999 - y
            case 3: xx = y; yy = 699 - x
            default: xx = x; yy = y
            }
            for c in 0..<4 { rotated[(yy * w + xx) * 4 + c] = pixels[(y * 700 + x) * 4 + c] }
        } }
        let result = ConditionInspector.inspect(rotated, width: w, height: h)
        #expect(result.state == .checked && result.findings.count == 1)
        #expect(result.findings.first?.kind == .cornerWhitening)
        let r = result.findings[0].region
        let cx = r.x + r.width / 2, cy = r.y + r.height / 2
        #expect((turns == 1 || turns == 2) ? cx > 0.9 : cx < 0.1)
        #expect((turns == 2 || turns == 3) ? cy > 0.9 : cy < 0.1)
    }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURES"] != nil))
func backLayoutFallbackChecksUnreadableLogosAndRejectsSavedFronts() throws {
    let paths = try #require(ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURES"]).split(separator: ":")
    let unknown = CardPreparation(crop: ImageCrop(), quarterTurns: 0, orientationVerified: false, warnings: [], orientationBasis: "unknown")
    for path in paths {
        let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: String(path))))
        for identity in record.cardIdentifications ?? [] {
            guard let image = record.processedImages?.first(where: { $0.id == identity.sourceEditID })?.image else { continue }
            let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(image.relativePath))
            let front = identity.evidence?.candidates.isEmpty == false
            if front {
                // Force the fallback even though real front preparation already excludes it.
                #expect(try ConditionInspector.analyze(data, preparation: unknown).state == .unsupported)
            } else if record.batchNumber == 15 && [11, 17].contains(identity.request.sequence) {
                for turn in 0..<4 {
                    let bytes = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: 1, quarterTurns: turn)).data
                    let result = try ConditionInspector.analyze(bytes, preparation: unknown)
                    #expect(result.state == .checked && result.backRecognition == "colour-layout")
                    #expect(result.coverage?.count == 3)
                    print("BACK-FALLBACK", identity.request.sequence, turn, result.state.rawValue, result.findings.count)
                }
            }
        }
    }
}
