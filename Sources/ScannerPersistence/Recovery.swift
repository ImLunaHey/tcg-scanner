import Foundation
import ScannerAPI

extension ScannerDatabase {
    /// Run only after acquiring the process-wide hardware lock, before accepting clients.
    /// Never resumes physical feeding. A manifest may be ahead of SQLite after a crash.
    @discardableResult public func recoverInterruptedJobs(storageRoot: URL, accessStorage: Bool = true) throws -> Int {
        let root = accessStorage ? storageRoot.standardizedFileURL.resolvingSymlinksInPath().path : storageRoot.standardizedFileURL.path
        let scanning = try unfinished()
        let records = try scanning + interruptedIdentifications().filter { candidate in !scanning.contains(where: { $0.id == candidate.id }) }
        for var record in records {
            let directory = URL(fileURLWithPath: record.outputDirectory).standardizedFileURL
            let safe = accessStorage && directory.path.hasPrefix(root + "/") && directory.resolvingSymlinksInPath().path == directory.path
            let manifest = directory.appendingPathComponent("manifest.json")
            if !record.state.terminal, safe, manifest.resolvingSymlinksInPath().path == manifest.path,
               let attrs = try? FileManager.default.attributesOfItem(atPath: manifest.path),
               attrs[.type] as? FileAttributeType == .typeRegular,
               let size = attrs[.size] as? NSNumber, size.intValue <= ScanLimits.responseBytes,
               let data = try? Data(contentsOf: manifest),
               let disk = try? Wire.decoder().decode(ScanRecord.self, from: data),
               disk.id == record.id, disk.request == record.request, disk.outputDirectory == record.outputDirectory,
               disk.continuations?.last?.requestID == record.continuations?.last?.requestID,
               disk.images.count >= record.images.count {
                record = disk
            }
            if !record.state.terminal {
                record.state = .failed; record.completedAt = Date(); record.sessionClosed = false
                record.errors.append(ScannerError("interrupted", "Service stopped before this job finished. Physical feeding was not resumed; inspect retained files before submitting a new job."))
                record.warnings.append("Unindexed or partial files may remain in the batch directory or its staging directories. Recovery never deletes or overwrites image files.")
            }
            if let index = record.continuations?.indices.last, record.continuations?[index].state.terminal == false {
                record.continuations?[index].state = record.state
                record.continuations?[index].completedAt = record.completedAt
                let first = record.continuations?[index].firstSequence ?? 1
                record.continuations?[index].imagesSaved = record.images.count - first + 1
            }
            for index in record.cardIdentifications?.indices ?? 0..<0 {
                if record.cardIdentifications?[index].state == .processing {
                    record.cardIdentifications?[index].state = .failed
                    record.cardIdentifications?[index].completedAt = Date()
                    record.cardIdentifications?[index].error = ScannerError("interrupted", "Identification was interrupted. Retry explicitly; the scan and original are unchanged.")
                }
            }
            if safe && manifest.resolvingSymlinksInPath().path == manifest.path {
                do { try Wire.encoder().encode(record).write(to: manifest, options: .atomic) }
                catch { record.warnings.append("Could not update recovered manifest: \(error.localizedDescription)") }
            } else {
                record.warnings.append(accessStorage
                    ? "Storage root changed or path is unsafe; on-disk manifest left untouched."
                    : "Scan storage was unavailable during recovery. On-disk files were not read or changed; inspect retained files after reconnecting the share.")
            }
            try save(record)
        }
        return records.count
    }
}
