import Foundation
import ScannerAPI

public protocol CardPrintingProviding: Sendable {
    func printings(cardID: String) async throws -> [CardPrintingVariant]
}

/// Fixed-host public metadata GETs only; no image or OCR text is uploaded.
public actor CardPrintingProvider: CardPrintingProviding {
    private let catalogue = TCGdexClient()
    public init() {}
    public func printings(cardID: String) async throws -> [CardPrintingVariant] {
        try await catalogue.detail(cardID).printingVariants
    }
}
