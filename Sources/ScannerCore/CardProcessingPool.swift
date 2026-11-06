import Foundation

/// Keeps decoding/Vision work bounded instead of launching every 600 DPI image.
/// Results publish through ScanStorage; acquisition sequence remains authoritative.
enum CardProcessingPool {
    static let concurrency = 2
    static func run<Item: Sendable>(_ items: [Item], operation: @escaping @Sendable (Item) async -> Void) async {
        await withTaskGroup(of: Void.self) { group in
            var iterator = items.makeIterator()
            for _ in 0..<concurrency {
                guard !Task.isCancelled, let item = iterator.next() else { break }
                group.addTask { await operation(item) }
            }
            while await group.next() != nil {
                guard !Task.isCancelled else { group.cancelAll(); break }
                if let item = iterator.next() { group.addTask { await operation(item) } }
            }
        }
    }
}
