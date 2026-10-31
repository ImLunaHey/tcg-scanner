import ArgumentParser
import Foundation
import ScannerAPI

struct Jobs: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Persistent job history and cancellation.", subcommands: [List.self, Status.self, Cancel.self])
    struct List: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Option var limit = 50
        @Option var offset = 0
        func validate() throws {
            guard (1...100).contains(limit), (0...1_000_000).contains(offset) else { throw ValidationError("limit must be 1–100 and offset 0–1000000.") }
        }
        func run() throws { _ = try options.perform("jobs.list", params: .object(["limit": .number(Double(limit)), "offset": .number(Double(offset))])) }
    }
    struct Status: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var id: String
        func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("Job ID must be a UUID.") } }
        func run() throws { try printScan(options.perform("scan.status", params: .object(["scanID": .string(id)])).decode(ScanRecord.self), options: options) }
    }
    struct Cancel: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var id: String
        func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("Job ID must be a UUID.") } }
        func run() throws { try printScan(options.perform("scan.cancel", params: .object(["scanID": .string(id)])).decode(ScanRecord.self), options: options) }
    }
}
struct ServiceStatusCommand: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "service-status", abstract: "Read queue and service health without opening a scanner session.")
    @OptionGroup var options: ClientOptions
    func run() throws { _ = try options.perform("service.status", params: .object([:])) }
}
struct Events: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Read replayable events; --follow waits for further pages.")
    @OptionGroup var options: ClientOptions
    @Option var after: Int64 = 0
    @Flag var follow = false
    func validate() throws { guard (0...9_007_199_254_740_991).contains(after) else { throw ValidationError("Invalid event cursor.") } }
    func run() throws {
        var cursor = after
        repeat {
            let page = try options.perform("events", params: .object(["after": .number(Double(cursor)), "waitSeconds": .number(follow ? 20 : 0)])).decode(EventPage.self)
            guard !page.resetRequired else { throw ValidationError("Event history changed or expired; refresh snapshots and restart with --after 0.") }
            cursor = page.nextCursor
        } while follow
    }
}
