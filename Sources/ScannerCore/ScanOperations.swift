import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func startScan(_ submission: ScanRequest) async throws -> ScanRecord {
        try await submitScan(submission, fromButton: false)
    }
    func submitScan(_ submission: ScanRequest, fromButton: Bool) async throws -> ScanRecord {
        try submission.validate()
        if let existing = try existingSubmission(submission) { return existing }
        try acceptingJobs()
        var request = submission
        // Resolve new card jobs centrally so every client, including the physical
        // button, preserves edges. Keep explicit white comparisons and opt-outs.
        if request.overscan == nil, request.prepareCards == true, request.background != .white {
            request.overscan = true
        }
        let profile: ScanProfile?
        if let id = submission.profileID {
            guard let saved = try database.profile(id) else { throw ScannerError("profileNotFound", "Unknown scan profile.") }
            profile = saved; request.dpi = saved.dpi; request.duplex = saved.duplex; request.format = saved.format
        } else { profile = nil }
        let device = try await selectDevice(request.deviceID)
        // Selection suspends. Recheck idempotency/capacity before creating files or reserving hardware.
        if let existing = try existingSubmission(submission) { return existing }
        try acceptingJobs()
        if fromButton {
            try Task.checkCancellation()
            guard !inspecting, activeScanID == nil, queue.isEmpty else { throw ScannerError("busy", "Scan-button press ignored while service is busy.") }
        }
        if submission.useDeviceSettings == true {
            guard let settings = try database.deviceSettings(device.id) else { throw ScannerError("settingsNotFound", "No configured defaults for this scanner.") }
            request.dpi = settings.dpi; request.duplex = settings.duplex; request.format = settings.format
        }
        let database = self.database
        try validateStorage()
        let number = try database.reserveBatchNumber(for: request.requestID)
        let store = try ScanStorage(root: storageRoot, device: device, request: request, batchNumber: number, queued: true, profile: profile, validateStorage: validateStorage, persist: database.save)
        let record = store.initialRecord
        // Reserve submission before another actor turn can observe a missing row.
        // No other operation can access this store until insertion completes.
        try database.insert(record, submission: submission)
        scans[record.id] = store; queue.append(record.id)
        scheduleNext()
        return record
    }
    private func existingSubmission(_ request: ScanRequest) throws -> ScanRecord? {
        guard let original = try database.submission(request.requestID) else { return nil }
        guard original == request else { throw ScannerError("conflict", "Request ID was already used with different settings.") }
        return try database.job(request.requestID)
    }
    private func acceptingJobs() throws {
        guard buttonMonitorTask == nil else { throw ScannerError("busy", "Button diagnostics are active; stop observation before scanning.") }
        guard !stopping && !paused && !requiresRestart && persistenceError == nil else { throw ScannerError("busy", "Service is stopping, paused or has a persistence error.") }
        guard queue.count < 100 else { throw ScannerError("resourceLimit", "Queue is full (100 waiting jobs).") }
    }
    func scheduleNext() {
        guard !stopping, !paused, !requiresRestart, persistenceError == nil, !inspecting, activeScanID == nil, !queue.isEmpty else { return }
        let id = queue.removeFirst()
        guard let store = scans[id] else { return }
        activeScanID = id
        scanTask = Task { await execute(id, store: store) }
    }
    private func execute(_ id: UUID, store: ScanStorage) async {
        let record = await store.snapshot()
        let outcome: CaptureOutcome
        do {
            try Task.checkCancellation()
            try await releaseAutomaticButton()
            try await store.markPreparing()
            outcome = try await backend.capture(deviceID: record.device.id, request: record.request, store: store)
        } catch {
            outcome = CaptureOutcome(errors: error is CancellationError ? [] : [ScannerError("capture", error: error)],
                                     cancelled: error is CancellationError, sessionClosed: error is CancellationError)
        }
        do {
            try await store.markProcessing()
            if record.request.prepareCards == true, outcome.errors.isEmpty, !outcome.cancelled { await prepareCapturedCards(store) }
            try await store.finish(errors: outcome.errors, cancelled: outcome.cancelled || Task.isCancelled, sessionClosed: outcome.sessionClosed)
        } catch {
            await store.recordPersistenceFailure(error)
            persistenceError = ScannerError("storage", error: error)
            // Do not start another physical job after a persistence failure.
            try? database.save(await store.snapshot())
        }
        if !outcome.sessionClosed { requiresRestart = true }
        scans[id] = nil; activeScanID = nil; scanTask = nil
        scheduleNext()
    }
    public func scanStatus(_ id: UUID) async throws -> ScanRecord {
        if let store = scans[id] { return await store.snapshot() }
        guard let record = try database.job(id) else { throw ScannerError("scanNotFound", "Unknown scan ID.") }
        return record
    }
    public func cancelScan(_ id: UUID) async throws -> ScanRecord {
        if activeScanID == id { scanTask?.cancel() }
        else if let position = queue.firstIndex(of: id), let store = scans[id] {
            queue.remove(at: position)
            do { try await store.finish(errors: [], cancelled: true, sessionClosed: true) }
            catch { persistenceError = ScannerError("storage", error: error); throw error }
            scans[id] = nil
        }
        return try await scanStatus(id)
    }
    public func shutdownScans() async {
        stopping = true
        try? await releaseAutomaticButton()
        await stopButtonObservation()
        let identificationWorkers = Array(identificationTasks.values)
        for worker in identificationWorkers { worker.cancel() }
        await cancelOutstanding()
        for worker in identificationWorkers { await worker.value }
    }
    public func prepareForSleep() async {
        paused = true
        try? await releaseAutomaticButton()
        await stopButtonObservation()
        try? database.event("service.sleep", payload: .null)
        await cancelOutstanding()
    }
    public func resumeAfterWake() throws {
        paused = false
        try database.event("service.wake", payload: .null)
        scheduleNext()
    }
    private func cancelOutstanding() async {
        // Capture task before awaits: completion clears scanTask.
        let task = scanTask
        task?.cancel()
        for id in queue { _ = try? await cancelScan(id) }
        await task?.value
    }
}
