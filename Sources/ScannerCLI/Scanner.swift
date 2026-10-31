import ArgumentParser

@main
struct Scanner: ParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "scanner", abstract: "Read-only scanner discovery and diagnostics through ScannerService.",
        version: "0.1.0", subcommands: [Devices.self, Capabilities.self, Settings.self, DiagnosticsCommand.self])
}
struct Devices: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "List local scanners exposed by ImageCaptureCore.")
    @OptionGroup var options: ClientOptions
    func run() throws { try options.perform("devices") }
}
struct Capabilities: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Inspect functional units and driver-exposed capabilities without scanning.")
    @OptionGroup var options: ClientOptions
    func run() throws { try options.perform("capabilities") }
}
struct Settings: ParsableCommand {
    static let configuration = CommandConfiguration(abstract: "Read current settings; no settings are changed.")
    @OptionGroup var options: ClientOptions
    func run() throws { try options.perform("settings") }
}
struct DiagnosticsCommand: ParsableCommand {
    static let configuration = CommandConfiguration(commandName: "diagnostics", abstract: "Report service environment, discovery and session diagnostics.")
    @OptionGroup var options: ClientOptions
    func run() throws { try options.perform("diagnostics") }
}
