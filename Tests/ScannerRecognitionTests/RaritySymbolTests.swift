import Foundation
import CoreImage
import ImageIO
import UniformTypeIdentifiers
import Testing
import ScannerAPI
@testable import ScannerRecognition

@Test func rarityEvidenceNeverCreatesIdentityOrChangesFinish() {
    var rare = CardCandidate(id: "test-1", name: "Example", localID: "001", setID: "test", setName: "Test", officialSetCount: 100, evidence: ["Collector number 001"], availableFinishes: [.normal, .holo])
    rare.rarities = ["Rare"]
    var common = rare; common.id = "other-1"; common.rarities = ["Common"]
    let mark = RaritySymbolDetector.Reading(symbol: "circle", similarity: 0.9, region: ImageCrop(x: 0.2, y: 0.9, width: 0.02, height: 0.02), collectorNumber: "1", setTotal: "100")
    let ranked = RarityEvidence.apply(mark, to: [rare, common])
    #expect(ranked.map(\.id) == [common.id, rare.id])
    #expect(ranked[0].evidence.last?.contains("agrees") == true)
    #expect(ranked[1].evidence.last?.contains("conflict") == true)
    #expect(ranked.allSatisfy { $0.availableFinishes == [.normal, .holo] })
    #expect(RarityEvidence.apply(mark, to: []).isEmpty)
    #expect(RarityEvidence.apply(nil, to: [rare]) == [rare])
    var unrelated = mark; unrelated.collectorNumber = "2"
    #expect(RarityEvidence.apply(unrelated, to: [rare]) == [rare])
    unrelated = mark; unrelated.setTotal = "101"
    #expect(RarityEvidence.apply(unrelated, to: [rare]) == [rare])
    #expect(RarityEvidence.expectedSymbol("Illustration rare") == nil)
    #expect(RarityEvidence.expectedSymbol("Promo") == nil)
}

