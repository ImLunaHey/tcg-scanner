import Foundation
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

@Test func conservativeCardBoundsIncludeMarginAndRejectBlankFrames() throws {
    let width = 100, height = 200
    var pixels = [UInt8](repeating: 0, count: width * height * 4)
    #expect(throws: ScannerError.self) { try CardProcessor.foregroundCrop(rgba: pixels, width: width, height: height) }
    for y in 20..<80 { for x in 30..<73 {
        let i = (y * width + x) * 4
        pixels[i] = 100; pixels[i + 1] = 150; pixels[i + 2] = 200; pixels[i + 3] = 255
    } }
    let crop = try CardProcessor.foregroundCrop(rgba: pixels, width: width, height: height)
    #expect(abs(crop.x - 0.29) < 0.000001 && abs(crop.y - 0.095) < 0.000001)
    #expect(abs(crop.width - 0.45) < 0.000001 && abs(crop.height - 0.31) < 0.000001)
    for y in 120..<180 { for x in 30..<73 {
        let i = (y * width + x) * 4
        pixels[i] = 200; pixels[i + 1] = 150; pixels[i + 2] = 100; pixels[i + 3] = 255
    } }
    #expect(throws: ScannerError.self) { try CardProcessor.foregroundCrop(rgba: pixels, width: width, height: height) }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_CARD_PREPARATION_FIXTURE"] != nil))
func prepareSavedCardFixtureWithoutFeeding() throws {
    let directory = try #require(ProcessInfo.processInfo.environment["SCANNER_CARD_PREPARATION_FIXTURE"])
    for sequence in 1...2 {
        let data = try Data(contentsOf: URL(fileURLWithPath: directory).appendingPathComponent(String(format: "C-%03d.png", sequence)))
        let plan = try CardProcessor.analyze(data)
        print("CARD PLAN", sequence, String(data: try Wire.encoder().encode(plan), encoding: .utf8)!)
        #expect(plan.crop.width < 0.4 && plan.crop.height < 0.4)
        #expect(plan.quarterTurns == (sequence == 1 ? 0 : 2))
        #expect(plan.orientationVerified == (sequence == 2))
        let output = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: sequence, crop: plan.crop, quarterTurns: plan.quarterTurns))
        try output.data.write(to: URL(fileURLWithPath: "/tmp/scanner-prepared-\(sequence).png"))
    }
}
