import Foundation
import ScannerAPI

/// Generates configuration only. Installation and launchctl are deployment operations.
enum LaunchAgent {
    static func write(options: ServiceOptions, to path: String) throws {
        guard path.hasPrefix("/") else { throw ScannerError("invalidSettings", "LaunchAgent plist path must be absolute.") }
        let executable = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL.path
        let logs = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/Scanner")
        try FileManager.default.createDirectory(at: logs, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        var arguments = [executable, "--database", options.database, "--socket", options.socket]
        if let root = options.storageRoot { arguments += ["--storage-root", root] }
        if let share = options.requiredSMBShare { arguments += ["--required-smb-share", share] }
        if options.verbose { arguments.append("--verbose") }
        let plist: [String: Any] = ["Label": "local.scanner.service", "ProgramArguments": arguments,
            "RunAtLoad": true, "KeepAlive": true, "ThrottleInterval": 10, "ExitTimeOut": 60,
            "ProcessType": "Interactive", "LimitLoadToSessionType": "Aqua", "Umask": 63,
            "StandardOutPath": logs.appendingPathComponent("stdout.log").path,
            "StandardErrorPath": logs.appendingPathComponent("service.jsonl").path]
        let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        try data.write(to: URL(fileURLWithPath: path), options: .withoutOverwriting)
    }
}
