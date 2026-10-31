import AppKit
import Foundation
import Observation
import ScannerAPI
import ScannerClient
import UserNotifications

@MainActor @Observable public final class ScannerModel {
    let thumbnails = ThumbnailLoader()
    let client: ScannerClient
    var devices: [Device] = []
    var profiles: [ScanProfile] = []
    var jobs: [JobSummary] = []
    var service: ServiceStatus?
    var inspectionError: String?
    var inspection: Inspection?
    var deviceStatus: DeviceStatus?
    var selectedDevice = ""
    var selectedProfile: UUID?
    var selectedJob: UUID?
    var detail: ScanRecord?
    var section: String? = "Dashboard"
    var error: String?
    var notice: String?
    var connectionError: String?
    var connected = false
    var busy = false
    var nextOffset: Int?
    var dpi = 300
    var duplex = false
    var format: OriginalFormat = .png
    var paperReady = false
    var pending: ScanRequest?
    var notificationsEnabled: Bool
    private let defaults: UserDefaults
    private var cursor: Int64 = 0
    private var knownStates: [UUID: ScanState] = [:]
    private var refreshing = false
    private var initialized = false

    public init(client: ScannerClient = ScannerClient(), defaults: UserDefaults = .standard) {
        self.client = client; self.defaults = defaults
        notificationsEnabled = defaults.bool(forKey: "scanner.notifications")
        if let data = defaults.data(forKey: "scanner.pendingScan") { pending = try? Wire.decoder().decode(ScanRequest.self, from: data) }
    }
    var canScan: Bool { connected && !busy && !selectedDevice.isEmpty && service?.activeJobID == nil && service?.paused == false && service?.stopping == false && inspection != nil && !dpiValues.isEmpty && paperReady }
    var dpiValues: [Int] {
        guard let allowed = inspection?.units.first(where: { $0.type == 3 })?.settings["resolutionDPI"]?.allowed,
              case .array(let values) = allowed else { return [] }
        return values.compactMap { value in
            guard case .number(let n) = value, n >= 150, n <= 600, n.rounded() == n else { return nil }
            return Int(n)
        }.sorted()
    }
    var supportsDuplex: Bool { inspection?.units.first(where: { $0.type == 3 })?.settings["supportsDuplexScanning"]?.current == .bool(true) }
    func run() async {
        while !Task.isCancelled {
            do {
                try await refresh()
                let page = try await client.events(after: cursor)
                cursor = page.resetRequired ? 0 : page.nextCursor
            } catch is CancellationError { return }
            catch {
                connected = false; connectionError = error.localizedDescription
                do { try await Task.sleep(for: .seconds(3)) } catch { return }
            }
        }
    }
    func refresh() async throws {
        guard !refreshing else { return }; refreshing = true; defer { refreshing = false }
        async let status = client.status()
        async let list = client.devices()
        async let page = client.jobs()
        async let presets = client.profiles()
        let (s, d, p, f) = try await (status, list, page, presets)
        service = s; devices = d; profiles = f; connected = true; connectionError = nil
        // Refresh loaded history in place; older pages remain available until a manual reload.
        let latest = Set(p.jobs.map(\.id))
        jobs = p.jobs + jobs.filter { !latest.contains($0.id) }
        if jobs.count <= 50 { nextOffset = p.nextOffset }
        if !d.contains(where: { $0.id == selectedDevice }) {
            selectedDevice = d.first?.id ?? ""; inspection = nil; deviceStatus = nil
        }
        if let selectedProfile, !f.contains(where: { $0.id == selectedProfile }) { self.selectedProfile = nil }
        if initialized && notificationsEnabled {
            for job in p.jobs where job.state.terminal && knownStates[job.id] != nil && knownStates[job.id] != job.state { notify(job) }
        }
        for job in p.jobs { knownStates[job.id] = job.state }
        initialized = true
        if let selectedJob { detail = try await client.job(selectedJob) }
        if inspection == nil && !selectedDevice.isEmpty && s.activeJobID == nil {
            do { try await inspectSelected(); inspectionError = nil }
            catch { inspectionError = error.localizedDescription }
        }
    }
    func inspectSelected() async throws {
        let id = selectedDevice
        guard !id.isEmpty else { return }
        let report = try await client.inspect(id)
        let status = try await client.device(id)
        guard id == selectedDevice else { return }
        inspection = report; deviceStatus = status
        if let settings = status.configuredSettings { dpi = settings.dpi; duplex = settings.duplex; format = settings.format }
        if !dpiValues.contains(dpi), let first = dpiValues.first { dpi = first }
        if !supportsDuplex { duplex = false }
    }
    func perform(_ action: @escaping @MainActor () async throws -> Void) {
        guard !busy else { return }
        busy = true
        Task {
            defer { busy = false }
            do { try await action() }
            catch { self.error = error.localizedDescription }
        }
    }
    func start() {
        guard canScan else { return }
        perform {
            let request = self.pending ?? ScanRequest(deviceID: self.selectedDevice, dpi: self.dpi, duplex: self.duplex, format: self.format, profileID: self.selectedProfile)
            self.pending = request
            self.defaults.set(try Wire.encoder().encode(request), forKey: "scanner.pendingScan")
            let record: ScanRecord
            do { record = try await self.client.start(request) }
            catch let error as ScannerError {
                // These rejections happen before the service reserves a job. Transport/storage
                // failures remain unresolved and keep the ID for an idempotent retry.
                if ["invalidSettings", "invalidParams", "profileNotFound", "settingsNotFound", "deviceNotFound", "noDevice", "busy", "resourceLimit"].contains(error.kind) {
                    self.pending = nil; self.defaults.removeObject(forKey: "scanner.pendingScan")
                }
                throw error
            }
            self.pending = nil; self.paperReady = false
            self.defaults.removeObject(forKey: "scanner.pendingScan")
            self.selectedJob = record.id; self.detail = record; self.section = "Library"
            try await self.refresh()
        }
    }
    func selectJob(_ id: UUID) {
        selectedJob = id; detail = nil; section = "Library"
        Task {
            do { let value = try await client.job(id); if selectedJob == id { detail = value } }
            catch { self.error = error.localizedDescription }
        }
    }
    func cancel(_ id: UUID) { perform { _ = try await self.client.cancel(id); try await self.refresh() } }
    func loadMore() {
        guard let offset = nextOffset else { return }
        perform {
            let page = try await self.client.jobs(offset: offset)
            let existing = Set(self.jobs.map(\.id))
            self.jobs += page.jobs.filter { !existing.contains($0.id) }; self.nextOffset = page.nextOffset
        }
    }
    func enableNotifications() {
        Task {
            do {
                notificationsEnabled = try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound])
                self.defaults.set(notificationsEnabled, forKey: "scanner.notifications")
                if !notificationsEnabled { notice = "Notifications are disabled. You can change this in macOS System Settings." }
            } catch { self.error = error.localizedDescription }
        }
    }
    private func notify(_ job: JobSummary) {
        let content = UNMutableNotificationContent()
        content.title = job.state == .completed ? "Scan complete" : "Scan \(job.state.rawValue)"
        content.body = "\(job.progress.imagesSaved) images saved. \(job.errors.first?.message ?? "")"
        let request = UNNotificationRequest(identifier: job.id.uuidString, content: content, trigger: nil)
        Task { try? await UNUserNotificationCenter.current().add(request) }
    }
}
