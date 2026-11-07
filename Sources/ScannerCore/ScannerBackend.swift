import Foundation
import ScannerAPI
import ScannerPersistence
import ScannerRecognition

public protocol ScannerBackend: Sendable {
    /// True only when no session remains owned, or removal of the exact
    /// quarantined device has been confirmed. Must never start scanning.
    func recoverAfterDisconnect() async -> Bool
    func devices() async throws -> [Device]
    func inspect(deviceID: String) async throws -> Inspection
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome
    func observeButton(deviceID: String, seconds: Int) async throws
    /// Returns only after a loaded-feeder Scan press and confirmed session closure.
    /// Cancellation must also close the session before returning or throwing.
    func waitForScanButton(deviceID: String) async throws
}

public extension ScannerBackend {
    func recoverAfterDisconnect() async -> Bool { false }
    func waitForScanButton(deviceID: String) async throws {
        throw ScannerError("unsupported", "Automatic Scan-button monitoring is not supported by this backend.")
    }
    func observeButton(deviceID: String, seconds: Int) async throws {
        throw ScannerError("unsupported", "Button observation is not supported by this backend.")
    }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        throw ScannerError("unsupported", "This backend does not implement scanning.")
    }
}

/// Guards against actor reentrancy: a second request cannot inspect hardware
/// while the first request is suspended awaiting a driver callback.
public actor ScannerCoordinator {
    let backend: any ScannerBackend
    let storeClient: (any StoreStaging)?
    var storeTransferTask: Task<Void, Never>?
    var preparingStoreTransfer = false
    var inspecting = false
    var buttonMonitorTask: Task<Void, Never>?
    var automaticButtonTask: Task<Void, Never>?
    let scanButtonDeviceID: String?
    var nextButtonAttempt = ContinuousClock.now
    var stopping = false
    var scans: [UUID: ScanStorage] = [:]
    var queue: [UUID] = []
    var paused = false
    var requiresRestart = false
    var persistenceError: ScannerError?
    var eventWaiters = 0
    var thumbnailWorkers = 0
    var thumbnailCache = ThumbnailCache()
    var editingJobs: Set<UUID> = []
    var preparingCardJobs: Set<UUID> = []
    var automaticallyIdentifyingCards = false
    var identificationTasks: [UUID: Task<Void, Never>] = [:]
    let cardRecognizer: any CardRecognizing
    let starWarsRecognizer: any CardRecognizing
    let yugiohRecognizer: any CardRecognizing
    let finishAnalyzer: CardFinishAnalyzer
    let database: ScannerDatabase
    let pricingCache: PricingCache
    var scanTask: Task<Void, Never>?
    var activeScanID: UUID?
    let storageRoot: URL
    let validateStorage: @Sendable () throws -> Void
    public init(backend: any ScannerBackend, storageRoot: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Scans"), database: ScannerDatabase? = nil, validateStorage: @escaping @Sendable () throws -> Void = {}, cardRecognizer: any CardRecognizing = CardRecognizer(), cardPricing: any CardPricingProviding = CardPricingProvider(), scanButtonDeviceID: String? = nil, storeClient: (any StoreStaging)? = nil, cardPrintings: any CardPrintingProviding = CardPrintingProvider(), cardReferences: any CardReferenceProviding = TCGdexReferenceCache(), yugiohRecognizer: any CardRecognizing = YugiohRecognizer(), starWarsRecognizer: any CardRecognizing = StarWarsRecognizer()) throws {
        self.storeClient = storeClient
        self.scanButtonDeviceID = scanButtonDeviceID
        self.backend = backend; self.storageRoot = storageRoot; self.validateStorage = validateStorage
        self.database = try database ?? ScannerDatabase()
        self.cardRecognizer = cardRecognizer
        self.yugiohRecognizer = yugiohRecognizer
        self.starWarsRecognizer = starWarsRecognizer
        self.finishAnalyzer = CardFinishAnalyzer(catalogue: cardPrintings, references: cardReferences)
        self.pricingCache = PricingCache(database: self.database, provider: cardPricing)
        for data in try self.database.interruptedStoreTransfers() {
            var ledger = try Wire.decoder().decode(StoreTransferLedger.self, from: data)
            ledger.status.state = .failed
            ledger.status.error = ScannerError("storeInterrupted", "Service restarted during transfer. Retry sending to resume safely.")
            ledger.status.updatedAt = Date()
            try self.database.saveStoreTransfer(ledger.status, record: Wire.encoder().encode(ledger))
        }
    }
    public func devices() async throws -> [Device] {
        let devices = try await backend.devices()
        try database.observeDevices(devices)
        await recoverScannerConnection()
        return devices
    }
    public func inspect(deviceID: String?) async throws -> Inspection {
        guard !inspecting && activeScanID == nil && !stopping && !paused && !requiresRestart else { throw ScannerError("busy", "Scanner is busy or shutting down; retry shortly.") }
        inspecting = true
        defer { inspecting = false; scheduleNext() }
        try await releaseAutomaticButton()
        let selected = try await selectDevice(deviceID)
        guard !stopping, !paused else { throw ScannerError("busy", "Service paused during scanner handoff.") }
        let report = try await backend.inspect(deviceID: selected.id)
        if report.errors.contains(where: { $0.kind == "cleanup" || $0.kind == "closeSession" }) { requiresRestart = true }
        try database.saveInspection(report)
        return report
    }
    func selectDevice(_ deviceID: String?) async throws -> Device {
        let devices = try await backend.devices()
        let selected: Device
        if let deviceID {
            guard let d = devices.first(where: { $0.id == deviceID }) else {
                throw ScannerError("deviceNotFound", "No scanner matches device ID \(deviceID).")
            }
            selected = d
        } else {
            guard !devices.isEmpty else { throw ScannerError("deviceNotFound", "No ICA scanner found. Check USB, power and the installed macOS driver.") }
            guard devices.count == 1 else { throw ScannerError("ambiguousDevice", "Multiple scanners found. Specify --device using an ID from devices.") }
            selected = devices[0]
        }
        return selected
    }

}
