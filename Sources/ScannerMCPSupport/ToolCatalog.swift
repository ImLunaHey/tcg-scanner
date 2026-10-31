import Foundation
import MCP
import ScannerAPI

/// A small flat schema shared by tool advertisement and argument validation.
struct ToolField: Sendable {
    var type: String
    var required = false
    var uuid = false
    var maximumLength = 512
    var range: ClosedRange<Int>?
    var choices: [String]?
    static func text(required: Bool = false, uuid: Bool = false, maximumLength: Int = 512) -> Self {
        Self(type: "string", required: required, uuid: uuid, maximumLength: maximumLength)
    }
    static func integer(_ range: ClosedRange<Int>, required: Bool = false) -> Self { Self(type: "integer", required: required, range: range) }
    static func boolean(required: Bool = false) -> Self { Self(type: "boolean", required: required) }
    static func format(required: Bool = false) -> Self { Self(type: "string", required: required, choices: ["tiff", "png"]) }
    var schema: Value {
        var fields: [String: Value] = ["type": .string(type)]
        if type == "string" {
            fields["minLength"] = .int(1); fields["maxLength"] = .int(maximumLength)
            if uuid { fields["format"] = .string("uuid") }
            if let choices { fields["enum"] = .array(choices.map(Value.string)) }
        }
        if let range { fields["minimum"] = .int(range.lowerBound); fields["maximum"] = .int(range.upperBound) }
        return .object(fields)
    }
    func validate(_ value: JSONValue) -> Bool {
        switch (type, value) {
        case ("string", .string(let text)):
            return !text.isEmpty && text.utf8.count <= maximumLength && !text.utf8.contains(0)
                && (!uuid || UUID(uuidString: text) != nil) && (choices?.contains(text) ?? true)
        case ("boolean", .bool): return true
        case ("integer", .number(let n)):
            guard n.isFinite, n.rounded() == n, let range else { return false }
            return n >= Double(range.lowerBound) && n <= Double(range.upperBound)
        default: return false
        }
    }
}
struct ScannerTool: Sendable {
    var name: String
    var method: String
    var description: String
    var fields: [String: ToolField]
    var changesState = false
    var destructive = false
    var openWorld = false
    var defaults: [String: JSONValue] = [:]
    var tool: Tool {
        Tool(name: name, description: description,
             inputSchema: .object(["type": "object", "properties": .object(fields.mapValues(\.schema)),
                                   "required": .array(fields.filter { $0.value.required }.keys.sorted().map(Value.string)), "additionalProperties": false]),
             annotations: .init(readOnlyHint: !changesState, destructiveHint: destructive, idempotentHint: true, openWorldHint: openWorld),
             outputSchema: .object(["type": "object", "required": ["schemaVersion", "ok"], "properties": [
                "schemaVersion": ["type": "integer", "const": 1], "ok": ["type": "boolean"],
                "data": ["type": "object"], "error": ["type": "object"]], "additionalProperties": false]))
    }
    func parameters(_ arguments: [String: Value]?) throws -> JSONValue {
        let encoded = try JSONEncoder().encode(arguments ?? [:])
        guard encoded.count <= 60 * 1024 else { throw ScannerError("invalidParams", "Tool arguments exceed 60 KiB.") }
        let params = try JSONDecoder().decode([String: JSONValue].self, from: encoded)
        guard params.keys.allSatisfy({ fields[$0] != nil }) else { throw ScannerError("invalidParams", "Unknown tool argument.") }
        for (name, field) in fields {
            if let value = params[name] {
                guard field.validate(value) else { throw ScannerError("invalidParams", "Invalid argument: \(name). See the tool schema.") }
            } else if field.required { throw ScannerError("invalidParams", "Missing required argument: \(name).") }
        }
        return .object(defaults.merging(params) { _, supplied in supplied })
    }
}

