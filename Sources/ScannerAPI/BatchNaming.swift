import Foundation

/// Stable labels for boxes and sleeves; UUIDs remain API/idempotency keys.
public enum BatchNaming {
    public static func number(_ value: Int) -> String { String(format: "%03ld", value) }
    public static func batchFolder(_ number: Int) -> String { "B-" + self.number(number) }
    public static func originalName(sequence: Int, format: OriginalFormat) -> String {
        "C-\(number(sequence)).\(format.rawValue)"
    }
    public static func exportDirectory(_ record: ScanRecord) -> String {
        record.batchNumber.map(batchFolder) ?? "batch-" + record.id.uuidString.lowercased()
    }
}
