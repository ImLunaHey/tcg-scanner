import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

actor MockBackend: ScannerBackend {
    var found: [Device]
    var inspections = 0
    var fail = false
    var hold = false
    var entered = false
    init(_ found: [Device]) { self.found = found }
    func devices() -> [Device] { found }
    func setHold(_ value: Bool) { hold = value }
    func setFail(_ value: Bool) { fail = value }
    func inspect(deviceID: String) async throws -> Inspection {
        inspections += 1; entered = true
        while hold { try await Task.sleep(for: .milliseconds(5)) }
        if fail { throw ScannerError("disconnected", "Mock disconnected") }
        var report = Inspection(device: found.first { $0.id == deviceID }!)
        report.sessionOpened = true; report.sessionClosed = true
        return report
    }
}

@Test func jsonPreservesBooleanAndNull() throws {
    let value: JSONValue = .object(["bool": .bool(true), "number": .number(1), "null": .null])
    #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(value)) == value)
}

@Test func missingAndAmbiguousDevicesNeverOpenSessions() async throws {
    for devices in [[], [Device(id: "a", name: "A"), Device(id: "b", name: "B")]] {
        let mock = MockBackend(devices)
        let coordinator = ScannerCoordinator(backend: mock)
        await #expect(throws: ScannerError.self) { try await coordinator.inspect(deviceID: nil) }
        #expect(await mock.inspections == 0)
    }
}

@Test func explicitSelectionAndFailureReleaseOwnership() async throws {
    let mock = MockBackend([Device(id: "a", name: "A"), Device(id: "b", name: "B")])
    let coordinator = ScannerCoordinator(backend: mock)
    await mock.setFail(true)
    await #expect(throws: ScannerError.self) { try await coordinator.inspect(deviceID: "b") }
    await mock.setFail(false)
    let report = try await coordinator.inspect(deviceID: "b")
    #expect(report.device.id == "b")
    #expect(await mock.inspections == 2)
}

@Test func concurrentClientsCannotOpenOverlappingSessions() async throws {
    let mock = MockBackend([Device(id: "a", name: "A")])
    await mock.setHold(true)
    let coordinator = ScannerCoordinator(backend: mock)
    let first = Task { try await coordinator.inspect(deviceID: nil) }
    while !(await mock.entered) { await Task.yield() }
    do {
        _ = try await coordinator.inspect(deviceID: nil)
        Issue.record("Expected busy error")
    } catch let error as ScannerError { #expect(error.kind == "busy") }
    #expect(try await coordinator.devices().count == 1)
    await mock.setHold(false)
    _ = try await first.value
    #expect(await mock.inspections == 1)
}

@Test func cancellationReleasesOwnership() async throws {
    let mock = MockBackend([Device(id: "a", name: "A")])
    await mock.setHold(true)
    let coordinator = ScannerCoordinator(backend: mock)
    let task = Task { try await coordinator.inspect(deviceID: nil) }
    while !(await mock.entered) { await Task.yield() }
    task.cancel()
    _ = await task.result
    await mock.setHold(false)
    #expect(try await coordinator.inspect(deviceID: nil).sessionClosed)
}

@Test func driverAnomaliesRemainVisible() throws {
    var unit = UnitSnapshot()
    unit.settings["physicalSize"] = Setting(current: .array([.number(0), .number(0)]), readOnly: true)
    var feature = Feature()
    feature.internalName = "blank_page_skip"
    feature.fields = ["current": .number(3), "values": .array([.number(0), .number(1), .number(2)]),
                      "labels": .array([.string("Off"), .string("On")])]
    unit.vendorFeatures = [feature]
    #expect(InspectionWarnings.evaluate([unit]).count == 3)
    let encoded = try JSONEncoder().encode(unit)
    let decoded = try JSONDecoder().decode(UnitSnapshot.self, from: encoded)
    #expect(decoded.settings["physicalSize"]?.defaultValue == nil)
    #expect(decoded.vendorFeatures[0].fields == feature.fields)
}
