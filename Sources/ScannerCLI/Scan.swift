import ArgumentParser
import Foundation
import ScannerAPI

struct Scan: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Scan loaded A4 paper until the feeder is empty (120-second limit).")
    @OptionGroup var options: ClientOptions
    @Option(help: "Output directory inside the service's storage root. Default: storage root.") var output: String?
    @Option(help: "Resolution supported by the driver (150–600 DPI).") var dpi = 300
    @Flag(help: "Scan both sides of each sheet.") var duplex = false
    @Option(help: "Original format: tiff or png.") var format = "tiff"
    @Option(help: "Request UUID to reuse when retrying a start request, including after a restart.") var requestID: String?
    @Option(help: "Saved profile UUID; its DPI, duplex and format replace those scan options.") var profile: String?
    @Flag(help: "Return the scan ID immediately instead of waiting for completion.") var noWait = false
    func validate() throws {
        guard OriginalFormat(rawValue: format) != nil else { throw ValidationError("Format must be tiff or png.") }
        guard (150...600).contains(dpi) else { throw ValidationError("DPI must be between 150 and 600.") }
        if let profile, UUID(uuidString: profile) == nil { throw ValidationError("profile must be a UUID.") }
        if let requestID, UUID(uuidString: requestID) == nil { throw ValidationError("request-id must be a UUID.") }
    }
    func run() throws {
        let directory = output.map { URL(fileURLWithPath: ($0 as NSString).expandingTildeInPath).standardizedFileURL.path }
        let request = ScanRequest(requestID: requestID.flatMap(UUID.init(uuidString:)) ?? UUID(), deviceID: options.device,
                                  outputDirectory: directory, dpi: dpi, duplex: duplex, format: OriginalFormat(rawValue: format)!, profileID: profile.flatMap(UUID.init(uuidString:)))
        let result = try options.perform("scan.start", params: .wrapping(request))
        var record = try result.decode(ScanRecord.self)
        if !options.json { print("Scan \(record.id.uuidString)\nOutput: \(record.outputDirectory)") }
        if !noWait {
            var previous = ""
            while !record.state.terminal {
                let progress = "\(record.state.rawValue): \(record.images.count) image(s) saved"
                if !options.json && progress != previous { print(progress); previous = progress }
                Thread.sleep(forTimeInterval: 0.25)
                record = try options.perform("scan.status", params: .object(["scanID": .string(record.id.uuidString)])).decode(ScanRecord.self)
            }
        }
        try printScan(record, options: options)
    }
}
struct ScanStatus: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scan-status", abstract: "Read a scan's persisted status.")
    @OptionGroup var options: ClientOptions
    @Argument var id: String
    func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("Scan ID must be a UUID.") } }
    func run() throws { try printScan(options.perform("scan.status", params: .object(["scanID": .string(id)])).decode(ScanRecord.self), options: options) }
}
struct ScanCancel: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scan-cancel", abstract: "Request scan cancellation; received originals remain on disk.")
    @OptionGroup var options: ClientOptions
    @Argument var id: String
    func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("Scan ID must be a UUID.") } }
    func run() throws { try printScan(options.perform("scan.cancel", params: .object(["scanID": .string(id)])).decode(ScanRecord.self), options: options) }
}
func printScan(_ record: ScanRecord, options: ClientOptions) throws {
    if options.json { try options.writeJSON(.wrapping(record)) }
    else {
        print("\(record.state.rawValue): \(record.images.count) image(s)\nManifest: \(record.outputDirectory)/manifest.json")
        for e in record.errors { print("Error [\(e.kind)]: \(e.message)") }
    }
    if record.state == .failed { throw ExitCode(1) }
    if record.state == .cancelled { throw ExitCode(6) }
}
