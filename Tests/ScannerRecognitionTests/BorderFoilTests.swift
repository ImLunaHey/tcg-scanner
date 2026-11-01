import CoreGraphics
import Foundation
import ImageIO
import Testing
@testable import ScannerRecognition

private func borderPNG(width: Int = 900, height: Int = 1260,
                       pixel: (Int, Int) -> UInt8) throws -> Data {
    var bytes = [UInt8](repeating: 255, count: width * height * 4)
    for y in 0..<height { for x in 0..<width {
        let value = pixel(x, y), index = (y * width + x) * 4
        bytes[index] = value; bytes[index + 1] = value; bytes[index + 2] = value
    } }
    let provider = try #require(CGDataProvider(data: Data(bytes) as CFData))
    let space = try #require(CGColorSpace(name: CGColorSpace.sRGB))
    let image = try #require(CGImage(width: width, height: height, bitsPerComponent: 8,
        bitsPerPixel: 32, bytesPerRow: width * 4, space: space,
        bitmapInfo: CGBitmapInfo.byteOrder32Big.union(CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent))
    let output = NSMutableData()
    let destination = try #require(CGImageDestinationCreateWithData(output, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, image, nil)
    #expect(CGImageDestinationFinalize(destination))
    return output as Data
}

private func oppositeBorderBands(_ x: Int, _ y: Int) -> UInt8 {
    (y % 16 < 8) != (x > 450) ? 180 : 80
}

@Test func borderTextureRequiresIndependentEvidenceOnBothSides() throws {
    let flat = try BorderFoilAnalyzer.analyze(borderPNG { _, _ in 128 }, orientation: 0)
    #expect(flat.state == .absent)
    #expect(try #require(flat.baseline) < 1e-12)
    let bands = try BorderFoilAnalyzer.analyze(borderPNG(pixel: oppositeBorderBands), orientation: 0)
    #expect(bands.state == .present)
    #expect(try #require(bands.baseline) > 0.05)
    // A single printed feature on one side must not stand in for both borders.
    let oneSide = try BorderFoilAnalyzer.analyze(borderPNG { x, y in
        x < 450 ? oppositeBorderBands(x, y) : 128
    }, orientation: 0)
    #expect(oneSide.state == .absent)
}

@Test func borderTextureConsensusKeepsDisagreementAndInvalidValuesDistinct() {
    let t = BorderFoilMeasurement.calibratedThreshold
    #expect(BorderFoilMeasurement.consensus(baseline: t, inset: t, downsampled: t).state == .absent)
    #expect(BorderFoilMeasurement.consensus(baseline: t * 2, inset: t / 2, downsampled: t * 2).state == .unstable)
    #expect(BorderFoilMeasurement.consensus(baseline: t * 2, inset: t * 2, downsampled: t * 2).state == .present)
    for value in [Double.nan, .infinity, -1] {
        #expect(BorderFoilMeasurement.consensus(baseline: value, inset: t, downsampled: t).state == .unsupported)
    }
}

@Test func borderTextureRejectsInvalidInputsAndUnsupportedGeometry() throws {
    #expect(throws: (any Error).self) { try BorderFoilAnalyzer.analyze(Data(), orientation: 0) }
    #expect(throws: (any Error).self) { try BorderFoilAnalyzer.analyze(Data(repeating: 0, count: 32 * 1024 * 1024 + 1), orientation: 0) }
    for orientation in [-1, 4] {
        #expect(throws: (any Error).self) { try BorderFoilAnalyzer.analyze(Data(), orientation: orientation) }
    }
    let oversizedAxis = try borderPNG(width: 20_001, height: 1) { _, _ in 128 }
    #expect(throws: (any Error).self) { try BorderFoilAnalyzer.analyze(oversizedAxis, orientation: 0) }
    let square = try borderPNG(width: 600, height: 600) { _, _ in 128 }
    #expect(try BorderFoilAnalyzer.analyze(square, orientation: 0).state == .unsupported)
    let small = try borderPNG(width: 360, height: 500) { _, _ in 128 }
    #expect(try BorderFoilAnalyzer.analyze(small, orientation: 0).state == .unsupported)
}

@Test func borderTextureQuarterTurnsRestoreTheSameMeasurements() throws {
    let upright = try BorderFoilAnalyzer.analyze(borderPNG(pixel: oppositeBorderBands), orientation: 0)
    let upsideDown = try borderPNG { x, y in oppositeBorderBands(899 - x, 1259 - y) }
    let landscape = try borderPNG(width: 1260, height: 900) { x, y in oppositeBorderBands(y, 1259 - x) }
    #expect(try BorderFoilAnalyzer.analyze(landscape, orientation: 0).state == .unsupported)
    for (data, turns) in [(upsideDown, 2), (landscape, 3)] {
        let restored = try BorderFoilAnalyzer.analyze(data, orientation: turns)
        #expect(restored.state == upright.state)
        #expect(abs(try #require(restored.baseline) - #require(upright.baseline)) < 1e-10)
        #expect(abs(try #require(restored.inset) - #require(upright.inset)) < 1e-10)
        #expect(abs(try #require(restored.downsampled) - #require(upright.downsampled)) < 1e-10)
    }
}

@Test func borderTextureHonoursCancellation() async throws {
    let task = Task {
        while !Task.isCancelled { await Task.yield() }
        return try BorderFoilAnalyzer.analyze(Data(), orientation: 0)
    }
    task.cancel()
    do {
        _ = try await task.value
        Issue.record("Cancelled border measurement unexpectedly succeeded")
    } catch {
        #expect(error is CancellationError)
    }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_BORDER_FIXTURES"] != nil))
func borderTextureMatchesFrozenExperimentalMeasurements() throws {
    struct Entry: Decodable {
        struct Input: Decodable { let id: String; let path: String }
        let input: Input
        let variant: String
        let features: [String: Double]
    }
    let files = try #require(ProcessInfo.processInfo.environment["SCANNER_BORDER_FIXTURES"]).split(separator: ":")
    for file in files {
        let rows = try JSONDecoder().decode([Entry].self, from: Data(contentsOf: URL(fileURLWithPath: String(file))))
        for (id, variants) in Dictionary(grouping: rows, by: { $0.input.id }).sorted(by: { $0.key < $1.key }) {
            let original = try #require(variants.first(where: { $0.variant == "baseline" }))
            let result = try BorderFoilAnalyzer.analyze(Data(contentsOf: URL(fileURLWithPath: original.input.path)), orientation: 0)
            #expect(result.state != .unsupported, "Unsupported saved fixture: \(id)")
            let measured = ["baseline": result.baseline, "inset": result.inset, "630": result.downsampled]
            for variant in variants {
                let actual = try #require(measured[variant.variant] ?? nil)
                let expected = try #require(variant.features["borderMinimumBand"])
                #expect(abs(actual - expected) < 1e-12, "Measurement changed: \(id) \(variant.variant)")
            }
        }
    }
}
