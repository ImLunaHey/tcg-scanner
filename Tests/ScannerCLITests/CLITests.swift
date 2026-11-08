import Testing
import Foundation
import ScannerAPI
@testable import ScannerCLI

@Test func identificationCLIRequiresExplicitReviewAndStableIDs() throws {
    let job = UUID().uuidString; let id = UUID().uuidString
    let start = try Scanner.parseAsRoot(["images", "identify", job, "1", "--request-id", id, "--json"]) as! Images.Identify
    #expect(start.requestID == id && start.options.json)
    #expect(try Scanner.parseAsRoot(["images", "prepare-card", job, "1", "--request-id", id]) is Images.PrepareCard)
    #expect(try Scanner.parseAsRoot(["images", "check-condition", job, "1", "--identification-id", id, "--json"]) is Images.CheckCondition)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["images", "check-condition", job, "1"]) }
    let scan = try Scanner.parseAsRoot(["scan", "--prepare-cards"]) as! Scan
    #expect(scan.prepareCards)
    #expect(try Scanner.parseAsRoot(["images", "identification", job, "1"]) is Images.Identification)
    #expect(try Scanner.parseAsRoot(["images", "review", job, "1", "--identification-id", id, "--reject"]) is Images.Review)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["images", "review", job, "1", "--identification-id", id]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["images", "review", job, "1", "--identification-id", id, "--reject", "--card-id", "swsh3-136"]) }
}

@Test func parsesDiagnosticCommandsAndOptions() throws {
    let devices = try Scanner.parseAsRoot(["devices", "--json", "--verbose", "--socket", "/tmp/private/sock"]) as! Devices
    #expect(devices.options.json)
    #expect(devices.options.verbose)
    #expect(devices.options.socket == "/tmp/private/sock")
    let caps = try Scanner.parseAsRoot(["capabilities", "--device", "abc"]) as! Capabilities
    #expect(caps.options.device == "abc")
    #expect(try Scanner.parseAsRoot(["settings"]) is Settings)
    #expect(try Scanner.parseAsRoot(["diagnostics"]) is DiagnosticsCommand)
    let scan = try Scanner.parseAsRoot(["scan", "--duplex", "--format", "png", "--dpi", "300"]) as! Scan
    #expect(scan.duplex)
    #expect(scan.format == "png")
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["scan", "--format", "jpeg"]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["scan", "--dpi", "-1"]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["settings", "--duplex"]) }
}
@Test func stableExitCodesAndPartialInspectionErrors() {
    #expect(ClientOptions.exitCode(for: "serviceUnavailable") == 3)
    #expect(ClientOptions.exitCode(for: "deviceNotFound") == 4)
    #expect(ClientOptions.exitCode(for: "busy") == 5)
    #expect(ClientOptions.hasInspectionErrors(.object(["inspection": .object(["errors": .array([.string("failed")])])])))
    #expect(!ClientOptions.hasInspectionErrors(.object(["errors": .array([])])))
}

@Test func parsesPersistentJobsProfilesAndEvents() throws {
    #expect(try Scanner.parseAsRoot(["jobs", "list", "--json"]) is Jobs.List)
    #expect(try Scanner.parseAsRoot(["service-status"]) is ServiceStatusCommand)
    #expect(try Scanner.parseAsRoot(["events", "--follow", "--after", "12"]) is Events)
    #expect(try Scanner.parseAsRoot(["profiles", "save", "Documents", "--duplex"]) is Profiles.Save)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["jobs", "list", "--limit", "101"]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["events", "--after", "-1"]) }
    #expect(try Scanner.parseAsRoot(["scan", "--profile", "pokemon"]) is Scan)
}

@Test func parsesCompleteProfilesLibraryAndConfigurationCommands() throws {
    #expect(try Scanner.parseAsRoot(["profiles", "create", "pokemon"]) is Profiles.Create)
    #expect(try Scanner.parseAsRoot(["profiles", "show", "pokemon"]) is Profiles.Show)
    let update = try Scanner.parseAsRoot(["profiles", "update", "pokemon", "--no-duplex"]) as! Profiles.Update
    #expect(update.duplex == false)
    #expect(update.dpi == nil && update.format == nil)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["profiles", "update", "pokemon"]) }
    #expect(try Scanner.parseAsRoot(["batches", "list", "--json"]) is Batches.List)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["batches", "list", "--limit", "101"]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["images", "metadata", "bad", "1"]) }
    #expect(try Scanner.parseAsRoot(["configure", "--dpi", "600", "--duplex"]) is Configure)
    #expect(try Scanner.parseAsRoot(["device", "--json"]) is DeviceCommand)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["scan", "--profile", "pokemon", "--use-device-settings"]) }
}

@Test func continuingBatchCLIUsesStableRequestWithoutNewSettings() throws {
    let id = UUID().uuidString, key = UUID().uuidString
    let command = try Scanner.parseAsRoot(["batches", "continue", id, "--request-id", key, "--no-wait", "--json"]) as! Batches.Continue
    #expect(command.id == id && command.requestID == key && command.noWait && command.options.json)
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["batches", "continue", id, "--dpi", "300"]) }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["batches", "continue", "bad"]) }
    #expect(try Scanner.parseAsRoot(["images", "metadata", id, "102"]) is Images.Metadata)
}
