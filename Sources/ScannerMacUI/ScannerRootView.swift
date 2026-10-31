import SwiftUI
import ScannerAPI

public struct ScannerRootView: View {
    @State private var model: ScannerModel
    public init() { _model = State(initialValue: ScannerModel()) }
    public var body: some View {
        NavigationSplitView {
            List(selection: $model.section) {
                Label("Dashboard", systemImage: "scanner").tag("Dashboard")
                Label("Library", systemImage: "photo.stack").tag("Library")
                Label("Profiles", systemImage: "slider.horizontal.3").tag("Profiles")
                Label("Scanner", systemImage: "gearshape").tag("Scanner")
            }
            .navigationTitle("Scanner")
            .safeAreaInset(edge: .bottom) {
                Label(model.connected ? "Service connected" : "Service unavailable", systemImage: model.connected ? "checkmark.circle.fill" : "exclamationmark.circle")
                    .font(.caption).foregroundStyle(model.connected ? .green : .orange).padding()
            }
            .navigationSplitViewColumnWidth(min: 180, ideal: 210)
        } detail: {
            Group {
                switch model.section {
                case "Library": LibraryView(model: model)
                case "Profiles": ProfilesView(model: model)
                case "Scanner": ScannerSettingsView(model: model)
                default: DashboardView(model: model)
                }
            }
            .navigationTitle(model.section ?? "Dashboard")
            .toolbar {
                ToolbarItem {
                    if model.busy { ProgressView().controlSize(.small) }
                    else { Button("Refresh", systemImage: "arrow.clockwise") { model.perform { try await model.refresh() } }.help("Refresh service state") }
                }
            }
        }
        .frame(minWidth: 900, minHeight: 640)
        .task { await model.run() }
        .alert("Scanner", isPresented: Binding(get: { model.error != nil || model.notice != nil }, set: { if !$0 { model.error = nil; model.notice = nil } })) {
            Button("OK") { model.error = nil; model.notice = nil }
        } message: { Text(model.error ?? model.notice ?? "") }
    }
}

struct DashboardView: View {
    @Bindable var model: ScannerModel
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Ready for your next batch").font(.largeTitle.bold())
                    Text("Originals are preserved. PNG is the default output format.").foregroundStyle(.secondary)
                }
                if !model.connected {
                    ContentUnavailableView("ScannerService is unavailable", systemImage: "network.slash", description: Text(model.connectionError ?? "The background service must be installed and running for this macOS user. The app reconnects automatically."))
                }
                if let service = model.service {
                    HStack(spacing: 16) {
                        Metric(title: "Scanners", value: "\(model.devices.count)", icon: "scanner")
                        Metric(title: "Queue", value: "\(service.queuedJobs)", icon: "list.bullet")
                        Metric(title: "Service", value: service.paused ? "Paused" : (service.activeJobID == nil ? "Idle" : "Scanning"), icon: "waveform.path")
                    }
                    if let failure = service.persistenceError { Text(failure.message).foregroundStyle(.red) }
                    if let active = service.activeJobID {
                        HStack {
                            Button("View active scan") { model.selectJob(active) }
                            Spacer()
                            Button("Cancel scan", role: .destructive) { model.cancel(active) }.disabled(model.busy)
                        }
                    }
                }
                GroupBox("New scan") {
                    VStack(alignment: .leading, spacing: 14) {
                        DevicePicker(model: model)
                        Picker("Profile", selection: $model.selectedProfile) {
                            Text("Custom settings").tag(nil as UUID?)
                            ForEach(model.profiles, id: \.id) { Text($0.name).tag(Optional($0.id)) }
                        }.disabled(model.pending != nil)
                        if model.selectedProfile == nil {
                            ScanControls(model: model).disabled(model.pending != nil)
                        } else if let profile = model.profiles.first(where: { $0.id == model.selectedProfile }) {
                            Text("\(profile.dpi) DPI · \(profile.duplex ? "Duplex" : "Simplex") · \(profile.format.rawValue.uppercased())").font(.callout)
                        }
                        Text("Current engine: A4, RGB colour, 8 bits per channel. Trading-card feeding has not been validated.").font(.caption).foregroundStyle(.secondary)
                        if model.pending != nil {
                            Label("A previous request has no confirmed reply. Retry uses its original ID and settings to prevent a duplicate scan.", systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
                        }
                        Toggle("I have loaded ordinary paper and the feeder is ready", isOn: $model.paperReady)
                        Button(model.pending == nil ? "Start scan" : "Retry previous request", systemImage: "scanner.fill") { model.start() }
                            .buttonStyle(.borderedProminent).controlSize(.large).disabled(!model.canScan)
                        if model.inspection == nil && model.connected { Text(model.inspectionError ?? "Inspect the connected scanner before starting.").font(.caption) }
                    }.padding(12)
                }
                GroupBox("Recent scans") {
                    VStack(spacing: 0) {
                        if model.jobs.isEmpty { Text("Your scans will appear here.").foregroundStyle(.secondary).padding() }
                        ForEach(Array(model.jobs.prefix(5)), id: \.id) { job in
                            Button { model.selectJob(job.id) } label: { JobRow(job: job) }.buttonStyle(.plain)
                            Divider()
                        }
                    }.padding(8)
                }
                Text(model.service.map { "Storage: \($0.storageRoot)" } ?? "").font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
            }.padding(28).frame(maxWidth: 1000)
        }
    }
}
private struct Metric: View {
    var title: String; var value: String; var icon: String
    var body: some View {
        GroupBox { VStack(alignment: .leading, spacing: 8) {
            Label(title, systemImage: icon).foregroundStyle(.secondary)
            Text(value).font(.title2.bold())
        }.frame(maxWidth: .infinity, alignment: .leading).padding(12) }
    }
}
struct JobRow: View {
    let job: JobSummary
    var body: some View {
        HStack {
            Image(systemName: job.state == .completed ? "checkmark.circle" : "doc.text.image").font(.title2).foregroundStyle(job.state == .failed ? .red : .accentColor)
            VStack(alignment: .leading) {
                Text(job.createdAt, format: .dateTime.month().day().hour().minute()).fontWeight(.medium)
                Text("\(job.progress.imagesSaved) images · \(job.id.uuidString.prefix(8))").font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            Text(job.state.rawValue.capitalized).font(.caption).padding(6).background(.quaternary, in: Capsule())
        }.padding(.vertical, 10).contentShape(Rectangle())
    }
}
