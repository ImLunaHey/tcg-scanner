import Foundation
import Hummingbird
import HTTPTypes
import ScannerAPI
import ScannerTransport

/// HTTP-only adapter. All scanner, profile, storage and scheduling operations use IPC.
public struct ScannerGateway: HTTPResponder {
    public typealias Context = BasicRequestContext
    let service: ServiceClient
    let access: GatewayAccess
    let configuration: GatewayConfiguration
    let webAssets: WebAssets?
    public init(socketPath: String = SocketLocation.defaultPath, token: String, configuration: GatewayConfiguration, webAssets: WebAssets? = nil) throws {
        self.service = ServiceClient(client: UnixClient(path: socketPath))
        self.access = try GatewayAccess(token: token)
        self.configuration = configuration
        self.webAssets = webAssets
    }
    public func respond(to request: Request, context: BasicRequestContext) async throws -> Response {
        var response: Response
        var isWebAsset = false
        do {
            guard let authority = request.head.authority, configuration.authorities.contains(authority.lowercased()) else {
                throw GatewayError(403, "accessDenied", "Unrecognized HTTP authority.")
            }
            if let origin = request.headers[.origin], origin != configuration.origin {
                throw GatewayError(403, "accessDenied", "Cross-origin requests are not allowed.")
            }
            guard request.headers[.cookie] == nil, request.headers[values: .authorization].count <= 1 else {
                throw GatewayError(400, "invalidParams", "Use one Bearer authorization header; cookies are not accepted.")
            }
            let path = try GatewayPath(request.head.path ?? "")
            if let asset = try webAssets?.response(for: request, path: path) {
                response = asset; isWebAsset = true
            } else {
            let principal = try await access.authenticate(request.headers[.authorization])
            let kind = path.components == ["api", "v1", "events"] ? "events" : (path.components.starts(with: ["api", "v1", "images"]) ? "image" : "request")
            try await access.acquire(kind)
            let lease = RequestLease(access: access, kind: kind)
            do {
                let body = try await collect(request)
                guard await access.validPrincipal(principal) else { throw GatewayError(401, "unauthorized", "Session expired or revoked.") }
                response = try await route(request, path: path, body: body, principal: principal)
                // Keep limits charged until streaming finishes, including disconnect/write failures.
                let original = response.body
                response.body = ResponseBody(contentLength: original.contentLength) { writer in
                    do { try await original.write(writer); await lease.finish() }
                    catch { await lease.finish(); throw error }
                }
            } catch {
                await lease.finish(); throw error
            }
            }
        } catch { response = errorResponse(error) }
        if response.headers[.cacheControl] == nil { response.headers[.cacheControl] = "no-store" }
        response.headers[HTTPField.Name("X-Content-Type-Options")!] = "nosniff"
        response.headers[HTTPField.Name("Referrer-Policy")!] = "no-referrer"
        response.headers[HTTPField.Name("Content-Security-Policy")!] = isWebAsset ? WebAssets.contentPolicy : "default-src 'none'; frame-ancestors 'none'"
        return response
    }
    func collect(_ request: Request) async throws -> [String: JSONValue] {
        if let encoding = request.headers[.contentEncoding], encoding != "identity" { throw GatewayError(415, "invalidParams", "Compressed request bodies are not accepted.") }
        let bytes: ByteBuffer
        do {
            bytes = try await withThrowingTaskGroup(of: ByteBuffer.self) { group in
                group.addTask { try await request.body.collect(upTo: 60 * 1024) }
                group.addTask { try await Task.sleep(for: .seconds(5)); throw GatewayError(408, "timeout", "Request body timed out.") }
                defer { group.cancelAll() }
                return try await group.next()!
            }
        } catch let error as GatewayError { throw error }
        catch { throw GatewayError(413, "resourceLimit", "Request body exceeds 60 KiB or cannot be read.") }
        guard bytes.readableBytes > 0 else { return [:] }
        guard request.method == .post || request.method == .patch else { throw GatewayError(400, "invalidParams", "This method does not accept a body.") }
        guard request.headers[.contentType]?.split(separator: ";").first?.trimmingCharacters(in: .whitespaces).lowercased() == "application/json" else {
            throw GatewayError(415, "invalidParams", "Use application/json.")
        }
        guard let value = try? Wire.decoder().decode(JSONValue.self, from: Data(bytes.readableBytesView)), case .object(let params) = value else {
            throw GatewayError(400, "invalidParams", "Expected a JSON object.")
        }
        return params
    }
}