@Test func savedRarityScansDiagnostic() throws {
    // Explicit opt-in, read-only local scans. Never feeds hardware or changes history.
    guard let path = ProcessInfo.processInfo.environment["SCANNER_RARITY_IMAGES"] else { return }
    let paths = try JSONDecoder().decode([String].self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    for file in paths {
        let mark = try RaritySymbolDetector.analyze(Data(contentsOf: URL(fileURLWithPath: file)), rotation: 0, language: .english)
        print("RARITY", file, mark.map { "\($0.collectorNumber)/\($0.setTotal) \($0.symbol) \($0.similarity)" } ?? "unreadable")
    }
}

@Test func footerShapeRejectsHolesClippingAndNoise() {
    let w = 100, h = 40
    func pixels(_ ink: (Int, Int) -> Bool) -> [UInt8] {
        (0..<w*h).flatMap { i -> [UInt8] in ink(i % w, i / w) ? [15,15,15,255] : [245,245,245,255] }
    }
    let circle = pixels { x,y in (x-24)*(x-24)+(y-20)*(y-20) <= 100 }
    #expect(RaritySymbolDetector.classify(rgba: circle, width: w, height: h, threshold: 90)?.symbol == "circle")
    let diamond = pixels { x,y in abs(x-24)+abs(y-20) <= 12 }
    #expect(RaritySymbolDetector.classify(rgba: diamond, width: w, height: h, threshold: 90)?.symbol == "diamond")
    let hollow = pixels { x,y in let r=(x-24)*(x-24)+(y-20)*(y-20); return r <= 100 && r >= 60 }
    #expect(RaritySymbolDetector.classify(rgba: hollow, width: w, height: h, threshold: 90) == nil)
    let clipped = pixels { x,y in x*x+(y-20)*(y-20) <= 100 }
    #expect(RaritySymbolDetector.classify(rgba: clipped, width: w, height: h, threshold: 90) == nil)
    #expect(RaritySymbolDetector.classify(rgba: pixels { _,_ in false }, width: w, height: h, threshold: 90) == nil)
    #expect(RaritySymbolDetector.classify(rgba: [], width: w, height: h, threshold: 90) == nil)
}

@Test func printedStarSilhouetteAndPairedStarsAreDistinct() {
    // A filled printed star with broader inner arms than a geometric pentagram.
    let rows = ["..............#.............", ".............##.............", ".............##.............",
        "............####............", "............####............", "............#####...........",
        "..........#.#####...........", ".........###########........", ".....#####################..",
        "############################", "############################", ".#########################..",
        "...######################...", "....####################....", ".....##################.....",
        "......################......", "......################......", "......###############.......",
        ".......###############......", "......################......", "......################......",
        "......################......", "......################......", ".....######.......#####.....",
        "......###..........####....."]
    for count in [1, 2] {
        let w = 100, h = 40
        var pixels = [UInt8](repeating: 245, count: w*h*4)
        for star in 0..<count { for (y,row) in rows.enumerated() { for (x,c) in row.enumerated() where c == "#" {
            let i = ((y+6)*w + x+6+star*32)*4
            for channel in 0..<3 { pixels[i+channel] = 15 }
        } } }
        for threshold in [90,135] {
            #expect(RaritySymbolDetector.classify(rgba: pixels, width: w, height: h, threshold: threshold)?.symbol == (count == 1 ? "star" : "doubleStar"))
        }
    }
}

@Test func rarityCoordinatesFollowSavedCardRotation() throws {
    guard let file = ProcessInfo.processInfo.environment["SCANNER_RARITY_ROTATION_IMAGE"] else { return }
    let data = try Data(contentsOf: URL(fileURLWithPath: file))
    let expected = try #require(try RaritySymbolDetector.analyze(data, rotation: 0, language: .english))
    let source = try CardOCR.image(data)
    for turn in 1...3 {
        let rotated = CIImage(cgImage: source).oriented(forExifOrientation: [1, 6, 3, 8][turn])
        let image = try #require(CIContext().createCGImage(rotated, from: rotated.extent))
        let output = NSMutableData()
        let writer = try #require(CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(writer, image, nil)
        #expect(CGImageDestinationFinalize(writer))
        let actual = try #require(try RaritySymbolDetector.analyze(output as Data, rotation: (4-turn)%4, language: .english))
        #expect(actual.symbol == expected.symbol)
        #expect(actual.collectorNumber == expected.collectorNumber)
        #expect(abs(actual.region.x - expected.region.x) < 0.01)
        #expect(abs(actual.region.y - expected.region.y) < 0.01)
    }
    #expect(try RaritySymbolDetector.analyze(data, rotation: 0, language: .korean) == nil)
}

@Test func catalogueRarityIsValidatedAndDecoded() async throws {
    let json = #"{"id":"test-1","localId":"1","name":"Example","set":{"id":"test","name":"Test"},"rarity":"Uncommon"}"#
    let client = TCGdexClient(load: { _,_ in Data(json.utf8) })
    #expect(try await client.detail("test-1").rarity == "Uncommon")
    let oversized = json.replacingOccurrences(of: "Uncommon", with: String(repeating: "X", count: 129))
    let invalid = TCGdexClient(load: { _,_ in Data(oversized.utf8) })
    await #expect(throws: ScannerError.self) { try await invalid.detail("test-1") }
}

@Test func savedRarityIdentityDiagnostic() async throws {
    guard let file = ProcessInfo.processInfo.environment["SCANNER_RARITY_IDENTITY_IMAGE"] else { return }
    // Local image processing plus public catalogue GETs; no image upload or history mutation.
    let result = try await CardRecognizer().recognize(Data(contentsOf: URL(fileURLWithPath: file)), language: .english)
    let symbol = try #require(result.raritySymbol)
    let candidate = try #require(result.candidates.first)
    #expect(candidate.localID == symbol.collectorNumber)
    #expect(candidate.evidence.contains { $0.hasPrefix("Printed black rarity symbol agrees") })
    let roundTrip = try JSONDecoder().decode(RecognitionEvidence.self, from: JSONEncoder().encode(result))
    #expect(roundTrip.raritySymbol == result.raritySymbol)
    print("RARITY_IDENTITY", candidate.id, candidate.name, candidate.rarities ?? [], symbol.symbol)
}
