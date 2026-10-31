import Foundation
import ScannerAPI

extension DiagnosticRouter {
    func identificationResponse(_ request: RPCRequest) async throws -> RPCResponse? {
        guard ["v1.images.identify", "v1.images.identification", "v1.images.review", "v1.images.prepareCard", "v1.images.estimateFinish", "v1.images.checkCondition"].contains(request.method) else { return nil }
        guard case .object(let params) = request.params else { throw RPCFailure(code: -32602, message: "Expected object parameters.") }
        let jobID = try uuid(params, "jobID")
        let sequence = try integer(params, "sequence", default: 0, range: 1...100)
        guard sequence > 0 else { throw RPCFailure(code: -32602, message: "sequence is required.") }
        let result: JSONValue
        switch request.method {
        case "v1.images.checkCondition":
            try only(params, keys: ["jobID", "sequence", "identificationID"])
            result = try .wrapping(await coordinator.checkCondition(jobID: jobID, sequence: sequence, identificationID: uuid(params, "identificationID")))
        case "v1.images.estimateFinish":
            try only(params, keys: ["jobID", "sequence", "identificationID"])
            result = try .wrapping(await coordinator.estimateFinish(jobID: jobID, sequence: sequence, identificationID: uuid(params, "identificationID")))
        case "v1.images.prepareCard":
            try only(params, keys: ["id", "jobID", "sequence"])
            result = try .wrapping(await coordinator.prepareCard(jobID: jobID, sequence: sequence, id: uuid(params, "id")))
        case "v1.images.identify":
            try only(params, keys: ["id", "jobID", "sequence", "language"])
            let id = try uuid(params, "id")
            let language = try optionalIdentifier(params, "language") ?? "en"
            result = try .wrapping(await coordinator.identifyCard(IdentificationRequest(id: id, jobID: jobID, sequence: sequence, language: language)))
        case "v1.images.review":
            try only(params, keys: ["identificationID", "jobID", "sequence", "cardID", "finish", "condition"])
            guard let finish = CardFinish(rawValue: try optionalIdentifier(params, "finish") ?? "unverified"),
                  let condition = CardCondition(rawValue: try optionalIdentifier(params, "condition") ?? "unassessed") else {
                throw RPCFailure(code: -32602, message: "Invalid finish or condition value.")
            }
            result = try .wrapping(await coordinator.reviewCard(jobID: jobID, sequence: sequence,
                identificationID: uuid(params, "identificationID"), cardID: optionalIdentifier(params, "cardID"), finish: finish, condition: condition))
        default:
            try only(params, keys: ["jobID", "sequence"])
            result = try .object(["schemaVersion": .number(1), "identification": .wrapping(await coordinator.cardIdentification(jobID: jobID, sequence: sequence))])
        }
        return RPCResponse(id: request.id, result: result)
    }
}
