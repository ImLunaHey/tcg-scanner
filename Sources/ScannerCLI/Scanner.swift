import ArgumentParser

@main
struct Scanner: ParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "scanner", abstract: "Scanner discovery, diagnostics and basic A4 scanning through ScannerService.",
        version: "0.3.0", subcommands: [Devices.self, Capabilities.self, Settings.self, DiagnosticsCommand.self, Scan.self, ScanStatus.self, ScanCancel.self, Jobs.self, Events.self, ServiceStatusCommand.self, Profiles.self])
}
struct Devices: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "List local scanners exposed by ImageCaptureCore.")
    @OptionGroup var options: ClientOptions
    func run() throws { _ = try options.perform("devices") }
}
struct Capabilities: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Inspect functional units and driver-exposed capabilities without scanning.")
    @OptionGroup var options: ClientOptions
    func run() throws { _ = try options.perform("capabilities") }
}
struct Settings: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Read current settings; no settings are changed.")
    @OptionGroup var options: ClientOptions
    func run() throws { _ = try options.perform("settings") }
}
struct DiagnosticsCommand: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "diagnostics", abstract: "Report service environment, discovery and session diagnostics.")
    @OptionGroup var options: ClientOptions
    func run() throws { _ = try options.perform("diagnostics") }
}
