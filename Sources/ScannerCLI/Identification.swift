import ArgumentParser
import Foundation
import ScannerAPI

extension Images {
    struct Identify: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Start English card identification on an existing original. Never feeds hardware. Reuse --request-id for retries.")
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        @Option var requestID: String = UUID().uuidString
        func validate() throws {
            guard let job = UUID(uuidString: jobID), let id = UUID(uuidString: requestID) else { throw ValidationError("Provide job and request UUIDs.") }
            try IdentificationRequest(id: id, jobID: job, sequence: sequence).validate()
        }
        func run() throws {
            _ = try options.perform("images.identify", params: .object(["jobID": .string(jobID), "sequence": .number(Double(sequence)), "id": .string(requestID), "language": .string("en")]))
        }
    }
    struct Identification: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Poll OCR, suggested matches and manual review metadata.")
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        func validate() throws {
            guard UUID(uuidString: jobID) != nil, (1...100).contains(sequence) else { throw ValidationError("Provide a job UUID and sequence 1–100.") }
        }
        func run() throws { _ = try options.perform("images.identification", params: .object(["jobID": .string(jobID), "sequence": .number(Double(sequence))])) }
    }
    struct Review: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Manually confirm a suggested card, finish and condition, or reject the identification.")
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        @Option var identificationID: String
        @Option var cardID: String?
        @Flag var reject = false
        @Option var finish = "unverified"
        @Option var condition = "unassessed"
        func validate() throws {
            guard UUID(uuidString: jobID) != nil, UUID(uuidString: identificationID) != nil, (1...100).contains(sequence),
                  reject != (cardID != nil), CardFinish(rawValue: finish) != nil, CardCondition(rawValue: condition) != nil else {
                throw ValidationError("Provide job/identification UUIDs, sequence 1–100 and either --card-id or --reject. See documentation for finish/condition values.")
            }
        }
        func run() throws {
            var params: [String: JSONValue] = ["jobID": .string(jobID), "sequence": .number(Double(sequence)), "identificationID": .string(identificationID), "finish": .string(finish), "condition": .string(condition)]
            if let cardID { params["cardID"] = .string(cardID) }
            _ = try options.perform("images.review", params: .object(params))
        }
    }
}
