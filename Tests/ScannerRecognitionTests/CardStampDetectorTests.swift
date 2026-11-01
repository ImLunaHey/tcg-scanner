import CoreGraphics
import Foundation
import ImageIO
import ScannerAPI
import Testing
@testable import ScannerRecognition

private func stampImage(shape: String = "stamp", darkCutouts: Bool = true, turns: Int = 0,
                        width: Int = 900, height: Int = 1260) throws -> Data {
    var pixels = [UInt8](repeating: 220, count: width * height * 4)
    for i in stride(from: 3, to: pixels.count, by: 4) { pixels[i] = 255 }
    let x0 = Int(Double(width) * 0.75), y0 = Int(Double(height) * 0.38)
    let w = Int(Double(width) * 0.21), h = Int(Double(height) * 0.135)
    for y in 0..<h { for x in 0..<w {
        let nx = (Double(x) / Double(w) - 0.5) * 2, ny = (Double(y) / Double(h) - 0.5) * 2
        let orange = shape == "stamp" ? PumpkinStampShape.mask[min(31, y * 32 / h) * 32 + min(31, x * 32 / w)] > 0 :
            shape == "circle" ? nx * nx + ny * ny < 1 : shape == "rectangle"
        let i = ((y0 + y) * width + x0 + x) * 4
        if orange { pixels[i] = 245; pixels[i + 1] = 120; pixels[i + 2] = 25 }
        else if shape == "stamp", darkCutouts { pixels[i] = 15; pixels[i + 1] = 15; pixels[i + 2] = 15 }
    } }
    let rotatedWidth = turns % 2 == 0 ? width : height, rotatedHeight = turns % 2 == 0 ? height : width
    var rotated = [UInt8](repeating: 0, count: pixels.count)
    for y in 0..<height { for x in 0..<width {
        let destination: (Int, Int)
        switch turns {
        case 1: destination = (height - 1 - y, x)
        case 2: destination = (width - 1 - x, height - 1 - y)
        case 3: destination = (y, width - 1 - x)
        default: destination = (x, y)
        }
        let a = (y * width + x) * 4, b = (destination.1 * rotatedWidth + destination.0) * 4
        for channel in 0..<4 { rotated[b + channel] = pixels[a + channel] }
    } }
    let provider = try #require(CGDataProvider(data: Data(rotated) as CFData))
    let image = try #require(CGImage(width: rotatedWidth, height: rotatedHeight, bitsPerComponent: 8, bitsPerPixel: 32,
        bytesPerRow: rotatedWidth * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGBitmapInfo.byteOrder32Big.union(CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent))
    let output = NSMutableData()
    let destination = try #require(CGImageDestinationCreateWithData(output, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, image, nil)
    #expect(CGImageDestinationFinalize(destination))
    return output as Data
}

@Test func pumpkinShapeNeedsFaceDetailsAndIsNotAnOrangeColourDetector() throws {
    #expect(PumpkinStampShape.rows.count == 32 && PumpkinStampShape.rows.allSatisfy { $0.count == 32 })
    let matching = try CardStampDetector.analyze(stampImage(), orientation: 0)
    #expect(matching.state == .present && matching.kind == "pikachu-pumpkin")
    #expect(matching.region != nil && (matching.similarity ?? 0) >= 0.88)
    #expect(try Wire.decoder().decode(CardStampEvidence.self, from: Wire.encoder().encode(matching)) == matching)
    for shape in ["blank", "rectangle", "circle"] {
        #expect(try CardStampDetector.analyze(stampImage(shape: shape), orientation: 0).state != .present)
    }
    // Matching orange silhouette with light eye/mouth holes is not enough.
    #expect(try CardStampDetector.analyze(stampImage(darkCutouts: false), orientation: 0).state == .uncertain)
}

@Test func pumpkinStampQuarterTurnsKeepRegionInUprightCoordinates() throws {
    for orientation in 0..<4 {
        let result = try CardStampDetector.analyze(stampImage(turns: (4 - orientation) % 4), orientation: orientation)
        #expect(result.state == .present && result.sourceQuarterTurns == orientation)
        let region = try #require(result.region)
        #expect(abs(region.x - 0.75) < 0.01 && abs(region.y - 0.38) < 0.01)
    }
}

@Test func pumpkinStampRejectsInvalidAndUnreadableSources() throws {
    for orientation in [-1, 4] {
        #expect(throws: ScannerError.self) { try CardStampDetector.analyze(Data(), orientation: orientation) }
    }
    #expect(throws: ScannerError.self) { try CardStampDetector.analyze(Data(), orientation: 0) }
    #expect(throws: ScannerError.self) { try CardStampDetector.analyze(Data(repeating: 0, count: 33 * 1024 * 1024), orientation: 0) }
    #expect(try CardStampDetector.analyze(stampImage(width: 100, height: 140), orientation: 0).state == .unsupported)
    #expect(try CardStampDetector.analyze(stampImage(width: 700, height: 700), orientation: 0).state == .unsupported)
}

@Test func pumpkinStampHonoursCancellation() async {
    let task = Task {
        withUnsafeCurrentTask { $0?.cancel() }
        return try CardStampDetector.analyze(Data(), orientation: 0)
    }
    do { _ = try await task.value; Issue.record("Cancelled stamp analysis did not stop") }
    catch { #expect(error is CancellationError) }
}

@Test func unknownPreparedCardStillKeepsItsPhysicalStamp() async throws {
    let preparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0,
        orientationVerified: true, warnings: [], orientationBasis: "text")
    let result = await CardFinishAnalyzer().estimate(try stampImage(),
        evidence: RecognitionEvidence(lines: [], candidates: []), preparation: preparation)
    #expect(result.edition?.stamp.state == .present)
    #expect(result.edition?.needsEditionHandling == true)
    #expect(result.edition?.name == nil && result.suggestedFinish == nil)
}

// Local opt-in comparison: original source files remain untouched. The synthetic
// mark above exercises geometry and colour guards; these are real scanned marks.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_STAMP_FIXTURES"] != nil))
func pumpkinStampMatchesSavedMarksAndRejectsSavedControlFronts() throws {
    struct Input: Decodable { let id: String; let path: String; let orientation: Int; let positive: Bool }
    struct Output: Encodable { let id: String; let result: CardStampEvidence }
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_STAMP_FIXTURES"])
    let inputs = try JSONDecoder().decode([Input].self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    var output: [Output] = []
    for input in inputs {
        let result = try CardStampDetector.analyze(Data(contentsOf: URL(fileURLWithPath: input.path)), orientation: input.orientation)
        if input.positive { #expect(result.state == .present, "Stamp fixture \(input.id)") }
        else { #expect(result.state == .absent || result.state == .unsupported, "Control fixture \(input.id)") }
        output.append(Output(id: input.id, result: result))
    }
    if let path = ProcessInfo.processInfo.environment["SCANNER_STAMP_OUTPUT"] {
        try Wire.encoder().encode(output).write(to: URL(fileURLWithPath: path), options: .withoutOverwriting)
    }
}
