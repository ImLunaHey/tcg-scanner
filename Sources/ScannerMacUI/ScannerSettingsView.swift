import SwiftUI
import ScannerAPI

struct DevicePicker: View {
    @Bindable var model: ScannerModel
    var body: some View {
        Picker("Scanner", selection: Binding(get: { model.selectedDevice }, set: { id in
            model.selectedDevice = id; model.inspection = nil; model.deviceStatus = nil; model.paperReady = false
            model.perform { try await model.inspectSelected() }
        })) {
            if model.devices.isEmpty { Text("No scanner connected").tag("") }
            ForEach(model.devices, id: \.id) { Text($0.name).tag($0.id) }
        }
        .disabled(model.busy || model.pending != nil || model.service?.activeJobID != nil)

    }
}
struct ScanControls: View {
    @Bindable var model: ScannerModel
    var body: some View {
        HStack(spacing: 24) {
            Picker("Resolution", selection: $model.dpi) {
                if model.dpiValues.isEmpty { Text("Inspect scanner").tag(model.dpi) }
                ForEach(model.dpiValues, id: \.self) { Text("\($0) DPI").tag($0) }
            }.disabled(model.dpiValues.isEmpty)
            Picker("Format", selection: $model.format) { Text("PNG").tag(OriginalFormat.png); Text("TIFF").tag(OriginalFormat.tiff) }
            Toggle("Duplex", isOn: $model.duplex).disabled(!model.supportsDuplex)
        }
    }
}
struct ScannerSettingsView: View {
    @Bindable var model: ScannerModel
    var body: some View {
        Form {
            Section("Scanner") {
                DevicePicker(model: model)
                Button("Inspect capabilities and readiness") { model.perform { try await model.inspectSelected() } }
                    .disabled(model.busy || model.selectedDevice.isEmpty || model.service?.activeJobID != nil)
                if let status = model.deviceStatus {
                    LabeledContent("Readiness at last inspection", value: status.ready.map { $0 ? "Ready" : "Not ready" } ?? "Unknown")
                    ForEach(status.warnings, id: \.self) { Text($0).foregroundStyle(.orange) }
                }
            }
            Section("Defaults for future jobs") {
                ScanControls(model: model)
                Text("Only DPI, duplex and output format are configurable in the current engine. A4, RGB colour and 8-bit channels are fixed. Defaults never change an active job.").font(.caption).foregroundStyle(.secondary)
                Button("Save device defaults") {
                    model.perform {
                        model.deviceStatus = try await model.client.setSettings(ScanSettings(dpi: model.dpi, duplex: model.duplex, format: model.format), deviceID: model.selectedDevice)
                        model.notice = "Device defaults saved."
                    }
                }.disabled(model.busy || model.inspection == nil || model.dpiValues.isEmpty || model.service?.activeJobID != nil)
            }
            Section("Notifications") {
                Button(model.notificationsEnabled ? "Notifications enabled" : "Enable completion and error notifications") { model.enableNotifications() }
                Text("Notifications apply to jobs observed while this app is running.").font(.caption).foregroundStyle(.secondary)
            }
            if let report = model.inspection {
                Section("Driver capability report") {
                    Text("Observed \(report.observedAt.formatted()). Property values are not necessarily defaults. Driver exposure does not mean hardware verification.").font(.caption)
                    ForEach(Array(report.units.enumerated()), id: \.offset) { _, unit in
                        DisclosureGroup("\(unit.className) · unit \(unit.type)") {
                            ForEach(unit.settings.keys.sorted(), id: \.self) { key in
                                if let setting = unit.settings[key] {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(key).fontWeight(.medium)
                                        Text("Current: \(describe(setting.current))")
                                        if let allowed = setting.allowed { Text("Allowed: \(describe(allowed))") }
                                        if let value = setting.defaultValue { Text("Default: \(describe(value))") }
                                        Text("\(setting.readOnly ? "Read only" : "Driver exposes a setter") · \(setting.evidence.rawValue)").font(.caption).foregroundStyle(.secondary)
                                    }.textSelection(.enabled).padding(.vertical, 4)
                                }
                            }
                            ForEach(Array((unit.vendorFeatures + unit.templates).enumerated()), id: \.offset) { _, feature in
                                DisclosureGroup(feature.label ?? feature.internalName ?? "Driver feature") {
                                    Text(feature.tooltip ?? "No driver description").font(.caption)
                                    Text(describe(.object(feature.fields))).textSelection(.enabled)
                                    Text("\(feature.evidence.rawValue) · inspection only").font(.caption)
                                }
                            }
                        }
                    }
                    ForEach(report.errors, id: \.message) { Text($0.message).foregroundStyle(.red) }
                    ForEach(report.warnings + report.limitations, id: \.self) { Text($0).font(.caption).foregroundStyle(.secondary) }
                    Text("Unknown: " + report.unknownCapabilities.joined(separator: ", ")).font(.caption)
                    Button("Export capability report…") { exportJSON(report, name: "scanner-capabilities.json", model: model) }
                }
            }
        }.formStyle(.grouped)
    }
}
func describe(_ value: JSONValue) -> String {
    guard let data = try? Wire.encoder().encode(value) else { return "Unknown" }
    return String(decoding: data, as: UTF8.self)
}
