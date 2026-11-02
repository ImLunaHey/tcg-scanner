import ArgumentParser
import Foundation
import ScannerAPI

struct Profiles: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Save scan presets without changing hardware settings.", subcommands: [List.self, Create.self, Show.self, Update.self, Save.self, Delete.self])
    struct List: ParsableCommand {
        @OptionGroup var options: ClientOptions
        func run() throws { _ = try options.perform("profiles.list", params: .object([:])) }
    }
    struct Save: ParsableCommand {
        static let configuration = CommandConfiguration(abstract: "Create a profile, or replace one using --id. Hardware support is checked when scanning.")
        @OptionGroup var options: ClientOptions
        @Argument var name: String
        @Option var id: String?
        @Option var dpi = 600
        @Flag var duplex = false
        @Option var format = "png"
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
        func run() throws { _ = try options.perform("profiles.delete", params: .object(["id": .string(id)])) }
    }
}

extension Profiles {
    struct Create: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument var name: String
        @Option(help: "Stable UUID for retrying profile creation.") var id: String?
        @Option var dpi = 600
        @Flag var duplex = false
        @Option var format = "png"
        func validate() throws {
            if let id, UUID(uuidString: id) == nil { throw ValidationError("id must be a UUID.") }
            guard OriginalFormat(rawValue: format) != nil else { throw ValidationError("Format must be tiff or png.") }
            try ScanProfile(name: name, dpi: dpi).validate()
        }
        func run() throws {
            let profile = ScanProfile(id: id.flatMap(UUID.init(uuidString:)) ?? UUID(), name: name, dpi: dpi, duplex: duplex, format: OriginalFormat(rawValue: format)!)
            _ = try options.perform("profiles.create", params: .wrapping(profile))
        }
    }
    struct Show: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument(help: "Profile UUID or unique name.") var profile: String
        func run() throws { _ = try options.perform("profiles.get", params: .object(["id": .string(profile)])) }
    }
    struct Update: ParsableCommand {
        @OptionGroup var options: ClientOptions
        @Argument(help: "Profile UUID or unique name.") var profile: String
        @Option var name: String?
        @Option var dpi: Int?
        @Flag(inversion: .prefixedNo) var duplex: Bool?
        @Option var format: String?
        func validate() throws {
            guard name != nil || dpi != nil || duplex != nil || format != nil else { throw ValidationError("Provide at least one field to update.") }
            if let dpi, !(150...600).contains(dpi) { throw ValidationError("DPI must be 150–600.") }
            if let format, OriginalFormat(rawValue: format) == nil { throw ValidationError("Format must be tiff or png.") }
        }
        func run() throws {
            let patch = ProfilePatch(name: name, dpi: dpi, duplex: duplex, format: format.flatMap(OriginalFormat.init(rawValue:)))
            guard case .object(var params) = try JSONValue.wrapping(patch) else { return }
            params["id"] = .string(profile)
            _ = try options.perform("profiles.update", params: .object(params))
        }
    }
}
