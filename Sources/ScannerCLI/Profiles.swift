import ArgumentParser
import Foundation
import ScannerAPI

struct Profiles: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Save scan presets without changing hardware settings.", subcommands: [List.self, Save.self, Delete.self])
    struct List: ParsableCommand {
        @OptionGroup var options: ClientOptions
        func run() throws { _ = try options.perform("profiles.list", params: .object([:])) }
    }
    struct Save: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Create a profile, or replace one using --id. Hardware support is checked when scanning.")
        @OptionGroup var options: ClientOptions
        @Argument var name: String
        @Option var id: String?
        @Option var dpi = 300
        @Flag var duplex = false
        @Option var format = "tiff"
        func validate() throws {
            if let id, UUID(uuidString: id) == nil { throw ValidationError("id must be a UUID.") }
            guard OriginalFormat(rawValue: format) != nil else { throw ValidationError("Format must be tiff or png.") }
            try ScanProfile(name: name, dpi: dpi).validate()
        }
        func run() throws {
            let profile = ScanProfile(id: id.flatMap(UUID.init(uuidString:)) ?? UUID(), name: name, dpi: dpi, duplex: duplex, format: OriginalFormat(rawValue: format)!)
            _ = try options.perform("profiles.save", params: .wrapping(profile))
        }
    }
    struct Delete: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var id: String
        func validate() throws { guard UUID(uuidString: id) != nil else { throw ValidationError("id must be a UUID.") } }
        func run() throws { _ = try options.perform("profiles.delete", params: .object(["id": .string(id)])) }
    }
}
