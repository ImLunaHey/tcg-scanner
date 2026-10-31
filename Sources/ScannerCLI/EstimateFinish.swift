import ArgumentParser
import Foundation
import ScannerAPI

extension Images {
    struct EstimateFinish: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Estimate a saved card's finish locally without changing reviewed fields or feeding hardware.")
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        @Option var identificationID: String
        func validate() throws {
            guard UUID(uuidString: jobID) != nil, UUID(uuidString: identificationID) != nil, (1...100).contains(sequence) else {
                throw ValidationError("Provide job/identification UUIDs and sequence 1–100.")
            }
        }
        func run() throws {
            _ = try options.perform("images.estimateFinish", params: .object(["jobID": .string(jobID), "sequence": .number(Double(sequence)), "identificationID": .string(identificationID)]))
        }
    }
}
