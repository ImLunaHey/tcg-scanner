import Foundation
import GRDB
import ScannerAPI

extension ScannerDatabase {
    public func storeTransfer(_ id: UUID) throws -> Data? {
        try queue.read { try Data.fetchOne($0, sql: "SELECT record FROM storeTransfers WHERE jobID = ?", arguments: [id.uuidString]) }
    }
    public func saveStoreTransfer(_ status: StoreTransferStatus, record: Data) throws {
        try queue.write { db in
            try db.execute(sql: "INSERT INTO storeTransfers(jobID,state,record) VALUES (?,?,?) ON CONFLICT(jobID) DO UPDATE SET state=excluded.state,record=excluded.record",
                arguments: [status.jobID.uuidString, status.state.rawValue, record])
            try Self.appendEvent("store.transfer.updated", payload: .wrapping(status), db: db)
        }
    }
    public func interruptedStoreTransfers() throws -> [Data] {
        try queue.read { try Data.fetchAll($0, sql: "SELECT record FROM storeTransfers WHERE state = 'sending'") }
    }
}
