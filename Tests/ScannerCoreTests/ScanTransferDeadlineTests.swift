import Testing
@testable import ScannerCore

@Test func progressingTransfersSurviveTwoMinutesButStallsAndRunawaysStop() {
    let start = ContinuousClock.now
    var deadline = ScanTransferDeadline(now: start)
    #expect(!deadline.expired(now: start + .seconds(119)))
    #expect(deadline.expired(now: start + .seconds(120)))
    deadline.receivedImage(now: start + .seconds(110))
    #expect(!deadline.expired(now: start + .seconds(180)))
    #expect(deadline.expired(now: start + .seconds(230)))
    deadline.receivedImage(now: start + .seconds(1799))
    #expect(deadline.expired(now: start + .seconds(1800)))
}
