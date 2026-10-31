import SwiftUI
import ScannerMacUI

@main struct ScannerMacApp: App {
    var body: some Scene {
        Window("Scanner", id: "scanner") { ScannerRootView() }
            .defaultSize(width: 1180, height: 800)
    }
}
