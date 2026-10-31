import CryptoKit
import Foundation
import ScannerAPI
import ScannerTransport

/// Typed service client. No hardware access or scanner configuration policy lives here.
public struct ScannerClient: Sendable {
    private let send: @Sendable (RPCRequest) async throws -> RPCResponse
    public init(path: String = SocketLocation.defaultPath) {
        let transport = UnixClient(path: path)
        send = { try await transport.callAsync($0) }
    }
    public init(send: @escaping @Sendable (RPCRequest) async throws -> RPCResponse) { self.send = send }
    public func call<T: Decodable & Sendable>(_ method: String, params: JSONValue? = nil, as type: T.Type = T.self) async throws -> T {
        try Task.checkCancellation()
        var request = RPCRequest(method: method); request.params = params
        let response = try await send(request)
        guard response.id == request.id, response.jsonrpc == "2.0" else { throw ScannerError("transport", "Mismatched service response.") }
        if let error = response.error { throw error.data ?? ScannerError("service", error.message) }
        guard let value = response.result else { throw ScannerError("transport", "Empty service response.") }
        return try value.decode(type)
    }
    public func devices() async throws -> [Device] {
        struct List: Decodable, Sendable { var devices: [Device] }
        return try await call("v1.devices", as: List.self).devices
    }
    public func profiles() async throws -> [ScanProfile] {
        struct List: Decodable, Sendable { var profiles: [ScanProfile] }
        return try await call("v1.profiles.list", as: List.self).profiles
    }
    public func status() async throws -> ServiceStatus { try await call("v1.service.status") }
    public func inspect(_ id: String) async throws -> Inspection { try await call("v1.capabilities", params: .object(["deviceID": .string(id)])) }
    public func device(_ id: String) async throws -> DeviceStatus { try await call("v1.device.get", params: .object(["deviceID": .string(id)])) }
    public func jobs(offset: Int = 0) async throws -> JobPage { try await call("v1.jobs.list", params: .object(["limit": .number(50), "offset": .number(Double(offset))])) }
    public func job(_ id: UUID) async throws -> ScanRecord { try await call("v1.jobs.get", params: .object(["id": .string(id.uuidString)])) }
    public func cancel(_ id: UUID) async throws -> ScanRecord { try await call("v1.jobs.cancel", params: .object(["id": .string(id.uuidString)])) }
    public func start(_ request: ScanRequest) async throws -> ScanRecord { try await call("v1.jobs.create", params: .wrapping(request)) }
    public func events(after: Int64) async throws -> EventPage { try await call("v1.events", params: .object(["after": .number(Double(after)), "waitSeconds": .number(10)])) }
    public func save(_ profile: ScanProfile, creating: Bool) async throws -> ScanProfile {
        try await call(creating ? "v1.profiles.create" : "v1.profiles.update", params: .wrapping(profile))
    }
    public func deleteProfile(_ id: UUID) async throws {
        let _: JSONValue = try await call("v1.profiles.delete", params: .object(["id": .string(id.uuidString)]))
    }
    public func setSettings(_ settings: ScanSettings, deviceID: String) async throws -> DeviceStatus {
        guard case .object(var params) = try JSONValue.wrapping(settings) else { throw ScannerError("encoding", "Invalid settings.") }
        params["deviceID"] = .string(deviceID)
        return try await call("v1.settings.set", params: .object(params))
    }
    public func edit(_ request: ImageEditRequest) async throws -> ProcessedImage { try await call("v1.images.edit", params: .wrapping(request)) }
    public func prepareCard(jobID: UUID, sequence: Int, id: UUID) async throws -> ProcessedImage {
        try await call("v1.images.prepareCard", params: .object(["jobID": .string(jobID.uuidString), "sequence": .number(Double(sequence)), "id": .string(id.uuidString)]))
    }
    public func identify(_ request: IdentificationRequest) async throws -> CardIdentification { try await call("v1.images.identify", params: .wrapping(request)) }
    public func review(jobID: UUID, sequence: Int, identificationID: UUID, cardID: String?, finish: CardFinish, condition: CardCondition) async throws -> CardIdentification {
        var params: [String: JSONValue] = ["jobID": .string(jobID.uuidString), "sequence": .number(Double(sequence)),
            "identificationID": .string(identificationID.uuidString), "finish": .string(finish.rawValue), "condition": .string(condition.rawValue)]
        if let cardID { params["cardID"] = .string(cardID) }
        return try await call("v1.images.review", params: .object(params))
    }
    public func thumbnail(jobID: UUID, sequence: Int, editID: UUID? = nil) async throws -> Data {
        let result: ImageThumbnail = try await call("v1.images.thumbnail", params: .object(imageParams(jobID, sequence, editID)))
        guard let data = Data(base64Encoded: result.base64), hash(data) == result.sha256 else { throw ScannerError("integrity", "Thumbnail checksum mismatch.") }
        return data
    }
    private func imageParams(_ job: UUID, _ sequence: Int, _ edit: UUID?) -> [String: JSONValue] {
        var params: [String: JSONValue] = ["jobID": .string(job.uuidString), "sequence": .number(Double(sequence))]
        if let edit { params["editID"] = .string(edit.uuidString) }
        return params
    }
    /// Streams verified chunks into a private temporary file, then publishes without replacement.
    /// The destination is selected locally by the user, never accepted from the server.
    public func export(jobID: UUID, sequence: Int, editID: UUID? = nil, to destination: URL) async throws {
        let params = imageParams(jobID, sequence, editID)
        let metadata: ImageMetadata = try await call("v1.images.get", params: .object(params))
        guard metadata.image.bytes > 0, metadata.image.bytes <= 512 * 1024 * 1024 else { throw ScannerError("resourceLimit", "Invalid image size.") }
        let temporary = destination.deletingLastPathComponent().appendingPathComponent(".scanner-export-" + UUID().uuidString)
        try Data().write(to: temporary, options: .withoutOverwriting)
        defer { try? FileManager.default.removeItem(at: temporary) }
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: temporary.path)
        let handle = try FileHandle(forWritingTo: temporary)
        defer { try? handle.close() }
        var offset = 0
        var digest = SHA256()
        while offset < metadata.image.bytes {
            try Task.checkCancellation()
            var chunkParams = params
            chunkParams["offset"] = .number(Double(offset)); chunkParams["length"] = .number(262_144)
            let chunk: ImageChunk = try await call("v1.images.read", params: .object(chunkParams))
            guard let bytes = Data(base64Encoded: chunk.base64), !bytes.isEmpty, bytes.count <= 262_144,
                  chunk.offset == offset, chunk.nextOffset == offset + bytes.count,
                  chunk.nextOffset <= metadata.image.bytes, chunk.metadata.jobID == jobID,
                  chunk.metadata.image.sha256 == metadata.image.sha256,
                  chunk.metadata.image.bytes == metadata.image.bytes,
                  chunk.eof == (chunk.nextOffset == metadata.image.bytes), hash(bytes) == chunk.chunkSHA256 else {
                throw ScannerError("integrity", "Image chunk failed validation.")
            }
            try handle.write(contentsOf: bytes); digest.update(data: bytes); offset = chunk.nextOffset
        }
        guard digest.finalize().map({ String(format: "%02x", $0) }).joined() == metadata.image.sha256 else { throw ScannerError("integrity", "Downloaded image checksum mismatch.") }
        try handle.synchronize(); try handle.close()
        try FileManager.default.moveItem(at: temporary, to: destination)
    }
}

private func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
