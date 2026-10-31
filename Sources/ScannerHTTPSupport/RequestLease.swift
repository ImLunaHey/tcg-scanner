import Foundation

/// Releases admission even if the server drops a response before writing its body.
/// The lock protects the single release shared by explicit completion and deinit.
final class RequestLease: @unchecked Sendable {
    private let access: GatewayAccess
    private let kind: String
    private let lock = NSLock()
    private var released = false
    init(access: GatewayAccess, kind: String) { self.access = access; self.kind = kind }
    private func take() -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard !released else { return false }; released = true; return true
    }
    func finish() async {
        if take() { await access.release(kind) }
    }
    deinit {
        if take() {
            let access = access; let kind = kind
            Task { await access.release(kind) }
        }
    }
}
