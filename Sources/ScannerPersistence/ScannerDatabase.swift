import Darwin
import Foundation
import GRDB
import ScannerAPI

/// GRDB serializes transactions on its own queue. No hardware or image decoding runs here.
public final class ScannerDatabase: Sendable {
    let queue: DatabaseQueue
    public init(path: String? = nil) throws {
        if let path {
            let directory = URL(fileURLWithPath: path).deletingLastPathComponent()
            guard directory.resolvingSymlinksInPath().path == directory.standardizedFileURL.path else {
                throw ScannerError("accessDenied", "Database directory cannot contain symlinks.")
            }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            var info = stat()
            guard lstat(directory.path, &info) == 0, info.st_mode & S_IFMT == S_IFDIR,
                  info.st_uid == getuid(), info.st_mode & 0o077 == 0 else {
                throw ScannerError("accessDenied", "Database directory must be private and owned by you.")
            }
            // Check SQLite sidecars as well; never follow a pre-existing symlink.
            for file in [path, path + "-wal", path + "-shm", path + "-journal"] {
                if lstat(file, &info) == 0 {
                    guard info.st_mode & S_IFMT == S_IFREG, info.st_uid == getuid(), info.st_mode & 0o077 == 0 else {
                        throw ScannerError("accessDenied", "Database files must be private regular files owned by you.")
                    }
                } else if errno != ENOENT { throw ScannerError("storage", "Cannot inspect database path.") }
            }
        }
        var configuration = Configuration()
        configuration.prepareDatabase { db in try db.execute(sql: "PRAGMA synchronous = FULL") }
        queue = try path.map { try DatabaseQueue(path: $0, configuration: configuration) } ?? DatabaseQueue(configuration: configuration)
        if let path { guard chmod(path, 0o600) == 0 else { throw ScannerError("storage", "Cannot protect database permissions.") } }
        var migrator = DatabaseMigrator()
        migrator.registerMigration("v1-job-ledger") { db in
            try db.execute(sql: """
                CREATE TABLE jobs (id TEXT PRIMARY KEY, request BLOB NOT NULL, state TEXT NOT NULL,
                    created REAL NOT NULL, record BLOB NOT NULL);
                CREATE INDEX jobs_history ON jobs(created DESC, id);
                CREATE TABLE batches (id TEXT PRIMARY KEY REFERENCES jobs(id), manifest BLOB NOT NULL);
                CREATE TABLE images (jobID TEXT NOT NULL REFERENCES jobs(id), sequence INTEGER NOT NULL,
                    record BLOB NOT NULL, PRIMARY KEY(jobID, sequence));
                CREATE TABLE profiles (id TEXT PRIMARY KEY, record BLOB NOT NULL);
                CREATE TABLE devices (id TEXT PRIMARY KEY, record BLOB NOT NULL, connected INTEGER NOT NULL);
                CREATE TABLE capabilities (deviceID TEXT PRIMARY KEY, record BLOB NOT NULL);
                CREATE TABLE events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, timestamp REAL NOT NULL,
                    kind TEXT NOT NULL, payload BLOB NOT NULL);
                """)
        }
        migrator.registerMigration("v2-device-settings") { db in
            try db.execute(sql: "CREATE TABLE deviceSettings (deviceID TEXT PRIMARY KEY, record BLOB NOT NULL)")
        }
        migrator.registerMigration("v3-batch-numbers") { db in
            // Reservations survive failures and are never recycled. Historical IDs are
            // allocated oldest-first; filesystem relocation is a separate maintenance step.
            try db.execute(sql: "CREATE TABLE batchNumbers (number INTEGER PRIMARY KEY AUTOINCREMENT, jobID TEXT NOT NULL UNIQUE)")
            for id in try String.fetchAll(db, sql: "SELECT id FROM jobs ORDER BY created, id") {
                try db.execute(sql: "INSERT INTO batchNumbers(jobID) VALUES (?)", arguments: [id])
            }
        }
        migrator.registerMigration("v4-identification-recovery") { db in
            try db.execute(sql: "CREATE TABLE pendingIdentifications (jobID TEXT PRIMARY KEY REFERENCES jobs(id))")
        }
        try migrator.migrate(queue)
    }
    public func reserveBatchNumber(for id: UUID) throws -> Int {
        try queue.write { db in
            if let number = try Int.fetchOne(db, sql: "SELECT number FROM batchNumbers WHERE jobID = ?", arguments: [id.uuidString]) { return number }
            try db.execute(sql: "INSERT INTO batchNumbers(jobID) VALUES (?)", arguments: [id.uuidString])
            return Int(db.lastInsertedRowID)
        }
    }
    public func insert(_ record: ScanRecord, submission: ScanRequest) throws {
        try queue.write { db in
            try db.execute(sql: "INSERT INTO jobs VALUES (?, ?, ?, ?, ?)", arguments: [record.id.uuidString,
                try Wire.encoder().encode(submission), record.state.rawValue, record.createdAt.timeIntervalSince1970, try Wire.encoder().encode(record)])
            try Self.writeMetadata(record, db: db)
        }
    }
    public func save(_ record: ScanRecord) throws {
        try queue.write { db in
            try db.execute(sql: "UPDATE jobs SET state = ?, record = ? WHERE id = ?",
                arguments: [record.state.rawValue, try Wire.encoder().encode(record), record.id.uuidString])
            guard db.changesCount == 1 else { throw ScannerError("storage", "Job is missing from the database.") }
            try Self.writeMetadata(record, db: db)
        }
    }
    private static func writeMetadata(_ record: ScanRecord, db: Database) throws {
        if record.cardIdentifications?.contains(where: { $0.state == .processing }) == true {
            try db.execute(sql: "INSERT OR IGNORE INTO pendingIdentifications VALUES (?)", arguments: [record.id.uuidString])
        } else {
            try db.execute(sql: "DELETE FROM pendingIdentifications WHERE jobID = ?", arguments: [record.id.uuidString])
        }
        try db.execute(sql: "INSERT OR REPLACE INTO batches VALUES (?, ?)", arguments: [record.id.uuidString, try Wire.encoder().encode(record)])
        for image in record.images {
            try db.execute(sql: "INSERT OR REPLACE INTO images VALUES (?, ?, ?)", arguments: [record.id.uuidString, image.sequence, try Wire.encoder().encode(image)])
        }
        try appendEvent("job.updated", payload: .wrapping(JobSummary(record)), db: db)
    }
    public func job(_ id: UUID) throws -> ScanRecord? {
        try queue.read { db in
            try Data.fetchOne(db, sql: "SELECT record FROM jobs WHERE id = ?", arguments: [id.uuidString]).map { try Wire.decoder().decode(ScanRecord.self, from: $0) }
        }
    }
    public func submission(_ id: UUID) throws -> ScanRequest? {
        try queue.read { db in
            try Data.fetchOne(db, sql: "SELECT request FROM jobs WHERE id = ?", arguments: [id.uuidString]).map { try Wire.decoder().decode(ScanRequest.self, from: $0) }
        }
    }
    public func jobs(limit: Int = 50, offset: Int = 0) throws -> JobPage {
        guard (1...100).contains(limit), (0...1_000_000).contains(offset) else { throw ScannerError("invalidParams", "Invalid history pagination.") }
        return try queue.read { db in
            let records = try Data.fetchAll(db, sql: "SELECT record FROM jobs ORDER BY created DESC, id LIMIT ? OFFSET ?", arguments: [limit + 1, offset])
            return try JobPage(jobs: records.prefix(limit).map { JobSummary(try Wire.decoder().decode(ScanRecord.self, from: $0)) }, nextOffset: records.count > limit ? offset + limit : nil)
        }
    }
    public func unfinished() throws -> [ScanRecord] {
        try queue.read { db in
            try Data.fetchAll(db, sql: "SELECT record FROM jobs WHERE state NOT IN ('completed','failed','cancelled') ORDER BY created, id")
                .map { try Wire.decoder().decode(ScanRecord.self, from: $0) }
        }
    }
    func interruptedIdentifications() throws -> [ScanRecord] {
        try queue.read { db in
            try Data.fetchAll(db, sql: "SELECT jobs.record FROM jobs JOIN pendingIdentifications ON jobs.id = pendingIdentifications.jobID")
                .map { try Wire.decoder().decode(ScanRecord.self, from: $0) }
        }
    }
    public func event(_ kind: String, payload: JSONValue) throws {
        try queue.write { db in try Self.appendEvent(kind, payload: payload, db: db) }
    }
    static func appendEvent(_ kind: String, payload: JSONValue, db: Database) throws {
        try db.execute(sql: "INSERT INTO events(timestamp, kind, payload) VALUES (?, ?, ?)", arguments: [Date().timeIntervalSince1970, kind, try Wire.encoder().encode(payload)])
        // A bounded replay window; clients detect missed events and refresh snapshots.
        try db.execute(sql: "DELETE FROM events WHERE cursor <= ?", arguments: [db.lastInsertedRowID - 10_000])
    }
    public func events(after: Int64, limit: Int = 100) throws -> EventPage {
        guard after >= 0, (1...100).contains(limit) else { throw ScannerError("invalidParams", "Invalid event cursor or limit.") }
        return try queue.read { db in
            let rows = try Row.fetchAll(db, sql: "SELECT * FROM events WHERE cursor > ? ORDER BY cursor LIMIT ?", arguments: [after, limit])
            let events = try rows.map { row in ServiceEvent(cursor: row["cursor"], timestamp: Date(timeIntervalSince1970: row["timestamp"]), kind: row["kind"], payload: try Wire.decoder().decode(JSONValue.self, from: row["payload"])) }
            return EventPage(events: events, after: after, oldestCursor: try Int64.fetchOne(db, sql: "SELECT MIN(cursor) FROM events") ?? 0, latestCursor: try Int64.fetchOne(db, sql: "SELECT MAX(cursor) FROM events") ?? 0)
        }
    }
}
