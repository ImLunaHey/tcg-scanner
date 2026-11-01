import Foundation
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

@Test func whiteBackingRejectsBlankAndIncludesGreyBorder() throws {
    let w = 200, h = 300
    var pixels = [UInt8](repeating: 245, count: w * h * 4)
    #expect(throws: ScannerError.self) { try CardProcessor.foregroundCrop(rgba: pixels, width: w, height: h, background: .white) }
    for y in 15..<155 { for x in 50..<150 {
        let i = (y * w + x) * 4
        let border = x < 55 || x >= 145 || y < 20 || y >= 150
        pixels[i] = border ? 170 : 230
        pixels[i+1] = border ? 170 : 180
        pixels[i+2] = border ? 170 : 80
    } }
    let crop = try CardProcessor.foregroundCrop(rgba: pixels, width: w, height: h, background: .white)
    #expect(crop.x <= 0.25 && crop.x + crop.width >= 0.75)
    #expect(crop.y <= 0.05 && crop.y + crop.height >= 155.0 / 300)
    #expect(crop.width < 0.52 && crop.height < 0.48)
}

// Explicit saved-scan integration test; never accesses scanner hardware.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_WHITE_FIXTURE"] != nil))
func prepareSavedWhiteBatch() throws {
    let directory = try #require(ProcessInfo.processInfo.environment["SCANNER_WHITE_FIXTURE"])
    let output = URL(fileURLWithPath: "/tmp/scanner-white-prepared", isDirectory: true)
    try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
    for sequence in 1...38 {
        let name = String(format: "C-%03d.png", sequence)
        let data = try Data(contentsOf: URL(fileURLWithPath: directory).appendingPathComponent(name))
        let plan = try CardProcessor.analyze(data, background: .white)
        #expect(plan.crop.width < 0.4 && plan.crop.height < 0.4)
        let rendered = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: sequence, crop: plan.crop, quarterTurns: plan.quarterTurns, deskewDegrees: plan.deskewDegrees))
        try rendered.data.write(to: output.appendingPathComponent(name))
        try Wire.encoder().encode(plan).write(to: output.appendingPathComponent(name + ".json"))
    }
}
