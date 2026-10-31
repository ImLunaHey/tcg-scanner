import SwiftUI
import ScannerAPI

struct ProfileDraft: Identifiable {
    var id: UUID { profile.id }
    var profile: ScanProfile
    var creating: Bool
}
struct ProfilesView: View {
    @Bindable var model: ScannerModel
    @State private var draft: ProfileDraft?
    @State private var deleting: ScanProfile?
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("Scan profiles").font(.largeTitle.bold())
                Spacer()
                Menu("New profile") {
                    Button("Custom") { draft = ProfileDraft(profile: ScanProfile(name: "New profile"), creating: true) }
                    Button("General documents") { draft = ProfileDraft(profile: ScanProfile(name: "General documents", duplex: true), creating: true) }
                    Button("Archival · 600 DPI") { draft = ProfileDraft(profile: ScanProfile(name: "Archival", dpi: 600, duplex: true), creating: true) }
                    Button("Pokémon · setup required") { draft = ProfileDraft(profile: ScanProfile(name: "Pokémon — setup required", dpi: 600, duplex: true), creating: true) }
                }.disabled(model.busy || !model.connected)
            }
            Text("Profiles are shared with the CLI, MCP and HTTP clients. Pokémon presets currently use A4 settings; card dimensions, feeding safety and automatic processing require the later card-validation milestone.")
                .font(.callout).foregroundStyle(.secondary)
            List {
                ForEach(model.profiles, id: \.id) { profile in
                    HStack(spacing: 16) {
                        Image(systemName: "doc.badge.gearshape").font(.title2).foregroundStyle(.tint)
                        VStack(alignment: .leading, spacing: 5) {
                            Text(profile.name).font(.headline)
                            Text("\(profile.dpi) DPI · \(profile.duplex ? "Duplex" : "Simplex") · \(profile.format.rawValue.uppercased()) · A4 colour").foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Use") { model.selectedProfile = profile.id; model.section = "Dashboard" }
                        Button("Edit") { draft = ProfileDraft(profile: profile, creating: false) }
                        Menu {
                            Button("Duplicate") {
                                var copy = profile; copy.id = UUID(); copy.name += " copy"
                                draft = ProfileDraft(profile: copy, creating: true)
                            }
                            Button("Delete…", role: .destructive) { deleting = profile }
                        } label: { Image(systemName: "ellipsis") }
                    }.padding(.vertical, 10)
                }
            }.listStyle(.inset).overlay {
                if model.profiles.isEmpty { ContentUnavailableView("No profiles yet", systemImage: "slider.horizontal.3", description: Text("Create a preset to reuse settings across all your scanner clients.")) }
            }
        }.padding(24)
        .sheet(item: $draft) { draft in ProfileEditor(model: model, draft: draft) }
        .confirmationDialog("Delete \(deleting?.name ?? "profile")?", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
            Button("Delete profile", role: .destructive) {
                guard let id = deleting?.id else { return }; deleting = nil
                model.perform { try await model.client.deleteProfile(id); try await model.refresh() }
            }
        } message: { Text("Saved scans keep their original profile snapshot.") }
    }
}
private struct ProfileEditor: View {
    @Environment(\.dismiss) private var dismiss
    @Bindable var model: ScannerModel
    @State var draft: ProfileDraft
    @State private var saving = false
    @State private var failure: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(draft.creating ? "Create profile" : "Edit profile").font(.title2.bold())
            Form {
                TextField("Name", text: $draft.profile.name)
                if model.dpiValues.isEmpty {
                    TextField("Resolution (150–600 DPI)", value: $draft.profile.dpi, format: .number)
                } else {
                    Picker("Resolution", selection: $draft.profile.dpi) {
                        ForEach(Array(Set(model.dpiValues + [draft.profile.dpi])).sorted(), id: \.self) { dpi in
                            Text("\(dpi) DPI" + (model.dpiValues.contains(dpi) ? "" : " · unsupported on selected scanner")).tag(dpi)
                        }
                    }
                }
                Toggle("Duplex", isOn: $draft.profile.duplex)
                Picker("Output", selection: $draft.profile.format) { Text("PNG").tag(OriginalFormat.png); Text("TIFF").tag(OriginalFormat.tiff) }
                Text("The selected scanner must support this resolution and duplex mode. Supported DPI: \(model.dpiValues.map(String.init).joined(separator: ", ")).").font(.caption).foregroundStyle(.secondary)
            }
            if let failure { Text(failure).foregroundStyle(.red) }
            HStack {
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Spacer()
                if saving { ProgressView().controlSize(.small) }
                Button("Save") {
                    saving = true
                    Task {
                        defer { saving = false }
                        do { _ = try await model.client.save(draft.profile, creating: draft.creating); try await model.refresh(); dismiss() }
                        catch { failure = error.localizedDescription }
                    }
                }.keyboardShortcut(.defaultAction).disabled(saving || draft.profile.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }.padding(28).frame(width: 460).interactiveDismissDisabled(saving)
    }
}
