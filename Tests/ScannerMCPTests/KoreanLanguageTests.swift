import Foundation
import Testing
import MCP
import ScannerAPI
@testable import ScannerMCPSupport

@Test func mcpLanguageSchemasValidateScanAndSavedIdentification() throws {
    let scan = try #require(ToolCatalog.all.first { $0.name == "scanner_start_scan" })
    let identify = try #require(ToolCatalog.all.first { $0.name == "scanner_identify_card" })
    let id = UUID().uuidString
    let parameters = try scan.parameters(["requestID": .string(id), "prepareCards": .bool(true), "cardLanguage": .string("ko")])
    let request = try parameters.decode(ScanRequest.self)
    #expect(request.cardLanguage == .korean && request.prepareCards == true)
    #expect(scan.changesState && scan.destructive)
    #expect(try identify.parameters(["jobID": .string(id), "sequence": .int(1), "id": .string(UUID().uuidString), "language": .string("ko")]) != .null)
    for invalid in ["ja", "Korean", "KO"] {
        #expect(throws: ScannerError.self) { try scan.parameters(["requestID": .string(id), "cardLanguage": .string(invalid)]) }
        #expect(throws: ScannerError.self) {
            try identify.parameters(["jobID": .string(id), "sequence": .int(1), "id": .string(UUID().uuidString), "language": .string(invalid)])
        }
    }
}
