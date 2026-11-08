import Foundation
import ScannerAPI
import ScannerCore

extension DiagnosticRouter {
    func serviceResponse(_ request: RPCRequest) async throws -> RPCResponse? {
        let methods: Set<String> = ["v1.inventory.missing", "v1.inventory.reference", "v1.inventory.list", "v1.diagnostics.buttonMonitor", "v1.diagnostics.stopButtonMonitor", "v1.jobs.list", "v1.batches.list", "v1.batches.get", "v1.batches.valuation", "v1.batches.storeStatus", "v1.batches.sendToStore", "v1.jobs.get", "v1.jobs.cancel",
            "v1.events", "v1.service.status", "v1.profiles.list", "v1.profiles.get", "v1.profiles.save", "v1.profiles.delete"]
        guard methods.contains(request.method) else { return nil }
        let params: [String: JSONValue]
        if let value = request.params {
            guard case .object(let object) = value else { throw RPCFailure(code: -32602, message: "Parameters must be an object.") }
            params = object
        } else { params = [:] }
        let result: JSONValue
        switch request.method {
        case "v1.inventory.missing":
            try only(params, keys: ["set"])
            result = try .wrapping(await coordinator.missingInventoryCards(set: identifier(params, "set")))
        case "v1.inventory.reference":
            try only(params, keys: ["key"])
            result = try .wrapping(await coordinator.inventoryReference(key: identifier(params, "key")))
        case "v1.inventory.list":
            try only(params, keys: ["limit", "offset", "search", "set"])
            let search: String
            if let value = params["search"] {
                guard case .string(let text) = value else { throw RPCFailure(code: -32602, message: "Expected search text.") }
                search = text
            } else { search = "" }
            let set: String
            if let value = params["set"] {
                guard case .string(let text) = value else { throw RPCFailure(code: -32602, message: "Expected set text.") }
                set = text
            } else { set = "" }
            result = try .wrapping(await coordinator.inventory(limit: integer(params, "limit", default: 50, range: 1...100),
                offset: integer(params, "offset", default: 0, range: 0...1_000_000), search: search, set: set))
        case "v1.diagnostics.buttonMonitor":
            try only(params, keys: ["seconds"])
            let seconds = try integer(params, "seconds", default: 300, range: 30...300)
            try await coordinator.startButtonObservation(seconds: seconds)
            result = .object(["schemaVersion": .number(1), "state": .string("opening"), "seconds": .number(Double(seconds))])
        case "v1.diagnostics.stopButtonMonitor":
            try only(params, keys: [])
            await coordinator.stopButtonObservation()
            result = .object(["schemaVersion": .number(1), "state": .string("stopped")])
        case "v1.batches.storeStatus", "v1.batches.sendToStore":
            try only(params, keys: ["id"])
            let id = try uuid(params, "id")
            result = try .wrapping(request.method == "v1.batches.storeStatus"
                ? await coordinator.storeTransferStatus(id) : await coordinator.sendToStore(id))
        case "v1.batches.valuation":
            try only(params, keys: ["id"])
            result = try .wrapping(await coordinator.batchValuation(uuid(params, "id")))
        case "v1.jobs.list", "v1.batches.list":
            try only(params, keys: ["limit", "offset"])
            result = try .wrapping(await coordinator.jobs(limit: integer(params, "limit", default: 50, range: 1...100),
                                                        offset: integer(params, "offset", default: 0, range: 0...1_000_000)))
        case "v1.jobs.get", "v1.jobs.cancel", "v1.batches.get":
            try only(params, keys: ["id"])
            let id = try uuid(params, "id")
            if request.method == "v1.jobs.cancel" { result = try .wrapping(await coordinator.cancelScan(id)) }
            else { result = try .wrapping(await coordinator.scanStatus(id)) }
        case "v1.service.status":
            try only(params, keys: [])
            result = try .wrapping(await coordinator.status())
        case "v1.events":
            try only(params, keys: ["after", "limit", "waitSeconds"])
            let after = try integer(params, "after", default: 0, range: 0...9_007_199_254_740_991)
            result = try .wrapping(await coordinator.events(after: Int64(after), limit: integer(params, "limit", default: 100, range: 1...100),
                                                          waitSeconds: integer(params, "waitSeconds", default: 0, range: 0...20)))
        case "v1.profiles.list":
            try only(params, keys: [])
            result = try .object(["schemaVersion": .number(1), "profiles": .wrapping(await coordinator.profiles())])
        case "v1.profiles.get":
            try only(params, keys: ["id"])
            result = try .wrapping(await coordinator.profile(identifier(params, "id")))
        case "v1.profiles.save":
            try only(params, keys: ["id", "name", "dpi", "duplex", "format"])
            let profile: ScanProfile
            do { profile = try JSONValue.object(params).decode(ScanProfile.self) }
            catch { throw RPCFailure(code: -32602, message: "Expected profile id, name, dpi, duplex and format.") }
            try await coordinator.saveProfile(profile)
            result = try .wrapping(profile)
        case "v1.profiles.delete":
            try only(params, keys: ["id"])
            try await coordinator.removeProfile(identifier(params, "id"))
            result = .object(["deleted": .bool(true)])
        default: return nil
        }
        return RPCResponse(id: request.id, result: result)
    }
    func only(_ params: [String: JSONValue], keys: Set<String>) throws {
        guard params.keys.allSatisfy(keys.contains) else { throw RPCFailure(code: -32602, message: "Unknown parameter.") }
    }
    func uuid(_ params: [String: JSONValue], _ key: String) throws -> UUID {
        guard case .string(let text) = params[key], let id = UUID(uuidString: text) else { throw RPCFailure(code: -32602, message: "Expected \(key) as UUID.") }
        return id
    }
    func integer(_ params: [String: JSONValue], _ key: String, default fallback: Int, range: ClosedRange<Int>) throws -> Int {
        guard let value = params[key] else { return fallback }
        guard case .number(let n) = value, n.isFinite, n.rounded() == n, n >= Double(range.lowerBound), n <= Double(range.upperBound) else {
            throw RPCFailure(code: -32602, message: "\(key) must be an integer within \(range).")
        }
        return Int(n)
    }
}
