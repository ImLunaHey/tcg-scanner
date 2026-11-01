import Foundation
import ScannerAPI

private final class StoreRedirectPolicy: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping @Sendable (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}

public struct StoreClient: StoreStaging {
    public let origin: String
    public let scannerInstanceID: String
    private let token: String
    private let send: @Sendable (URLRequest) async throws -> (Data, Int)
    public init(configuration: StoreConfiguration) throws {
        let url = try configuration.validatedURL()
        origin = url.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        scannerInstanceID = configuration.scannerInstanceID
        token = try configuration.token()
        let settings = URLSessionConfiguration.ephemeral
        settings.httpShouldSetCookies = false; settings.httpCookieStorage = nil; settings.urlCache = nil
        settings.timeoutIntervalForRequest = 45; settings.timeoutIntervalForResource = 90
        let session = URLSession(configuration: settings, delegate: StoreRedirectPolicy(), delegateQueue: nil)
        send = { request in
            let (bytes, response) = try await session.bytes(for: request)
            guard let response = response as? HTTPURLResponse else { throw ScannerError("storeResponse", "Store returned an invalid response.") }
            var data = Data()
            for try await byte in bytes {
                guard data.count < 2 * 1024 * 1024 else { throw ScannerError("storeResponse", "Store response exceeds the allowed size.") }
                data.append(byte)
            }
            return (data, response.statusCode)
        }
    }
    init(origin: String, scannerInstanceID: String, token: String, send: @escaping @Sendable (URLRequest) async throws -> (Data, Int)) {
        self.origin = origin; self.scannerInstanceID = scannerInstanceID; self.token = token; self.send = send
    }
    public func importBatch(_ value: StoreImport) async throws -> StoreImportReply {
        try await request("/api/v1/admin/scan-batches", body: Wire.encoder().encode(value), contentType: "application/json")
    }
    public func upload(card: StoreCardReply, front: Data, back: Data) async throws -> StoreCardReply {
        guard !front.isEmpty, !back.isEmpty, front.count <= 10 * 1024 * 1024, back.count <= 10 * 1024 * 1024 else {
            throw ScannerError("resourceLimit", "Store accepts at most 10 MiB per prepared PNG.")
        }
        let boundary = "scanner-" + UUID().uuidString
        var data = Data()
        func text(_ value: String) { data.append(Data(value.utf8)) }
        text("--\(boundary)\r\nContent-Disposition: form-data; name=\"revision\"\r\n\r\n\(card.revision)\r\n")
        for (side, bytes) in [("front", front), ("back", back)] {
            text("--\(boundary)\r\nContent-Disposition: form-data; name=\"\(side)\"; filename=\"\(side).png\"\r\nContent-Type: image/png\r\n\r\n")
            data.append(bytes); text("\r\n")
        }
        text("--\(boundary)--\r\n")
        return try await request("/api/v1/admin/scan-cards/\(card.id.uuidString)/photos", body: data, contentType: "multipart/form-data; boundary=\(boundary)")
    }
    private func request<T: Decodable>(_ path: String, body: Data, contentType: String) async throws -> T {
        var request = URLRequest(url: URL(string: origin + path)!)
        request.httpMethod = "POST"; request.httpBody = body
        request.setValue("Bearer " + token, forHTTPHeaderField: "Authorization")
        request.setValue(contentType, forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        do {
            let (data, status) = try await send(request)
            guard status == 200 else {
                // Never relay an upstream body which could reflect credentials or private data.
                throw ScannerError(status == 409 ? "storeConflict" : "storeHTTP", "Store returned HTTP \(status). \(status == 409 ? "The staged card changed; review it in Store." : "Check Store access and retry sending the batch.")")
            }
            guard let value = try? Wire.decoder().decode(T.self, from: data) else { throw ScannerError("storeResponse", "Store returned an unexpected staging response.") }
            return value
        } catch let error as ScannerError { throw error }
        catch { throw ScannerError("storeUnavailable", "Store could not be reached. Retry the transfer; scans are preserved.") }
    }
}
