import Foundation
import ScannerAPI

extension DiagnosticRouter {
    func controlResponse(_ request: RPCRequest) async throws -> RPCResponse? {
        let methods: Set<String> = ["v1.device.get", "v1.settings.set", "v1.profiles.create", "v1.profiles.update", "v1.images.get", "v1.images.read", "v1.images.thumbnail", "v1.images.edit"]
        guard methods.contains(request.method) else { return nil }
        guard case .object(let params) = request.params ?? .object([:]) else { throw RPCFailure(code: -32602, message: "Expected object parameters.") }
        let result: JSONValue
        switch request.method {
        case "v1.device.get":
            try only(params, keys: ["deviceID"])
            result = try .wrapping(await coordinator.deviceStatus(optionalIdentifier(params, "deviceID")))
        case "v1.settings.set":
            try only(params, keys: ["deviceID", "dpi", "duplex", "format"])
            let settings: ScanSettings
            do { settings = try JSONValue.object(params.filter { $0.key != "deviceID" }).decode(ScanSettings.self) }
            catch { throw RPCFailure(code: -32602, message: "Expected dpi, duplex, format and optional deviceID.") }
            result = try .wrapping(await coordinator.setSettings(deviceID: optionalIdentifier(params, "deviceID"), settings: settings))
        case "v1.profiles.create":
            try only(params, keys: ["id", "name", "dpi", "duplex", "format"])
            let profile: ScanProfile
            do { profile = try JSONValue.object(params).decode(ScanProfile.self) }
            catch { throw RPCFailure(code: -32602, message: "Expected id, name, dpi, duplex and format.") }
            result = try .wrapping(await coordinator.createProfile(profile))
        case "v1.profiles.update":
            try only(params, keys: ["id", "name", "dpi", "duplex", "format"])
            let id = try identifier(params, "id")
            let patch: ProfilePatch
            guard params.values.allSatisfy({ $0 != .null }) else { throw RPCFailure(code: -32602, message: "Profile updates cannot contain null fields.") }
            do { patch = try JSONValue.object(params.filter { $0.key != "id" }).decode(ProfilePatch.self) }
            catch { throw RPCFailure(code: -32602, message: "Invalid profile update fields.") }
            result = try .wrapping(await coordinator.updateProfile(id, patch: patch))
        case "v1.images.edit":
            try only(params, keys: ["id", "jobID", "sequence", "crop", "quarterTurns"])
            guard case .object(let crop) = params["crop"], Set(crop.keys) == ["x", "y", "width", "height"] else { throw RPCFailure(code: -32602, message: "Expected a normalized crop rectangle.") }
            let edit: ImageEditRequest
            do { edit = try JSONValue.object(params).decode(ImageEditRequest.self); try edit.validate() }
            catch { throw RPCFailure(code: -32602, message: "Invalid image edit parameters.") }
            result = try .wrapping(await coordinator.editImage(edit))
        case "v1.images.get", "v1.images.read", "v1.images.thumbnail":
            try only(params, keys: request.method == "v1.images.thumbnail" ? ["jobID", "sequence", "editID", "format"] : request.method != "v1.images.read" ? ["jobID", "sequence", "editID"] : ["jobID", "sequence", "offset", "length", "editID"])
            let id = try uuid(params, "jobID")
            let editID = try params["editID"].map { _ in try uuid(params, "editID") }
            let sequence = try integer(params, "sequence", default: 0, range: 1...100)
            guard sequence > 0 else { throw RPCFailure(code: -32602, message: "sequence is required.") }
            if request.method == "v1.images.get" { result = try .wrapping(await coordinator.imageMetadata(jobID: id, sequence: sequence, editID: editID)) }
            else if request.method == "v1.images.thumbnail" {
                guard let format = ThumbnailFormat(rawValue: try optionalIdentifier(params, "format") ?? "png") else { throw RPCFailure(code: -32602, message: "Thumbnail format must be png or jpeg.") }
                result = try .wrapping(await coordinator.thumbnail(jobID: id, sequence: sequence, editID: editID, format: format))
            }
            else {
                result = try .wrapping(await coordinator.readImage(jobID: id, sequence: sequence,
                    offset: integer(params, "offset", default: 0, range: 0...512 * 1024 * 1024),
                    length: integer(params, "length", default: 65_536, range: 1...262_144), editID: editID))
            }
        default: return nil
        }
        return RPCResponse(id: request.id, result: result)
    }
    func identifier(_ params: [String: JSONValue], _ key: String) throws -> String {
        guard let value = try optionalIdentifier(params, key) else { throw RPCFailure(code: -32602, message: "Missing \(key).") }
        return value
    }
    func optionalIdentifier(_ params: [String: JSONValue], _ key: String) throws -> String? {
        guard let value = params[key] else { return nil }
        guard case .string(let text) = value, !text.isEmpty, text.utf8.count <= 512, !text.utf8.contains(0) else {
            throw RPCFailure(code: -32602, message: "\(key) must be a nonempty string of at most 512 bytes.")
        }
        return text
    }
}
