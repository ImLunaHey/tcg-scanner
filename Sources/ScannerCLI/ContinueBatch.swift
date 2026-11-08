import ArgumentParser
import Foundation
import ScannerAPI

extension Batches {
    struct Continue: ParsableCommand {
        static let configuration = CommandConfiguration(commandName: "continue", abstract: "Physically scan another feed into a completed batch, keeping its settings and card numbers.")
        @OptionGroup var options: ClientOptions
        @Argument var id: String
        @Option(help: "Reuse this UUID when retrying this feed; a new UUID requests another feed.") var requestID = UUID().uuidString
        @Flag(help: "Return immediately; poll the same batch ID for progress.") var noWait = false
        func validate() throws {
            guard UUID(uuidString: id) != nil, UUID(uuidString: requestID) != nil else { throw ValidationError("Provide batch and request UUIDs.") }
        }
        func run() throws {
            var record = try options.request("batches.continue", params: .object(["id": .string(id), "requestID": .string(requestID)])).decode(ScanRecord.self)
            if !options.json { print("Continuing \(record.batchNumber.map(BatchNaming.batchFolder) ?? id); request \(requestID)") }
            while !noWait && !record.state.terminal {
                Thread.sleep(forTimeInterval: 0.5)
                record = try options.request("batches.get", params: .object(["id": .string(id)])).decode(ScanRecord.self)
            }
            try printScan(record, options: options)
        }
    }
}
