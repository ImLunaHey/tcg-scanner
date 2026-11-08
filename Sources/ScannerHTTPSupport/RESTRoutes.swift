import Foundation
import Hummingbird
import HTTPTypes
import ScannerAPI

extension ScannerGateway {
    func route(_ request: Request, path: GatewayPath, body: [String: JSONValue], principal: String) async throws -> Response {
        guard path.components.starts(with: ["api", "v1"]) else { throw GatewayError(404, "notFound", "Unknown API route.") }
        let parts = Array(path.components.dropFirst(2))
        let method = request.method
        if parts == ["session"] {
            try path.only([]); try only(body, [])
            if method == .post {
                let (token, expiry) = try await access.issue(for: principal)
                return try jsonResponse(.object(["schemaVersion": .number(1), "token": .string(token), "expiresAt": .string(ISO8601DateFormatter().string(from: expiry))]), status: 201)
            }
            if method == .delete { try await access.revoke(principal); return try jsonResponse(.object(["revoked": .bool(true)])) }
        }
        if parts.count == 3, parts[0] == "batches", parts[2] == "store" {
            try path.only([]); try only(body, [])
            guard method == .get || method == .post else { throw GatewayError(405, "methodNotAllowed", "Use GET or POST for Store transfers.") }
            return try jsonResponse(await service.call(method == .get ? "batches.storeStatus" : "batches.sendToStore",
                ["id": .string(try uuid(parts[1]))]), status: method == .post ? 202 : 200)
        }
        if parts == ["events"], method == .get { return try await events(request, path: path, principal: principal) }
        if parts.count == 3, parts[0] == "images", parts[2] == "prepare-card", method == .post {
            try path.only([]); try only(body, ["id"])
            var params = try imageParameters(parts[1]); params.merge(body) { _, value in value }
            return try jsonResponse(await service.call("images.prepareCard", params), status: 201)
        }
        if parts.count == 3, parts[0] == "images", parts[2] == "identification" {
            try path.only([])
            var params = try imageParameters(parts[1])
            let rpc: String
            if method == .get { try only(body, []); rpc = "images.identification" }
            else if method == .post { try only(body, ["id", "language"]); rpc = "images.identify" }
            else if method == .patch { try only(body, ["identificationID", "cardID", "finish", "condition"]); rpc = "images.review" }
            else { throw GatewayError(405, "methodNotAllowed", "Use GET, POST or PATCH for identification.") }
            params.merge(body) { _, value in value }
            return try jsonResponse(await service.call(rpc, params), status: method == .post ? 202 : 200)
        }
        if parts.count == 3, parts[0] == "images", parts[2] == "condition-check", method == .post {
            try path.only([]); try only(body, ["identificationID"])
            var params = try imageParameters(parts[1]); params.merge(body) { _, value in value }
            return try jsonResponse(await service.call("images.checkCondition", params))
        }
        if parts.count == 3, parts[0] == "images", parts[2] == "finish-estimate", method == .post {
            try path.only([]); try only(body, ["identificationID"])
            var params = try imageParameters(parts[1]); params.merge(body) { _, value in value }
            return try jsonResponse(await service.call("images.estimateFinish", params))
        }
        if parts.first == "images", method == .get {
            try path.only(parts.count == 3 && parts[2] == "thumbnail" ? ["editID", "format"] : ["editID"])
            return try await image(parts: parts, request: request, principal: principal, editID: path.query["editID"], thumbnailFormat: path.query["format"])
        }
        if parts.count == 3, parts[0] == "images", parts[2] == "edits", method == .post {
            try path.only([]); try only(body, ["id", "crop", "quarterTurns", "deskewDegrees"])
            var params = try imageParameters(parts[1])
            params.merge(body) { _, value in value }
            return try jsonResponse(await service.call("images.edit", params), status: 201)
        }
        let rpc: String
        var params = body
        var status = 200
        if method == .get {
            switch parts {
            case ["inventory"]:
                rpc = "inventory.list"; try path.only(["limit", "offset", "search"])
                params = ["limit": .number(Double(try path.number("limit", default: 50, range: 1...100))),
                          "offset": .number(Double(try path.number("offset", default: 0, range: 0...1_000_000))),
                          "search": .string(path.query["search"] ?? "")]
            case ["devices"]: rpc = "devices"; try path.only([])
            case ["profiles"]: rpc = "profiles.list"; try path.only([])
            case ["jobs"], ["batches"]:
                rpc = parts[0] + ".list"; try path.only(["limit", "offset"])
                params = ["limit": .number(Double(try path.number("limit", default: 50, range: 1...100))), "offset": .number(Double(try path.number("offset", default: 0, range: 0...1_000_000)))]
            case ["diagnostics"]:
                rpc = "diagnostics"; try path.only(["deviceID"])
                if let id = path.query["deviceID"] { params["deviceID"] = .string(id) }
            case ["status"]: rpc = "service.status"; try path.only([])
            default:
                try path.only([])
                if parts.count == 2, parts[0] == "devices" { rpc = "device.get"; params = ["deviceID": .string(parts[1])] }
                else if parts.count == 3, parts[0] == "devices", ["capabilities", "settings"].contains(parts[2]) {
                    rpc = parts[2]; params = ["deviceID": .string(parts[1])]
                } else if parts.count == 2, ["profiles", "jobs", "batches"].contains(parts[0]) {
                    rpc = parts[0] + ".get"; params = ["id": .string(try uuid(parts[1]))]
                } else if parts.count == 3, parts[0] == "batches", parts[2] == "valuation" {
                    rpc = "batches.valuation"; params = ["id": .string(try uuid(parts[1]))]
                } else if parts.count == 3, parts[0] == "batches", parts[2] == "manifest" {
                    rpc = "batches.get"; params = ["id": .string(try uuid(parts[1]))]
                } else { throw GatewayError(404, "notFound", "Unknown API route.") }
            }
        } else {
            try path.only([])
            if parts == ["jobs"], method == .post {
                rpc = "jobs.create"; status = 202
                try only(body, ["requestID", "deviceID", "profileID", "useDeviceSettings", "dpi", "duplex", "format", "prepareCards", "background", "overscan", "cardLanguage"])
                if let key = request.headers[HTTPField.Name("Idempotency-Key")!] {
                    let id = try uuid(key)
                    if let supplied = body["requestID"] {
                        guard case .string(let rawID) = supplied, try uuid(rawID) == id else {
                            throw GatewayError(400, "invalidParams", "Idempotency-Key and requestID must match.")
                        }
                    }
                    params["requestID"] = .string(id)
                }
                guard case .string(let id) = params["requestID"] else { throw GatewayError(400, "invalidParams", "Provide requestID or Idempotency-Key as a UUID.") }
                params["requestID"] = .string(try uuid(id))
                params = ["dpi": .number(600), "duplex": .bool(false), "format": .string("png")].merging(params) { _, v in v }
            } else if parts == ["profiles"], method == .post {
                rpc = "profiles.create"; status = 201
                try only(body, ["id", "name", "dpi", "duplex", "format"])
                guard case .string(let id) = params["id"] else { throw GatewayError(400, "invalidParams", "Profile creation requires a stable UUID id.") }
                params["id"] = .string(try uuid(id))
                params = ["dpi": .number(600), "duplex": .bool(false), "format": .string("png")].merging(params) { _, v in v }
            } else if parts.count == 2, parts[0] == "profiles", method == .patch || method == .delete {
                rpc = method == .patch ? "profiles.update" : "profiles.delete"
                try only(body, method == .patch ? ["name", "dpi", "duplex", "format"] : [])
                params["id"] = .string(try uuid(parts[1]))
            } else if parts.count == 3, parts[0] == "jobs", parts[2] == "cancel", method == .post {
                rpc = "jobs.cancel"; try only(body, []); params["id"] = .string(try uuid(parts[1]))
            } else if parts.count == 3, parts[0] == "devices", parts[2] == "settings", method == .patch {
                rpc = "settings.set"; try only(body, ["dpi", "duplex", "format"]); params["deviceID"] = .string(parts[1])
            } else { throw GatewayError(405, "methodNotAllowed", "Method or route not supported.") }
        }
        let result = try await service.call(rpc, params)
        var response = try jsonResponse(result, status: status)
        if parts.count == 3, parts[0] == "batches", parts[2] == "manifest" {
            response.headers[.contentDisposition] = "attachment; filename=\"batch-\(try uuid(parts[1])).json\""
        }
        return response
    }
}
