import Foundation
import ScannerAPI

public struct GatewayConfiguration: Sendable {
    public var host: String
    public var port: Int
    public var origin: String
    public var authorities: Set<String>
    public init(host: String = "127.0.0.1", port: Int = 8170, allowRemote: Bool = false, publicOrigin: String? = nil) throws {
        guard (1024...65535).contains(port) else { throw ScannerError("configuration", "HTTP port must be 1024–65535.") }
        let loopback = host == "127.0.0.1" || host == "::1"
        let parts = host.split(separator: ".").compactMap { Int($0) }
        let tailscale = parts.count == 4 && parts.map(String.init).joined(separator: ".") == host && parts.allSatisfy { (0...255).contains($0) }
            && parts[0] == 100 && (64...127).contains(parts[1])
        guard loopback || (allowRemote && tailscale) else {
            throw ScannerError("configuration", "Bind to loopback, or explicitly use --allow-remote with a Tailscale IPv4 address (100.64.0.0/10). Wildcard/public binds are rejected.")
        }
        let authority = (host == "::1" ? "[::1]" : host) + ":\(port)"
        let origin = publicOrigin ?? "http://" + authority
        guard let url = URLComponents(string: origin), let urlHost = url.host, url.user == nil, url.password == nil,
              url.path.isEmpty, url.query == nil, url.fragment == nil,
              url.scheme == "https" || (url.scheme == "http" && publicOrigin == nil),
              let authorityStart = origin.range(of: "://"), !urlHost.isEmpty else {
            throw ScannerError("configuration", "public-origin must be an HTTPS origin without credentials, path, query or fragment.")
        }
        self.host = host; self.port = port; self.origin = origin
        authorities = [authority, String(origin[authorityStart.upperBound...])]
        if loopback { authorities.insert("localhost:\(port)") }
    }
}
