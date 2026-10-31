import Foundation
import ScannerAPI

enum HumanOutput {
    static func printResult(command: String, result: JSONValue) throws {
        switch command {
        case "devices":
            let list = try result.decode(DeviceList.self)
            if list.devices.isEmpty { print("No ICA scanners found. Check power, USB and the installed macOS driver.") }
            for d in list.devices { device(d) }
        case "capabilities": inspection(try result.decode(Inspection.self))
        case "settings":
            let report = try result.decode(SettingsReport.self)
            device(report.device)
            units(report.units)
            messages(report.warnings, report.errors)
        case "diagnostics":
            let d = try result.decode(Diagnostics.self)
            print("OS: \(d.operatingSystem)\nBackend: \(d.backend)\nDiscovered scanners: \(d.devices.count)")
            if let report = d.inspection { inspection(report) }
            else { for d in d.devices { device(d) } }
            messages(d.warnings, [])
        case "jobs.list":
            let page = try result.decode(JobPage.self)
            for job in page.jobs { print("\(job.id.uuidString)  \(job.state.rawValue)  \(job.progress.imagesSaved) image(s)") }
            if page.jobs.isEmpty { print("No jobs.") }
            if let offset = page.nextOffset { print("More jobs: --offset \(offset)") }
        case "service.status":
            let status = try result.decode(ServiceStatus.self)
            print("Active: \(status.activeJobID?.uuidString ?? "none"); queued: \(status.queuedJobs); paused: \(status.paused); stopping: \(status.stopping)")
            if let error = status.persistenceError { print("Error: \(error.message)") }
        default:
            let encoder = Wire.encoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            print(String(decoding: try encoder.encode(result), as: UTF8.self))
        }
    }
    private static func device(_ d: Device) {
        print("\(d.name) [\(d.id)]\n  Transport: \(d.transport ?? "unknown")\n  Driver: \(d.driverPath) (module \(d.driverModuleVersion ?? "unknown"))")
    }
    private static func inspection(_ r: Inspection) {
        device(r.device)
        print("Session opened: \(r.sessionOpened); closed: \(r.sessionClosed)")
        print("Capabilities below are driver-exposed, not scan-tested.")
        units(r.units)
        print("Unknown: \(r.unknownCapabilities.joined(separator: ", "))")
        messages(r.warnings, r.errors)
    }
    private static func units(_ units: [UnitSnapshot]) {
        for unit in units {
            print("Functional unit \(unit.type): \(unit.className)")
            for key in unit.settings.keys.sorted() {
                guard let setting = unit.settings[key] else { continue }
                let allowed = setting.allowed.map { "; allowed: \(text($0))" } ?? ""
                print("  \(key): \(text(setting.current))\(allowed)\(setting.readOnly ? " [read-only]" : "")")
            }
            for feature in unit.vendorFeatures {
                print("  \(feature.label ?? feature.internalName ?? "Unnamed feature"): \(text(feature.fields["current"] ?? .null))")
                if let values = feature.fields["values"] { print("    values: \(text(values))") }
                if let labels = feature.fields["labels"] { print("    labels: \(text(labels))") }
            }
        }
    }
    private static func messages(_ warnings: [String], _ errors: [ScannerError]) {
        for w in warnings { print("Warning: \(w)") }
        for e in errors { print("Error [\(e.kind)]: \(e.message)") }
    }
    private static func text(_ value: JSONValue) -> String {
        guard let data = try? Wire.encoder().encode(value) else { return "unknown" }
        return String(decoding: data, as: UTF8.self)
    }
}
