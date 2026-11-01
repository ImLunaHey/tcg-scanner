import Foundation
import ImageIO
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_600DPI_FIXTURE"] != nil))
func saved600DPIOverscanCanBePreparedWithoutReducingCardResolution() throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_600DPI_FIXTURE"])
    let data = try Data(contentsOf: URL(fileURLWithPath: path))
    let source = try #require(CGImageSourceCreateWithData(data as CFData, nil))
    let properties = try #require(CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any])
    let width = try #require(properties[kCGImagePropertyPixelWidth] as? Int)
    let height = try #require(properties[kCGImagePropertyPixelHeight] as? Int)
    #expect(width * height > 40_000_000 && width * height <= 48_000_000)
    let plan = try CardProcessor.analyze(data)
    let request = ImageEditRequest(jobID: UUID(), sequence: 1, crop: plan.crop,
        quarterTurns: plan.quarterTurns, deskewDegrees: plan.deskewDegrees)
    let output = try ImageProcessor.render(data, request: request)
    #expect(min(output.width, output.height) > 1200)
    #expect(max(output.width, output.height) < 3000)
    // Input allowance does not permit an oversized uncropped output.
    #expect(throws: ScannerError.self) {
        try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: 1))
    }
}
