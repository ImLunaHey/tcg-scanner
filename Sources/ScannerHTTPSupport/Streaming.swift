import CryptoKit
import Foundation
import Hummingbird
import HTTPTypes
import ScannerAPI

extension ScannerGateway {
    func events(_ request: Request, path: GatewayPath, principal: String) async throws -> Response {
        try path.only(["after", "seconds"])
        let duration = try path.number("seconds", default: 25, range: 1...25)
        var after = try path.number("after", default: 0, range: 0...9_007_199_254_740_991)
        if let last = request.headers[HTTPField.Name("Last-Event-ID")!] {
            guard path.query["after"] == nil, let value = Int(last), (0...9_007_199_254_740_991).contains(value) else {
                throw GatewayError(400, "invalidParams", "Use either a valid Last-Event-ID or after cursor.")
            }
            after = value
        }
        let first = try await eventPage(after: after, wait: 0)
        let initialCursor = after
        let body = ResponseBody { writer in
            var cursor = initialCursor
            var page = first
            let deadline = ContinuousClock.now + .seconds(duration)
            try await writer.write(ByteBuffer(string: "retry: 1000\n\n"))
            do {
                while true {
                    try Task.checkCancellation()
                    guard await access.validPrincipal(principal) else { throw GatewayError(401, "unauthorized", "Session expired or revoked.") }
                    if page.resetRequired {
                        let data = String(decoding: try Wire.encoder().encode(page), as: UTF8.self)
                        try await writer.write(ByteBuffer(string: "event: reset\ndata: \(data)\n\n"))
                        break
                    }
                    for event in page.events {
                        let data = String(decoding: try Wire.encoder().encode(event), as: UTF8.self)
                        // Fixed event name avoids treating stored payload strings as SSE control fields.
                        try await writer.write(ByteBuffer(string: "id: \(event.cursor)\nevent: scanner\ndata: \(data)\n\n"))
                    }
                    cursor = Int(page.nextCursor)
                    if page.events.isEmpty { try await writer.write(ByteBuffer(string: ": heartbeat\n\n")) }
                    if ContinuousClock.now >= deadline { break }
                    page = try await eventPage(after: cursor, wait: 1)
                }
            } catch {
                if Task.isCancelled { throw error }
                let failure = (error as? GatewayError)?.error ?? ScannerError("serviceUnavailable", "Event stream interrupted; reconnect with the last received cursor.")
                let data = String(decoding: try Wire.encoder().encode(failure), as: UTF8.self)
                try await writer.write(ByteBuffer(string: "event: error\ndata: \(data)\n\n"))
            }
            try await writer.finish(nil)
        }
        return Response(status: .ok, headers: [.contentType: "text/event-stream", HTTPField.Name("X-Accel-Buffering")!: "no"], body: body)
    }
    private func eventPage(after: Int, wait: Int) async throws -> EventPage {
        try await service.call("events", ["after": .number(Double(after)), "limit": .number(100), "waitSeconds": .number(Double(wait))]).decode(EventPage.self)
    }
    func image(parts: [String], request: Request, principal: String) async throws -> Response {
        guard (2...3).contains(parts.count) else { throw GatewayError(404, "notFound", "Unknown image route.") }
        let identifier = parts[1].split(separator: ":", omittingEmptySubsequences: false)
        guard identifier.count == 2, let sequence = Int(identifier[1]), (1...100).contains(sequence) else {
            throw GatewayError(400, "invalidParams", "Image ID must be JOB_UUID:SEQUENCE.")
        }
        let jobID = try uuid(String(identifier[0]))
        let params: [String: JSONValue] = ["jobID": .string(jobID), "sequence": .number(Double(sequence))]
        if parts.count == 3 {
            if parts[2] == "metadata" { return try jsonResponse(await service.call("images.get", params)) }
            if parts[2] == "thumbnail" {
                let result = try await service.call("images.thumbnail", params).decode(ImageThumbnail.self)
                guard let data = Data(base64Encoded: result.base64), SHA256.hash(data: data).hex == result.sha256 else { throw GatewayError(502, "integrity", "Invalid thumbnail response.") }
                return Response(status: .ok, headers: [.contentType: "image/png"], body: .init(byteBuffer: ByteBuffer(bytes: data)))
            }
            throw GatewayError(404, "notFound", "Unknown image route.")
        }
        guard request.headers[.range] == nil else { throw GatewayError(416, "unsupported", "Range requests are not supported; download the complete original.") }
        let first = try await imageChunk(params, offset: 0)
        let metadata = first.metadata
        let format = metadata.image.uniformType == "public.png" ? "png" : "tiff"
        let body = ResponseBody(contentLength: metadata.image.bytes) { writer in
            var chunk = first
            var offset = 0
            var digest = SHA256()
            let deadline = ContinuousClock.now + .seconds(300)
            while true {
                try Task.checkCancellation()
                guard ContinuousClock.now < deadline, await access.validPrincipal(principal) else { throw GatewayError(401, "unauthorized", "Download expired or session revoked.") }
                guard chunk.metadata.jobID == metadata.jobID, chunk.metadata.image.sha256 == metadata.image.sha256,
                      chunk.metadata.image.bytes == metadata.image.bytes, chunk.offset == offset,
                      let data = Data(base64Encoded: chunk.base64), !data.isEmpty,
                      data.count <= 262_144, chunk.nextOffset == offset + data.count,
                      chunk.nextOffset <= metadata.image.bytes, chunk.eof == (chunk.nextOffset == metadata.image.bytes),
                      SHA256.hash(data: data).hex == chunk.chunkSHA256 else {
                    throw GatewayError(502, "integrity", "Original chunk verification failed.")
                }
                digest.update(data: data)
                if chunk.eof, digest.finalize().hex != metadata.image.sha256 { throw GatewayError(502, "integrity", "Original checksum verification failed.") }
                try await writer.write(ByteBuffer(bytes: data))
                offset = chunk.nextOffset
                if chunk.eof { break }
                chunk = try await imageChunk(params, offset: offset)
            }
            try await writer.finish(nil)
        }
        return Response(status: .ok, headers: [.contentType: "image/\(format)", .eTag: "\"\(metadata.image.sha256)\"",
            HTTPField.Name("X-Image-SHA256")!: metadata.image.sha256,
            .contentDisposition: "attachment; filename=\"\(jobID)-\(sequence).\(format)\""], body: body)
    }
    private func imageChunk(_ params: [String: JSONValue], offset: Int) async throws -> ImageChunk {
        try await service.call("images.read", params.merging(["offset": .number(Double(offset)), "length": .number(262_144)]) { _, value in value }).decode(ImageChunk.self)
    }
}
private extension SHA256.Digest {
    var hex: String { map { String(format: "%02x", $0) }.joined() }
}
