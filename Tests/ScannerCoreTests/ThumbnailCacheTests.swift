import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

@Test func thumbnailCacheBoundsMemoryAndInvalidatesChangedFiles() {
    var cache = ThumbnailCache(limit: 8)
    let image = ImageThumbnail(data: Data([1, 2, 3]), sha256: "fixture", width: 1, height: 1)
    cache.insert(image, key: "a", fingerprint: "1")
    cache.insert(image, key: "b", fingerprint: "1")
    #expect(cache.value(key: "a", fingerprint: "1")?.base64 == image.base64)
    cache.insert(image, key: "c", fingerprint: "1")
    #expect(cache.value(key: "b", fingerprint: "1") == nil)
    #expect(cache.value(key: "a", fingerprint: "2") == nil)
    #expect(cache.value(key: "a", fingerprint: "1") == nil)
    cache.insert(ImageThumbnail(data: Data(repeating: 0, count: 12), sha256: "large", width: 1, height: 1), key: "large", fingerprint: "1")
    #expect(cache.value(key: "large", fingerprint: "1") == nil)
}
