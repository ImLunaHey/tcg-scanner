import Foundation
import MCP
import ScannerAPI
import ScannerTransport

/// Thin MCP adapter. ScannerService owns every hardware operation and storage read.
public actor ScannerMCPServer {
    public let server: Server
    private let client: UnixClient
    private let readOnly: Bool
    private var activeCalls = 0
    public init(socketPath: String = SocketLocation.defaultPath, readOnly: Bool = false) {
        self.client = UnixClient(path: socketPath); self.readOnly = readOnly
        server = Server(name: "ScannerMCP", version: "0.4.0", instructions:
            "All scanning is owned by ScannerService. Discovery/diagnostics never start a scan. Check capabilities and readiness, obtain confirmation of loaded appropriate media, then explicitly call scanner_start_scan. Reuse requestID on retries and poll terminal job state. Never assume all trading cards are safe to feed. MCP disconnect/cancellation does not cancel a service job; use scanner_cancel_job.",
            capabilities: .init(tools: .init(listChanged: false)), configuration: .strict)
    }
    public func start(transport: any Transport) async throws {
        await server.withMethodHandler(ListTools.self) { [weak self] params in
            guard let self else { throw MCPError.internalError("Server stopped.") }
            guard params.cursor == nil else { throw MCPError.invalidParams("Tool listing has no additional pages.") }
            return ListTools.Result(tools: await self.tools())
        }
        await server.withMethodHandler(CallTool.self) { [weak self] params in
            guard let self else { throw MCPError.internalError("Server stopped.") }
            return try await self.call(params)
        }
        try await server.start(transport: transport)
    }
    public func runStdio() async throws {
        try await start(transport: StdioTransport())
        await server.waitUntilCompleted()
        await server.stop()
    }
    public func stop() async { await server.stop() }
    public func tools() -> [Tool] { ToolCatalog.all.filter { !readOnly || !$0.changesState }.map(\.tool) }
    public func call(_ params: CallTool.Parameters) async throws -> CallTool.Result {
        guard let tool = ToolCatalog.all.first(where: { $0.name == params.name }) else { throw MCPError.invalidParams("Unknown scanner tool.") }
        if readOnly && tool.changesState { return try failure(ScannerError("accessDenied", "This MCP process is read-only.")) }
        guard activeCalls < 8 else { return try failure(ScannerError("resourceLimit", "At most eight tool calls may run concurrently.")) }
        activeCalls += 1
        defer { activeCalls -= 1 }
        do {
            var request = RPCRequest(method: "v1." + tool.method)
            request.params = try tool.parameters(params.arguments)
            let response = try await client.callAsync(request)
            if let error = response.error { return try failure(error.data ?? ScannerError("api", error.message, domain: "JSON-RPC", code: error.code)) }
            guard let value = response.result else { throw ScannerError("transport", "Service returned no result.") }
            return try result(.object(["schemaVersion": .number(1), "ok": .bool(true), "data": value]), isError: false)
        } catch let error as ScannerError { return try failure(error) }
        catch { return try failure(ScannerError("transport", error: error)) }
    }
    private func failure(_ error: ScannerError) throws -> CallTool.Result {
        try result(.object(["schemaVersion": .number(1), "ok": .bool(false), "error": .wrapping(error)]), isError: true)
    }
    private func result(_ value: JSONValue, isError: Bool) throws -> CallTool.Result {
        // Text JSON keeps older clients usable; structuredContent carries the same object.
        let bytes = try Wire.encoder().encode(value)
        let structured = try JSONDecoder().decode(Value.self, from: bytes)
        return CallTool.Result(content: [.text(text: String(decoding: bytes, as: UTF8.self), annotations: nil, _meta: nil)], structuredContent: Optional.some(structured), isError: isError)
    }
}
