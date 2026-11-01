import CoreGraphics
import Foundation
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

@Test(arguments: [-8.0, -3, 0, 2.5, 7])
func deskewEnclosesEveryObservedCorner(angle: Double) {
    let radians = angle * .pi / 180
    let corners = [CGPoint(x: -63, y: -88), CGPoint(x: 63, y: -88), CGPoint(x: 63, y: 88), CGPoint(x: -63, y: 88)]
        .map { p in CGPoint(x: 200 + p.x * cos(radians) - p.y * sin(radians), y: 250 + p.x * sin(radians) + p.y * cos(radians)) }
    let result = CardDeskew.analyze(points: corners, width: 400, height: 500)
    #expect(abs(result.degrees + angle) < 0.11)
    let rect = result.crop
    #expect(rect.width > 0 && rect.height > 0 && rect.x >= 0 && rect.y >= 0)
    // Aligned enclosing dimensions include a protective margin, never trim corners.
    let canvasWidth = 400 * cos(radians) + 500 * abs(sin(radians))
    let canvasHeight = 500 * cos(radians) + 400 * abs(sin(radians))
    #expect(abs(rect.width * canvasWidth - 128) < 0.5)
    #expect(abs(rect.height * canvasHeight - 178) < 0.5)
}

@Test func legacyEditsDecodeWithoutDeskewAndRejectInvalidAngles() throws {
    let request = ImageEditRequest(jobID: UUID(), sequence: 1)
    var value = try #require(JSONSerialization.jsonObject(with: Wire.encoder().encode(request)) as? [String: Any])
    value.removeValue(forKey: "deskewDegrees")
    let decoded = try Wire.decoder().decode(ImageEditRequest.self, from: JSONSerialization.data(withJSONObject: value))
    #expect(decoded.deskewDegrees == nil)
    for angle in [Double.infinity, .nan, -12.1, 12.1] {
        var invalid = request; invalid.deskewDegrees = angle
        #expect(throws: ScannerError.self) { try invalid.validate() }
    }
}

// Saved hardware fixtures only: these tests never open a scanner or overwrite scans.
// B-012 has inverted fronts/backs in the first three pairs and upright pairs thereafter.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_ORIENTATION_BATCH"] != nil))
func savedMixedBatchDeskewAndBackOrientation() throws {
    let root = URL(fileURLWithPath: try #require(ProcessInfo.processInfo.environment["SCANNER_ORIENTATION_BATCH"]))
    let output = URL(fileURLWithPath: ProcessInfo.processInfo.environment["SCANNER_ORIENTATION_OUTPUT"] ?? NSTemporaryDirectory()).appendingPathComponent("orientation-validation")
    try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
    for sequence in 1...22 {
        let data = try Data(contentsOf: root.appendingPathComponent(String(format: "C-%03d.png", sequence)))
        let plan = try CardProcessor.analyze(data)
        print("ORIENTATION", sequence, String(data: try Wire.encoder().encode(plan), encoding: .utf8)!)
        #expect(plan.orientationVerified)
        #expect(plan.orientationBasis == (sequence % 2 == 0 ? "text" : "pokemon-back-pattern"))
        #expect(plan.quarterTurns == (sequence <= 6 ? 2 : 0))
        let rendered = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: sequence, crop: plan.crop, quarterTurns: plan.quarterTurns, deskewDegrees: plan.deskewDegrees))
        try rendered.data.write(to: output.appendingPathComponent(String(format: "C-%03d.png", sequence)))
        #expect(rendered.width < rendered.height)
        if sequence % 2 == 0 {
            #expect(try PokemonBackOrientation.rotation(rendered.data, words: ["Pokemon"]) == nil)
        }
    }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_ORIENTATION_BATCH"] != nil))
func savedFrontAndBackAtEveryQuarterTurnAndSmallTilt() throws {
    let root = URL(fileURLWithPath: try #require(ProcessInfo.processInfo.environment["SCANNER_ORIENTATION_BATCH"]))
    for sequence in [1, 6] {
        let original = try Data(contentsOf: root.appendingPathComponent(String(format: "C-%03d.png", sequence)))
        let baseline = try CardProcessor.analyze(original)
        for turns in 0..<4 {
            for tilt in [-5.0, 5.0] {
                let input = try ImageProcessor.render(original, request: ImageEditRequest(jobID: UUID(), sequence: sequence, quarterTurns: turns, deskewDegrees: tilt)).data
                let plan = try CardProcessor.analyze(input)
                print("TRANSFORM", sequence, turns, tilt, plan.quarterTurns, plan.deskewDegrees ?? 0, plan.orientationBasis ?? "nil")
                #expect(plan.orientationVerified)
                #expect(plan.orientationBasis == (sequence == 1 ? "pokemon-back-pattern" : "text"))
                #expect(plan.quarterTurns == (6 - turns) % 4)
                #expect(abs((plan.deskewDegrees ?? 0) + tilt - (baseline.deskewDegrees ?? 0)) < 0.6)
            }
        }
    }
}

@Test func stylizedLogoEvidenceIsBounded() {
    for text in ["POKEMON", "PokéNON", "PaKeMoy", "POKOMOY", "Pokernon"] { #expect(PokemonBackOrientation.resemblesLogo(text)) }
    for text in ["TM", "Energy", "Trainer", "Pokémon search", "", "abcdefg"] { #expect(!PokemonBackOrientation.resemblesLogo(text)) }
}

// Rounded, rasterized borders match scanner silhouettes more closely than four
// ideal corners. Sub-degree tilt can improve the enclosing area by less than 0.5%.
@Test(arguments: [-0.3, 0, 0.3], [0, 1, 2, 3])
func subtleTiltOnRasterizedRoundedCardIsCorrected(angle: Double, quarterTurns: Int) {
    let radians = angle * .pi / 180
    var points: [CGPoint] = []
    let halfWidth = 115.0, halfHeight = 161.0, radius = 9.0
    for y in 20..<360 {
        let occupied = (50..<290).filter { x in
            let px = Double(x) - 170, py = Double(y) - 190
            let dx = abs(px * cos(radians) + py * sin(radians))
            let dy = abs(-px * sin(radians) + py * cos(radians))
            guard dx <= halfWidth, dy <= halfHeight else { return false }
            let cx = max(0, dx - halfWidth + radius), cy = max(0, dy - halfHeight + radius)
            return cx * cx + cy * cy <= radius * radius
        }
        if let left = occupied.first, let right = occupied.last {
            points += [CGPoint(x: left, y: y), CGPoint(x: right + 1, y: y),
                       CGPoint(x: left, y: y + 1), CGPoint(x: right + 1, y: y + 1)]
        }
    }
    for _ in 0..<quarterTurns { points = points.map { CGPoint(x: 500 - $0.y, y: $0.x) } }
    let result = CardDeskew.analyze(points: points, width: 400, height: 500)
    #expect(abs(result.degrees + angle) <= 0.11)
    if angle == 0 { #expect(result.degrees == 0) }
    #expect(result.warnings.isEmpty)
}

@Test func isolatedEdgeNoiseDoesNotRotateStraightCardsOrTrimObservedPixels() {
    var points: [CGPoint] = []
    for y in 80...260 {
        let left = y.isMultiple(of: 31) ? 95 : 100
        points += [CGPoint(x: left, y: y), CGPoint(x: 230, y: y)]
    }
    let result = CardDeskew.analyze(points: points, width: 400, height: 500)
    #expect(result.degrees == 0)
    #expect(result.crop.x * 400 < 95)
    #expect((result.crop.x + result.crop.width) * 400 > 230)
}
