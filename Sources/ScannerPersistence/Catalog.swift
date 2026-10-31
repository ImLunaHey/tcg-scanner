import Foundation
import GRDB
import ScannerAPI

extension ScannerDatabase {
    public func profiles() throws -> [ScanProfile] {
        try queue.read { db in try Data.fetchAll(db, sql: "SELECT record FROM profiles ORDER BY id").map { try Wire.decoder().decode(ScanProfile.self, from: $0) } }
    }
    public func profile(_ id: UUID) throws -> ScanProfile? {
        try queue.read { db in try Data.fetchOne(db, sql: "SELECT record FROM profiles WHERE id = ?", arguments: [id.uuidString]).map { try Wire.decoder().decode(ScanProfile.self, from: $0) } }
    }
    public func saveProfile(_ profile: ScanProfile) throws {
        try profile.validate()
        try queue.write { db in
            try db.execute(sql: "INSERT OR REPLACE INTO profiles VALUES (?, ?)", arguments: [profile.id.uuidString, try Wire.encoder().encode(profile)])
            try Self.appendEvent("profile.updated", payload: .wrapping(profile), db: db)
        }
    }
    public func deleteProfile(_ id: UUID) throws {
        try queue.write { db in
            try db.execute(sql: "DELETE FROM profiles WHERE id = ?", arguments: [id.uuidString])
            guard db.changesCount == 1 else { throw ScannerError("profileNotFound", "Unknown profile ID.") }
            try Self.appendEvent("profile.deleted", payload: .string(id.uuidString), db: db)
        }
    }
    public func observeDevices(_ devices: [Device]) throws {
        try queue.write { db in
            let old = Set(try String.fetchAll(db, sql: "SELECT id FROM devices WHERE connected = 1"))
            let current = Set(devices.map(\.id))
            try db.execute(sql: "UPDATE devices SET connected = 0")
            for device in devices {
                try db.execute(sql: "INSERT OR REPLACE INTO devices VALUES (?, ?, 1)", arguments: [device.id, try Wire.encoder().encode(device)])
                if !old.contains(device.id) { try Self.appendEvent("device.connected", payload: .wrapping(device), db: db) }
            }
            for id in old.subtracting(current) { try Self.appendEvent("device.disconnected", payload: .string(id), db: db) }
        }
    }
    public func saveInspection(_ inspection: Inspection) throws {
        try queue.write { db in
            try db.execute(sql: "INSERT OR REPLACE INTO capabilities VALUES (?, ?)", arguments: [inspection.device.id, try Wire.encoder().encode(inspection)])
        }
    }
    public func cachedInspection(_ deviceID: String) throws -> Inspection? {
        try queue.read { db in
            try Data.fetchOne(db, sql: "SELECT record FROM capabilities WHERE deviceID = ?", arguments: [deviceID]).map { try Wire.decoder().decode(Inspection.self, from: $0) }
        }
    }
}
