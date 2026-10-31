import ArgumentParser
import Foundation
import ScannerAPI

extension Images {
    struct PrepareCard: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Crop and orient a saved card into a new PNG. Never feeds hardware.")
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        @Option var requestID = UUID().uuidString
        func validate() throws {
            guard UUID(uuidString: jobID) != nil, UUID(uuidString: requestID) != nil, (1...100).contains(sequence) else {
                throw ValidationError("Provide job/request UUIDs and sequence 1–100.")
            }
        }
        func run() throws {
            _ = try options.perform("images.prepareCard", params: .object(["jobID": .string(jobID), "sequence": .number(Double(sequence)), "id": .string(requestID)]))
        }
    }
}
