import ArgumentParser
import CryptoKit
import Foundation
import ScannerAPI

struct DeviceCommand: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "device", abstract: "Inspect scanner readiness and configured defaults without scanning.")
    @OptionGroup var options: ClientOptions
    func run() throws { _ = try options.perform("device.get") }
}
struct Configure: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Save validated defaults for future scans using --use-device-settings. No hardware writes.")
    @OptionGroup var options: ClientOptions
    @Option var dpi = 300
    @Flag var duplex = false
    @Option var format = "png"
    func validate() throws {
        guard (150...600).contains(dpi), OriginalFormat(rawValue: format) != nil else { throw ValidationError("Use DPI 150–600 and format tiff/png.") }
    }
    func run() throws {
        var params: [String: JSONValue] = ["dpi": .number(Double(dpi)), "duplex": .bool(duplex), "format": .string(format)]
        if let id = options.device { params["deviceID"] = .string(id) }
        _ = try options.perform("settings.set", params: .object(params))
    }
}
struct Batches: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "List batches or retrieve manifests.", subcommands: [List.self, Show.self, Manifest.self])
    struct List: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Option var limit = 50
        @Option var offset = 0
        func validate() throws {
            guard (1...100).contains(limit), (0...1_000_000).contains(offset) else { throw ValidationError("limit must be 1–100 and offset 0–1000000.") }
        }
        func run() throws { _ = try options.perform("batches.list", params: .object(["limit": .number(Double(limit)), "offset": .number(Double(offset))])) }
    }
    struct Show: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var id: String
        func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("Batch ID must be a UUID.") } }
        func run() throws { _ = try options.perform("batches.get", params: .object(["id": .string(id)])) }
    }
    struct Manifest: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var id: String
        func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("Batch ID must be a UUID.") } }
        func run() throws { try options.writeJSON(options.request("batches.get", params: .object(["id": .string(id)]))) }
    }
}
struct Images: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Inspect, identify or download a saved original by job ID and page sequence.", subcommands: [Metadata.self, Download.self, Identify.self, Identification.self, Review.self, PrepareCard.self, EstimateFinish.self, CheckCondition.self])
    struct Metadata: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        func validate() throws {
            guard UUID(uuidString: jobID) != nil, (1...100).contains(sequence) else { throw ValidationError("Provide a job UUID and sequence 1–100.") }
        }
        func run() throws { _ = try options.perform("images.get", params: .object(["jobID": .string(jobID), "sequence": .number(Double(sequence))])) }
    }
    struct Download: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var jobID: String
        @Argument var sequence: Int
        @Option(help: "Destination file; existing files are never replaced.") var output: String
        func validate() throws {
            guard UUID(uuidString: jobID) != nil, (1...100).contains(sequence) else { throw ValidationError("Provide a job UUID and sequence 1–100.") }
        }
        func run() throws {
            do {
                let destination = URL(fileURLWithPath: (output as NSString).expandingTildeInPath).standardizedFileURL
                let parameters: [String: JSONValue] = ["jobID": .string(jobID), "sequence": .number(Double(sequence))]
                let metadata = try options.request("images.get", params: .object(parameters)).decode(ImageMetadata.self)
                try OriginalDownload.write(to: destination, metadata: metadata) { offset in
                    var params = parameters; params["offset"] = .number(Double(offset)); params["length"] = .number(262_144)
                    return try options.request("images.read", params: .object(params)).decode(ImageChunk.self)
                }
                if options.json { try options.writeJSON(.object(["schemaVersion": .number(1), "output": .string(destination.path), "sha256": .string(metadata.image.sha256)])) }
                else { print("Saved \(destination.path) (SHA-256 verified)") }
            } catch let code as ExitCode { throw code }
            catch {
                if options.json { try options.writeJSON(.object(["schemaVersion": .number(1), "error": .wrapping(ScannerError("download", error: error))])) }
                else { FileHandle.standardError.write(Data("Download failed: \(error.localizedDescription)\n".utf8)) }
                throw ExitCode(1)
            }
        }
    }
}

/// Local CLI export only; the service/MCP never accepts an export filesystem path.
enum OriginalDownload {
    static func write(to destination: URL, metadata: ImageMetadata, read: (Int) throws -> ImageChunk) throws {
        guard !FileManager.default.fileExists(atPath: destination.path) else { throw ScannerError("conflict", "Destination already exists.") }
        let staging = destination.deletingLastPathComponent().appendingPathComponent(".scanner-download-" + UUID().uuidString)
        try Data().write(to: staging, options: .withoutOverwriting)
        defer { try? FileManager.default.removeItem(at: staging) }
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: staging.path)
        let file = try FileHandle(forWritingTo: staging)
        defer { try? file.close() }
        var digest = SHA256(); var offset = 0
        while offset < metadata.image.bytes {
            let chunk = try read(offset)
            guard chunk.metadata.jobID == metadata.jobID, chunk.metadata.image.sequence == metadata.image.sequence,
                  chunk.metadata.image.sha256 == metadata.image.sha256,
                  let data = Data(base64Encoded: chunk.base64), !data.isEmpty, data.count <= 262_144,
                  chunk.offset == offset, chunk.nextOffset == offset + data.count, chunk.nextOffset <= metadata.image.bytes,
                  chunk.eof == (chunk.nextOffset == metadata.image.bytes),
                  hex(SHA256.hash(data: data)) == chunk.chunkSHA256 else { throw ScannerError("download", "Invalid or inconsistent image chunk.") }
            try file.write(contentsOf: data); digest.update(data: data); offset = chunk.nextOffset
        }
        guard hex(digest.finalize()) == metadata.image.sha256 else { throw ScannerError("download", "Original checksum mismatch.") }
        try file.synchronize(); try file.close()
        // moveItem refuses collisions, including a file created during download.
        try FileManager.default.moveItem(at: staging, to: destination)
    }
    private static func hex(_ hash: SHA256.Digest) -> String { hash.map { String(format: "%02x", $0) }.joined() }
}
