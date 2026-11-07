import Foundation
import CoreGraphics
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_SWU_BATCH"] != nil))
func savedStarWarsPreparation() throws {
    let root = try #require(ProcessInfo.processInfo.environment["SCANNER_SWU_BATCH"])
    for sequence in 1...32 {
        let data = try Data(contentsOf: URL(fileURLWithPath: root).appendingPathComponent(String(format: "C-%03d.png", sequence)))
        do {
            let plan = try CardProcessor.analyze(data)
            print("SWU", sequence, plan.orientationBasis ?? "nil", plan.quarterTurns, plan.deskewDegrees ?? 0)
            let edit = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: sequence, crop: plan.crop, quarterTurns: plan.quarterTurns, deskewDegrees: plan.deskewDegrees))
            try edit.data.write(to: URL(fileURLWithPath: "/tmp/swu-prepared-\(sequence).png"))
            if sequence < 29 && !sequence.isMultiple(of: 2) { #expect(plan.orientationBasis == "starwars-back-pattern") }
        } catch {
            Issue.record("Capture \(sequence): \(error)")
        }
    }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_SWU_ROTATIONS"] != nil))
func savedStarWarsBackRotationsRejectPlayableFronts() throws {
    let back = try Data(contentsOf: URL(fileURLWithPath: "/tmp/swu-prepared-1.png"))
    for turn in 0..<4 {
        let rotated = try ImageProcessor.render(back, request: ImageEditRequest(jobID: UUID(), sequence: 1, quarterTurns: turn))
        #expect(try StarWarsBackOrientation.rotation(rotated.data) == (4 - turn) % 4)
    }
    for sequence in stride(from: 2, through: 32, by: 2) {
        let data = try Data(contentsOf: URL(fileURLWithPath: "/tmp/swu-prepared-\(sequence).png"))
        #expect(try StarWarsBackOrientation.rotation(data) == nil)
    }
}

@Test(arguments: [-2.5, -0.7, 0.8, 2.5])
func disconnectedDarkBorderPixelsPreservePhysicalTilt(angle: Double) throws {
    let width = 400, height = 600
    var rgba = [UInt8](repeating: 0, count: width * height * 4)
    let context = try #require(CGContext(data: &rgba, width: width, height: height, bitsPerComponent: 8,
        bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue))
    context.setFillColor(gray: 0, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    context.translateBy(x: 200, y: 300); context.rotate(by: angle * .pi / 180)
    // A faint outer border disconnected from the bright printed areas.
    context.setStrokeColor(gray: 0.06, alpha: 1); context.setLineWidth(2)
    context.stroke(CGRect(x: -90, y: -125, width: 180, height: 250))
    context.setFillColor(gray: 0.9, alpha: 1)
    context.fill(CGRect(x: -60, y: -40, width: 120, height: 50))
    context.fill(CGRect(x: -55, y: 35, width: 110, height: 20))
    let boundary = try CardProcessor.foregroundBoundary(rgba: rgba, width: width, height: height, background: .black, darkUnion: true)
    let result = CardDeskew.analyze(points: boundary.points, width: width, height: height)
    #expect(abs(result.degrees - angle) < 0.2)
    let radians = abs(result.degrees) * .pi / 180
    let canvasWidth = Double(width) * cos(radians) + Double(height) * sin(radians)
    let canvasHeight = Double(height) * cos(radians) + Double(width) * sin(radians)
    #expect(result.crop.width * canvasWidth >= 180)
    #expect(result.crop.height * canvasHeight >= 250)
    #expect(boundary.points.count > 100)
}
