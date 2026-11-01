import Foundation
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

/// Explicit offline hardware-result check. Never opens or feeds the scanner.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_OVERSCAN_FIXTURE"] != nil))
func savedOverscanCardsHaveBackgroundOutsideEveryEdge() throws {
    let directory = try #require(ProcessInfo.processInfo.environment["SCANNER_OVERSCAN_FIXTURE"])
    for sequence in 1...6 {
        let url = URL(fileURLWithPath: directory).appendingPathComponent(String(format: "C-%03d.png", sequence))
        let plan = try CardProcessor.analyze(Data(contentsOf: url), background: .black)
        let originalBounds = try #require(plan.sourceCrop)
        #expect(originalBounds.x > 0 && originalBounds.y > 0)
        #expect(originalBounds.x + originalBounds.width < 1 && originalBounds.y + originalBounds.height < 1)
        #expect(plan.orientationVerified)
        // B-023 C-001 front had a visible 0.3-degree tilt discarded by the old cutoff.
        if sequence == 2 { #expect(abs((plan.deskewDegrees ?? 0) + 0.3) < 0.11) }
        #expect(!plan.warnings.contains { $0.contains("scan boundary") })
        print("OVERSCAN PLAN", sequence, String(decoding: try Wire.encoder().encode(plan), as: UTF8.self))
    }
}
