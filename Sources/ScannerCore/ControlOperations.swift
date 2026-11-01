import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func profile(_ identifier: String) throws -> ScanProfile {
        if let id = UUID(uuidString: identifier), let profile = try database.profile(id) { return profile }
        let matches = try database.profiles().filter { $0.name.caseInsensitiveCompare(identifier) == .orderedSame }
        guard matches.count <= 1 else { throw ScannerError("ambiguousProfile", "Multiple profiles have this name; use a UUID.") }
        guard let profile = matches.first else { throw ScannerError("profileNotFound", "Unknown profile: \(identifier)") }
        return profile
    }
    public func createProfile(_ profile: ScanProfile) throws -> ScanProfile {
        if let existing = try database.profile(profile.id) {
            guard existing == profile else { throw ScannerError("conflict", "Profile ID already exists with different settings.") }
            return existing
        }
        try uniqueProfileName(profile)
        try saveProfile(profile)
        return profile
    }
    public func updateProfile(_ identifier: String, patch: ProfilePatch) throws -> ScanProfile {
        guard !patch.isEmpty else { throw ScannerError("invalidSettings", "Provide at least one profile field to update.") }
        var value = try profile(identifier)
        value.name = patch.name ?? value.name; value.dpi = patch.dpi ?? value.dpi
        value.duplex = patch.duplex ?? value.duplex; value.format = patch.format ?? value.format
        try uniqueProfileName(value)
        try saveProfile(value)
        return value
    }
    public func removeProfile(_ identifier: String) throws { try deleteProfile(profile(identifier).id) }
    func uniqueProfileName(_ profile: ScanProfile) throws {
        guard try !database.profiles().contains(where: { $0.id != profile.id && $0.name.caseInsensitiveCompare(profile.name) == .orderedSame }) else {
            throw ScannerError("conflict", "A profile with this name already exists.")
        }
    }
    public func configuredSettings(_ id: String) throws -> ScanSettings? { try database.deviceSettings(id) }
    public func setSettings(deviceID: String?, settings: ScanSettings) async throws -> DeviceStatus {
        try settings.validate()
        guard !inspecting, activeScanID == nil, queue.isEmpty, !stopping, !paused, !requiresRestart else {
            throw ScannerError("busy", "Settings cannot change while a job is active or queued, or the service is unavailable.")
        }
        inspecting = true
        defer { inspecting = false; scheduleNext() }
        try await releaseAutomaticButton()
        let device = try await selectDevice(deviceID)
        guard !stopping, !paused else { throw ScannerError("busy", "Service paused during scanner handoff.") }
        let inspection = try await backend.inspect(deviceID: device.id)
        try database.saveInspection(inspection)
        guard inspection.errors.isEmpty, inspection.sessionClosed else { throw ScannerError("inspection", "Could not inspect and close scanner session; defaults were not changed.") }
        guard let adf = inspection.units.first(where: { $0.type == 3 }),
              case .array(let resolutions) = adf.settings["resolutionDPI"]?.allowed,
              resolutions.contains(.number(Double(settings.dpi))) else { throw ScannerError("unsupportedSetting", "DPI is not exposed as supported by the ADF.") }
        if settings.duplex && adf.settings["supportsDuplexScanning"]?.current != .bool(true) {
            throw ScannerError("unsupportedSetting", "Driver does not report duplex support.")
        }
        // A job may have queued while inspection suspended; never change its defaults.
        guard queue.isEmpty, !stopping, !paused else { throw ScannerError("busy", "Service state changed during validation; defaults were not changed.") }
        try database.saveDeviceSettings(settings, deviceID: device.id)
        return DeviceStatus(device: device, service: status(), inspection: inspection, configuredSettings: settings, ready: nil,
            warnings: ["Defaults saved for future jobs using useDeviceSettings=true. No hardware settings were written or scan initiated.",
                       "Current engine supports A4 RGB 8-bit scans; format/other settings are verified during preparation."])
    }
    public func deviceStatus(_ deviceID: String?) async throws -> DeviceStatus {
        let device = try await selectDevice(deviceID)
        var inspection: Inspection?
        var warnings: [String] = []
        if inspecting || activeScanID != nil || stopping || paused || requiresRestart { warnings.append("Scanner is busy or unavailable; no readiness inspection performed.") }
        else {
            do { inspection = try await inspect(deviceID: device.id) }
            catch let error as ScannerError where error.kind == "busy" { warnings.append(error.message) }
        }
        let service = status()
        let ready: Bool?
        if service.activeJobID != nil || service.paused || service.stopping || service.queuedJobs > 0 { ready = false }
        else if let inspection {
            if !inspection.errors.isEmpty || !inspection.sessionClosed { ready = false }
            else if let adf = inspection.units.first(where: { $0.type == 3 }), case .bool(let loaded) = adf.settings["documentLoaded"]?.current { ready = loaded }
            else { ready = nil }
        } else { ready = nil }
        warnings.append("Readiness is a momentary ADF observation, not a guarantee a later scan will succeed.")
        return DeviceStatus(device: device, service: service, inspection: inspection,
            configuredSettings: try database.deviceSettings(device.id), ready: ready, warnings: warnings)
    }
}