enum ToolCatalog {
    static let all: [ScannerTool] = {
        let device: [String: ToolField] = ["deviceID": .text()]
        let id: [String: ToolField] = ["id": .text(required: true, uuid: true)]
        let profileID: [String: ToolField] = ["id": .text(required: true)]
        let page: [String: ToolField] = ["limit": .integer(1...100), "offset": .integer(0...1_000_000)]
        let image: [String: ToolField] = ["jobID": .text(required: true, uuid: true), "sequence": .integer(1...100, required: true)]
        let settings: [String: ToolField] = ["dpi": .integer(150...600, required: true), "duplex": .boolean(required: true), "format": .format(required: true)]
        return [
            ScannerTool(name: "scanner_identify_card", method: "images.identify", description: "Start local English OCR and TCGdex catalogue matching for an existing original. Never scans hardware or uploads images. Reuse id for retries; poll scanner_get_card_identification. Suggestions require review; finish and condition are not automatically assessed.", fields: image.merging(["id": .text(required: true, uuid: true)]) { a, _ in a }, changesState: true, openWorld: true),
            ScannerTool(name: "scanner_get_card_identification", method: "images.identification", description: "Read identification progress, OCR evidence, candidate sets and manual review metadata.", fields: image),
            ScannerTool(name: "scanner_review_card", method: "images.review", description: "Record a user's reviewed candidate ID, finish and condition. Do not infer condition from OCR. Omit cardID to reject suggestions. identificationID prevents reviewing a replaced result.", fields: image.merging(["identificationID": .text(required: true, uuid: true), "cardID": .text(maximumLength: 100), "finish": ToolField(type: "string", choices: CardFinish.allCases.map(\.rawValue)), "condition": ToolField(type: "string", choices: CardCondition.allCases.map(\.rawValue))]) { a, _ in a }, changesState: true),
            ScannerTool(name: "scanner_list_devices", method: "devices", description: "Discover connected scanners without feeding paper.", fields: [:]),
            ScannerTool(name: "scanner_get_device", method: "device.get", description: "Read scanner readiness, ADF loaded state when available, and configured defaults. No scanning.", fields: device),
            ScannerTool(name: "scanner_get_capabilities", method: "capabilities", description: "Inspect driver-exposed capabilities and explicit unknowns. Requires an idle scanner; never scans.", fields: device),
            ScannerTool(name: "scanner_get_settings", method: "settings", description: "Read observed driver settings and saved configuredSettings for future jobs. Requires idle hardware.", fields: device),
            ScannerTool(name: "scanner_set_settings", method: "settings.set", description: "Validate and persist DPI/duplex/format defaults for future jobs using useDeviceSettings=true. Does not write hardware settings or feed paper. Rejects busy scanners and unsupported settings.", fields: settings.merging(device) { a, _ in a }, changesState: true),
            ScannerTool(name: "scanner_list_profiles", method: "profiles.list", description: "List saved scan profiles.", fields: [:]),
            ScannerTool(name: "scanner_get_profile", method: "profiles.get", description: "Get a profile by UUID or unique name.", fields: profileID),
            ScannerTool(name: "scanner_create_profile", method: "profiles.create", description: "Create a profile. Reuse the same UUID and fields when retrying; a conflicting name or ID fails. This does not validate trading-card feeding safety.", fields: settings.merging(["id": .text(required: true, uuid: true), "name": .text(required: true, maximumLength: 128)]) { a, _ in a }, changesState: true),
            ScannerTool(name: "scanner_update_profile", method: "profiles.update", description: "Update supplied profile fields only; queued/active jobs keep their original snapshots. Identify by UUID or unique name.", fields: ["id": .text(required: true), "name": .text(maximumLength: 128), "dpi": .integer(150...600), "duplex": .boolean(), "format": .format()], changesState: true),
            ScannerTool(name: "scanner_start_scan", method: "jobs.create", description: "Physically feed loaded ordinary A4 paper, asynchronously. Confirm media readiness before calling. Defaults to 300 DPI, simplex, PNG. Only this tool starts scanning. Reuse requestID for retries; a new UUID means another physical scan. A profile or useDeviceSettings replaces DPI/duplex/format. Card feeding remains unvalidated.", fields: ["requestID": .text(required: true, uuid: true), "deviceID": .text(), "outputDirectory": .text(maximumLength: 2048), "profileID": .text(uuid: true), "useDeviceSettings": .boolean(), "dpi": .integer(150...600), "duplex": .boolean(), "format": .format()], changesState: true, destructive: true, defaults: ["dpi": .number(300), "duplex": .bool(false), "format": .string("png")]),
            ScannerTool(name: "scanner_get_job", method: "jobs.get", description: "Poll job state and saved images. Check state/errors; a successful query does not imply a successful scan.", fields: id),
            ScannerTool(name: "scanner_list_jobs", method: "jobs.list", description: "Read persistent job history with bounded pagination.", fields: page),
            ScannerTool(name: "scanner_cancel_job", method: "jobs.cancel", description: "Cancel queued or active scan. Originals remain; active cancellation is asynchronous. Poll until terminal.", fields: id, changesState: true, destructive: true),
            ScannerTool(name: "scanner_list_batches", method: "batches.list", description: "List scan batches (one batch per job).", fields: page),
            ScannerTool(name: "scanner_get_batch", method: "batches.get", description: "Retrieve batch metadata and manifest including original image sequence, dimensions, paths and SHA-256.", fields: id),
            ScannerTool(name: "scanner_get_scan_metadata", method: "jobs.get", description: "Retrieve the complete scan record, settings, images, errors and output manifest metadata.", fields: id),
            ScannerTool(name: "scanner_get_batch_manifest", method: "batches.get", description: "Retrieve the JSON batch manifest from the service ledger.", fields: id),
            ScannerTool(name: "scanner_get_image_metadata", method: "images.get", description: "Read metadata for an original identified by job UUID and page sequence.", fields: image),
            ScannerTool(name: "scanner_read_image", method: "images.read", description: "Retrieve original image bytes as a bounded base64 chunk. Follow nextOffset until eof, verify chunkSHA256 and final image SHA-256. Accepts no filesystem path.", fields: image.merging(["offset": .integer(0...512 * 1024 * 1024), "length": .integer(1...262_144)]) { a, _ in a }),
            ScannerTool(name: "scanner_get_service_status", method: "service.status", description: "Read queue/health without opening a hardware session.", fields: [:]),
            ScannerTool(name: "scanner_get_events", method: "events", description: "Read progress events, optionally long-poll up to 20 seconds. Use nextCursor; on resetRequired refresh snapshots and start after=0.", fields: ["after": .integer(0...9_007_199_254_740_991), "limit": .integer(1...100), "waitSeconds": .integer(0...20)]),
            ScannerTool(name: "scanner_get_diagnostics", method: "diagnostics", description: "Inspect service and driver diagnostics. Never feeds paper. Hardware inspection can return busy during an active scan; service status/events remain available.", fields: device)
        ]
    }()
}
