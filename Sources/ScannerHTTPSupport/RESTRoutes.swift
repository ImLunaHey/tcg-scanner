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
        if parts == ["events"], method == .get { return try await events(request, path: path, principal: principal) }
        if parts.first == "images", method == .get { try path.only([]); return try await image(parts: parts, request: request, principal: principal) }
        let rpc: String
        var params = body
        var status = 200
        if method == .get {
            switch parts {
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
                } else if parts.count == 3, parts[0] == "batches", parts[2] == "manifest" {
                    rpc = "batches.get"; params = ["id": .string(try uuid(parts[1]))]
                } else { throw GatewayError(404, "notFound", "Unknown API route.") }
            }
        } else {
            try path.only([])
            if parts == ["jobs"], method == .post {
                rpc = "jobs.create"; status = 202
                try only(body, ["requestID", "deviceID", "profileID", "useDeviceSettings", "dpi", "duplex", "format"])
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
                params = ["dpi": .number(300), "duplex": .bool(false), "format": .string("png")].merging(params) { _, v in v }
            } else if parts == ["profiles"], method == .post {
                rpc = "profiles.create"; status = 201
                try only(body, ["id", "name", "dpi", "duplex", "format"])
                guard case .string(let id) = params["id"] else { throw GatewayError(400, "invalidParams", "Profile creation requires a stable UUID id.") }
                params["id"] = .string(try uuid(id))
                params = ["dpi": .number(300), "duplex": .bool(false), "format": .string("png")].merging(params) { _, v in v }
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
