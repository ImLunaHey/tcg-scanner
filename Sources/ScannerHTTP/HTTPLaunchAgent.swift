import Foundation
import ScannerAPI

extension HTTPMain {
    func writeLaunchAgent(to path: String) throws {
        guard path.hasPrefix("/"), tokenFile.hasPrefix("/"), socket.hasPrefix("/") else {
            throw ScannerError("configuration", "LaunchAgent, token and socket paths must be absolute.")
        }
        let executable = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL.path
        let logs = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/Scanner")
        try FileManager.default.createDirectory(at: logs, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        var arguments = [executable, "--socket", socket, "--host", host, "--port", String(port), "--token-file", tokenFile]
        if allowRemote { arguments.append("--allow-remote") }
        if let publicOrigin { arguments += ["--public-origin", publicOrigin] }
        let plist: [String: Any] = ["Label": "local.scanner.http", "ProgramArguments": arguments,
            "RunAtLoad": true, "KeepAlive": true, "ThrottleInterval": 10, "ExitTimeOut": 60,
            "ProcessType": "Background", "LimitLoadToSessionType": "Aqua", "Umask": 63,
            "StandardOutPath": logs.appendingPathComponent("http-stdout.log").path,
            "StandardErrorPath": logs.appendingPathComponent("http-stderr.log").path]
        try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
            .write(to: URL(fileURLWithPath: path), options: .withoutOverwriting)
    }
}
