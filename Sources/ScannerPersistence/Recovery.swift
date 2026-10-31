import Foundation
import ScannerAPI

extension ScannerDatabase {
    /// Run only after acquiring the process-wide hardware lock, before accepting clients.
    /// Never resumes physical feeding. A manifest may be ahead of SQLite after a crash.
    @discardableResult public func recoverInterruptedJobs(storageRoot: URL) throws -> Int {
        let root = storageRoot.standardizedFileURL.resolvingSymlinksInPath().path
        let records = try unfinished()
        for var record in records {
            let directory = URL(fileURLWithPath: record.outputDirectory).standardizedFileURL
            let safe = directory.path.hasPrefix(root + "/") && directory.resolvingSymlinksInPath().path == directory.path
            let manifest = directory.appendingPathComponent("manifest.json")
            if safe, manifest.resolvingSymlinksInPath().path == manifest.path,
               let attrs = try? FileManager.default.attributesOfItem(atPath: manifest.path),
               attrs[.type] as? FileAttributeType == .typeRegular,
               let size = attrs[.size] as? NSNumber, size.intValue <= 4 * 1024 * 1024,
               let data = try? Data(contentsOf: manifest),
               let disk = try? Wire.decoder().decode(ScanRecord.self, from: data),
               disk.id == record.id, disk.request == record.request, disk.outputDirectory == record.outputDirectory {
                record = disk
            }
            if !record.state.terminal {
                record.state = .failed; record.completedAt = Date(); record.sessionClosed = false
                record.errors.append(ScannerError("interrupted", "Service stopped before this job finished. Physical feeding was not resumed; inspect retained files before submitting a new job."))
                record.warnings.append("Unindexed or partial files may remain in incoming/ or originals/. Recovery never deletes or overwrites image files.")
            }
            if safe {
                do { try Wire.encoder().encode(record).write(to: manifest, options: .atomic) }
                catch { record.warnings.append("Could not update recovered manifest: \(error.localizedDescription)") }
            } else { record.warnings.append("Storage root changed or path is unsafe; on-disk manifest left untouched.") }
            try save(record)
        }
        return records.count
    }
}
