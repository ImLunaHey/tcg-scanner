import ArgumentParser
import Foundation
import ScannerAPI

extension Images {
    struct CheckCondition: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Check a prepared card back for visible marks locally. Does not assign a condition grade.")
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
            _ = try options.perform("images.checkCondition", params: .object(["jobID": .string(jobID), "sequence": .number(Double(sequence)), "identificationID": .string(identificationID)]))
        }
    }
}
