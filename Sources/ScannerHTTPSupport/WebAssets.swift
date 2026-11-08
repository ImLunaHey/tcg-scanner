import Darwin
import Foundation
import Hummingbird
import HTTPTypes
import ScannerAPI

/// A bounded, immutable snapshot of public build assets. Scan storage is never served.
public struct WebAssets: Sendable {
    struct Asset: Sendable { let data: Data; let type: String }
    private var files: [String: Asset] = [:]
    public init(directory: String) throws {
        let root = URL(fileURLWithPath: directory).standardizedFileURL
        guard directory.hasPrefix("/"), root.resolvingSymlinksInPath() == root,
              let walker = FileManager.default.enumerator(atPath: root.path) else {
            throw ScannerError("configuration", "Web root must be an existing absolute directory without symlinks.")
        }
        var total = 0
        for case let relative as String in walker {
            let url = root.appendingPathComponent(relative)
            let attributes = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
            guard attributes.isSymbolicLink != true else { throw ScannerError("configuration", "Web assets cannot contain symlinks.") }
            if attributes.isDirectory == true { continue }
            let type: String
            switch url.pathExtension {
            case "html" where relative == "index.html": type = "text/html; charset=utf-8"
            case "js": type = "text/javascript; charset=utf-8"
            case "css": type = "text/css; charset=utf-8"
            case "svg": type = "image/svg+xml"
            case "woff2": type = "font/woff2"
            default: throw ScannerError("configuration", "Web root contains an unexpected file type.")
            }
            guard relative == "index.html" || relative == "favicon.svg" || relative.hasPrefix("assets/"), files.count < 128 else {
                throw ScannerError("configuration", "Web root contains unexpected or excessive assets.")
            }
            let fd = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
            guard fd >= 0 else { throw ScannerError("configuration", "Cannot open web asset.") }
            defer { Darwin.close(fd) }
            var info = stat()
            guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_nlink == 1,
                  info.st_size > 0, info.st_size <= 8 * 1024 * 1024, total + Int(info.st_size) <= 32 * 1024 * 1024 else {
                throw ScannerError("configuration", "Web assets must be bounded regular files.")
            }
            var data = Data(count: Int(info.st_size))
            let count = data.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress, $0.count) }
            guard count == info.st_size else { throw ScannerError("configuration", "Cannot read complete web asset.") }
            total += data.count; files["/" + relative] = Asset(data: data, type: type)
        }
        guard let index = files["/index.html"] else { throw ScannerError("configuration", "Web root requires index.html.") }
        files["/"] = index
    }
    func response(for request: Request, path: GatewayPath) throws -> Response? {
        let key = "/" + path.components.joined(separator: "/")
        let appRoute = ["/inventory", "/library", "/profiles", "/scanner"].contains(key)
            || (path.components.count == 2 && path.components[0] == "batches"
                && UUID(uuidString: path.components[1]) != nil)
        guard let asset = files[key] ?? (appRoute ? files["/index.html"] : nil) else { return nil }
        guard request.method == .get || request.method == .head else { throw GatewayError(405, "methodNotAllowed", "Web assets are read-only.") }
        guard request.headers[.transferEncoding] == nil, request.headers[.contentLength] == nil || request.headers[.contentLength] == "0" else {
            throw GatewayError(400, "invalidParams", "Web asset requests cannot contain a body.")
        }
        try path.only([])
        return Response(status: .ok, headers: [.contentType: asset.type],
                        body: request.method == .head ? .init() : .init(byteBuffer: ByteBuffer(bytes: asset.data)))
    }
    // WebKit reports renderer-managed style attributes under style-src-attr.
    // Permit attributes only; stylesheets and executable code remain same-origin.
    static let contentPolicy = "default-src 'none'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' blob:; connect-src 'self'; worker-src 'self'; font-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
}
