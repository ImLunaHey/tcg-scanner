import Darwin
import Foundation
import ScannerAPI

/// Independent of socket and database paths: one scanner owner per macOS user.
public final class HardwareOwnerLock: Sendable {
    public static var defaultPath: String {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Scanner/run/hardware.lock").path
    }
    private let descriptor: Int32
    public init(path: String = HardwareOwnerLock.defaultPath) throws {
        let directory = URL(fileURLWithPath: path).deletingLastPathComponent()
        guard directory.standardizedFileURL.path == directory.resolvingSymlinksInPath().path else {
            throw ScannerError("accessDenied", "Hardware lock directory cannot contain symlinks.")
        }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        var info = stat()
        guard lstat(directory.path, &info) == 0, info.st_mode & S_IFMT == S_IFDIR, info.st_uid == getuid(), info.st_mode & 0o077 == 0 else {
            throw ScannerError("accessDenied", "Hardware lock directory must be private and owned by you.")
        }
        let fd = open(path, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard fd >= 0 else { throw ScannerError("storage", "Cannot open hardware owner lock.") }
        guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_uid == getuid(), info.st_mode & 0o077 == 0 else {
            Darwin.close(fd); throw ScannerError("accessDenied", "Hardware lock must be a private regular file owned by you.")
        }
        guard flock(fd, LOCK_EX | LOCK_NB) == 0 else {
            Darwin.close(fd); throw ScannerError("busy", "Another ScannerService owns hardware access for this user, possibly at a different socket.")
        }
        descriptor = fd
    }
    deinit { Darwin.close(descriptor) }
}
