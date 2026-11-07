import Foundation
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
            print("SWU", sequence, plan.orientationBasis ?? "nil", plan.quarterTurns)
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
