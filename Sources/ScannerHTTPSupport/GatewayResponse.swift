import Foundation
import Hummingbird
import ScannerAPI
import ScannerTransport

struct GatewayError: Error, Sendable {
    var status: Int
    var error: ScannerError
    init(_ status: Int, _ kind: String, _ message: String) { self.status = status; error = ScannerError(kind, message) }
    init(_ error: ScannerError) {
        self.error = error
        switch error.kind {
        case "deviceNotFound", "profileNotFound", "imageNotFound", "scanNotFound", "notFound", "settingsNotFound": status = 404
        case "busy", "conflict", "ambiguousDevice", "storeNotReady", "storeConflict": status = 409
        case "invalidParams", "invalidSettings", "unsupported", "unsupportedSetting", "unsupportedGame", "cardNotFound": status = 422
        case "accessDenied": status = 403
        case "resourceLimit": status = 429
        case "serviceUnavailable", "transport", "storage", "storageUnavailable", "storeNotConfigured": status = 503
        default: status = 500
        }
    }
}
func jsonResponse(_ value: JSONValue, status: Int = 200) throws -> Response {
    Response(status: .init(code: status), headers: [.contentType: "application/json; charset=utf-8"],
             body: .init(byteBuffer: ByteBuffer(bytes: try Wire.encoder().encode(value))))
}
func errorResponse(_ error: any Error) -> Response {
    let failure: GatewayError
    if let e = error as? GatewayError { failure = e }
    else if let e = error as? ScannerError { failure = GatewayError(e) }
    else { failure = GatewayError(500, "internal", "HTTP gateway could not complete the request.") }
    let value: JSONValue = .object(["schemaVersion": .number(1), "error": (try? .wrapping(failure.error)) ?? .null])
    var response = (try? jsonResponse(value, status: failure.status)) ?? Response(status: .internalServerError)
    response.headers[.connection] = "close"
    if failure.status == 401 { response.headers[.wwwAuthenticate] = "Bearer realm=\"scanner\"" }
    if failure.status == 429 { response.headers[.retryAfter] = "60" }
    return response
}

struct ServiceClient: Sendable {
    let client: UnixClient
    func call(_ method: String, _ params: [String: JSONValue] = [:]) async throws -> JSONValue {
        var request = RPCRequest(method: "v1." + method); request.params = .object(params)
        let response = try await client.callAsync(request)
        if let error = response.error {
            if error.code == -32602 { throw GatewayError(400, "invalidParams", error.message) }
            throw GatewayError(error.data ?? ScannerError("service", error.message))
        }
        guard let value = response.result else { throw GatewayError(502, "transport", "Missing service result.") }
        return value
    }
}

struct GatewayPath {
    let components: [String]
    let query: [String: String]
    init(_ target: String) throws {
        guard target.utf8.count <= 8192, target.hasPrefix("/"), !target.hasPrefix("//"),
              let url = URLComponents(string: target), url.fragment == nil, url.host == nil else {
            throw GatewayError(400, "invalidParams", "Invalid request target.")
        }
        let raw = url.percentEncodedPath == "/" ? [] : Array(url.percentEncodedPath.split(separator: "/", omittingEmptySubsequences: false).dropFirst())
        components = try raw.map {
            guard let value = String($0).removingPercentEncoding, !value.isEmpty,
                  value != ".", value != "..", !value.contains("/"), !value.contains("\\"),
                  !value.utf8.contains(0), value.utf8.count <= 512 else {
                throw GatewayError(400, "invalidParams", "Invalid path component.")
            }
            return value
        }
        var query: [String: String] = [:]
        for item in url.queryItems ?? [] {
            guard let value = item.value, query[item.name] == nil else { throw GatewayError(400, "invalidParams", "Invalid or duplicate query parameter.") }
            query[item.name] = value
        }
        self.query = query
    }
    func only(_ names: Set<String>) throws {
        guard query.keys.allSatisfy(names.contains) else { throw GatewayError(400, "invalidParams", "Unknown query parameter.") }
    }
    func number(_ key: String, default fallback: Int, range: ClosedRange<Int>) throws -> Int {
        guard let text = query[key] else { return fallback }
        guard let value = Int(text), range.contains(value) else { throw GatewayError(400, "invalidParams", "Invalid \(key).") }
        return value
    }
}
func uuid(_ value: String) throws -> String {
    guard let id = UUID(uuidString: value) else { throw GatewayError(400, "invalidParams", "Expected UUID.") }
    return id.uuidString
}
func only(_ params: [String: JSONValue], _ fields: Set<String>) throws {
    guard params.keys.allSatisfy(fields.contains) else { throw GatewayError(400, "invalidParams", "Unknown JSON field.") }
}
