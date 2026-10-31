import ArgumentParser
import Foundation
import ScannerAPI
import ScannerTransport

struct ClientOptions: ParsableArguments {
    @Flag(help: "Write a schema-versioned JSON result to stdout.") var json = false
    @Flag(name: .shortAndLong, help: "Log local API requests to stderr.") var verbose = false
    @Option(help: "Scanner ID returned by devices; required if multiple scanners are present.") var device: String?
    @Option(help: "Absolute path to the ScannerService Unix socket.") var socket = SocketLocation.defaultPath

    func request(_ command: String, params: JSONValue? = nil) throws -> JSONValue {
        if verbose { stderr("Requesting v1.\(command) via \(socket)") }
        let result: JSONValue
        do {
            var request = RPCRequest(method: "v1." + command, deviceID: device)
            if let params { request.params = params }
            let response = try UnixClient(path: socket).call(request)
            if let error = response.error {
                throw error.data ?? ScannerError("api", error.message, domain: "JSON-RPC", code: error.code)
            }
            guard let value = response.result else { throw ScannerError("transport", "Service returned no result.") }
            result = value
        } catch {
            let failure = (error as? ScannerError) ?? ScannerError("transport", error: error)
            if json { try writeJSON(JSONValue.object(["schemaVersion": .number(1), "error": .wrapping(failure)])) }
            else { stderr("Error [\(failure.kind)]: \(failure.message)") }
            throw ExitCode(Self.exitCode(for: failure.kind))
        }
        return result
    }
    func perform(_ command: String, params: JSONValue? = nil) throws -> JSONValue {
        let result = try request(command, params: params)
        if command.hasPrefix("scan.") { return result }
        if json { try writeJSON(result) }
        else { try HumanOutput.printResult(command: command, result: result) }
        if Self.hasInspectionErrors(result) { throw ExitCode(1) }
        return result
    }
    static func exitCode(for kind: String) -> Int32 {
        switch kind { case "serviceUnavailable": 3; case "deviceNotFound", "ambiguousDevice": 4; case "busy": 5; default: 1 }
    }
    static func hasInspectionErrors(_ value: JSONValue) -> Bool {
        guard case .object(let object) = value else { return false }
        if case .array(let errors) = object["errors"], !errors.isEmpty { return true }
        if let inspection = object["inspection"] { return hasInspectionErrors(inspection) }
        return false
    }
    func writeJSON(_ value: JSONValue) throws {
        let encoder = Wire.encoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        FileHandle.standardOutput.write(try encoder.encode(value))
        FileHandle.standardOutput.write(Data([10]))
    }
    private func stderr(_ text: String) { FileHandle.standardError.write(Data((text + "\n").utf8)) }
}
