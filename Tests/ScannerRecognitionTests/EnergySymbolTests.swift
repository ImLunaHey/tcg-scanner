import CoreImage
import Foundation
import ImageIO
import ScannerAPI
import Testing
import UniformTypeIdentifiers
@testable import ScannerRecognition

private let energyHeading = [RecognizedLine(text: "ENERGY", x: 0.06, y: 0.94, width: 0.3, height: 0.03)]

@Test func energyTypeRequiresHeadingAndRejectsSpecialEnergyBodyText() {
    #expect(EnergySymbolDetector.hasEnergyHeading(energyHeading))
    #expect(!EnergySymbolDetector.hasEnergyHeading([RecognizedLine(text: "Psychic Pokémon", x: 0.1, y: 0.94)]))
    #expect(!EnergySymbolDetector.hasEnergyHeading([RecognizedLine(text: "Energy Search", x: 0.1, y: 0.94)]))
    #expect(!EnergySymbolDetector.hasEnergyHeading(energyHeading + [RecognizedLine(text: "This card provides energy", y: 0.3)]))
    #expect(!EnergySymbolDetector.hasEnergyHeading([RecognizedLine(text: "ENERGY", x: 0.6, y: 0.02)]))
}

@Test func energyShapeRejectsCirclesBlankAndNoise() {
    let circle = (0..<576).map { i in pow(Double(i % 24) - 11.5, 2) + pow(Double(i / 24) - 11.5, 2) < 120 }
    let noise = (0..<576).map { ($0 * 73 % 109) < 50 }
    for mask in [circle, noise, Array(repeating: false, count: 576), Array(repeating: true, count: 576)] {
        #expect(EnergySymbolDetector.classify(.init(mask: mask, ratio: 1, box: .zero)) == nil)
    }
}

@Test func savedEnergyCornerSymbols() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_ENERGY_SYMBOL_IMAGES"] else { return }
    let paths = try JSONDecoder().decode([String].self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    for path in paths {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let result = try EnergySymbolDetector.analyze(data, reading: .init(rotation: 0, lines: energyHeading))
        print("ENERGY_SYMBOL", URL(fileURLWithPath: path).lastPathComponent, result?.type ?? "unreadable", result?.similarity ?? 0)
        if path.contains("edit-") { #expect(result?.type == "Psychic") }
    }
}

@Test func savedEnergyRecognitionKeepsTypeSeparateFromPrintingInEveryOrientation() async throws {
    guard let file = ProcessInfo.processInfo.environment["SCANNER_ENERGY_RECOGNITION_IMAGE"] else { return }
    let source = try CardOCR.image(Data(contentsOf: URL(fileURLWithPath: file)))
    let recognizer = CardRecognizer()
    // Removing only the corner icon must remove the type evidence, despite
    // unchanged purple background and the large central Psychic symbol.
    let base = CIImage(cgImage: source)
    let region = CGRect(x: Double(source.width) * 0.82, y: Double(source.height) * 0.87,
                        width: Double(source.width) * 0.18, height: Double(source.height) * 0.13)
    let covered = CIImage(color: .white).cropped(to: region).composited(over: base)
    let coveredImage = try #require(CIContext().createCGImage(covered, from: base.extent))
    let coveredBytes = NSMutableData()
    let coveredWriter = try #require(CGImageDestinationCreateWithData(coveredBytes, UTType.png.identifier as CFString, 1, nil))
    CGImageDestinationAddImage(coveredWriter, coveredImage, nil)
    #expect(CGImageDestinationFinalize(coveredWriter))
    #expect(try EnergySymbolDetector.analyze(coveredBytes as Data, reading: .init(rotation: 0, lines: energyHeading)) == nil)
    for exif in [1, 6, 3, 8] {
        let rotated = CIImage(cgImage: source).oriented(forExifOrientation: Int32(exif))
        let image = try #require(CIContext().createCGImage(rotated, from: rotated.extent))
        let bytes = NSMutableData()
        let writer = try #require(CGImageDestinationCreateWithData(bytes, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(writer, image, nil)
        #expect(CGImageDestinationFinalize(writer))
        let evidence = try await recognizer.recognize(bytes as Data, language: .english)
        #expect(evidence.energySymbol?.type == "Psychic")
        #expect(evidence.candidates.isEmpty)
        #expect(evidence.warnings.contains { $0.contains("Exact printing is unknown") })
    }
}

@Test func savedPreviouslyMatchedEnergyTypesAgreeWithCornerSymbols() throws {
    struct Fixture: Decodable { let name: String; let path: String }
    guard let file = ProcessInfo.processInfo.environment["SCANNER_ENERGY_KNOWN_IMAGES"] else { return }
    let fixtures = try JSONDecoder().decode([Fixture].self, from: Data(contentsOf: URL(fileURLWithPath: file)))
    for fixture in fixtures {
        let data = try Data(contentsOf: URL(fileURLWithPath: fixture.path))
        var symbols: [EnergySymbolEvidence] = []
        for reading in try CardOCR.read(data) {
            if let symbol = try EnergySymbolDetector.analyze(data, reading: reading) { symbols.append(symbol) }
        }
        print("KNOWN_ENERGY", fixture.name, symbols.map(\.displayName))
        #expect(symbols.count == 1 && symbols.first?.displayName == fixture.name)
    }
}
