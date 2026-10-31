import SwiftUI
import ScannerAPI

struct LibraryView: View {
    @Bindable var model: ScannerModel
    var body: some View {
        HStack(spacing: 0) {
            VStack {
                List(selection: Binding(get: { model.selectedJob }, set: { if let id = $0 { model.selectJob(id) } })) {
                    ForEach(model.jobs, id: \.id) { JobRow(job: $0).tag($0.id) }
                }
                if model.nextOffset != nil { Button("Load older scans") { model.loadMore() }.disabled(model.busy).padding(10) }
            }.frame(width: 260)
            Divider()
            if let record = model.detail { BatchDetail(model: model, record: record).id(record.id) }
            else { ContentUnavailableView("Select a scan", systemImage: "photo.stack", description: Text("Browse saved originals and processed images.")) }
        }
    }
}
private struct EditingImage: Identifiable { var id: Int { image.sequence }; var image: OriginalImage }
private struct BatchDetail: View {
    @Bindable var model: ScannerModel
    let record: ScanRecord
    @State private var editing: EditingImage?
    @State private var compare = false
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    VStack(alignment: .leading, spacing: 6) {
                        if let number = record.batchNumber { Text(BatchNaming.batchFolder(number)).font(.title.bold()) }
                        Text(record.createdAt, format: .dateTime.month().day().hour().minute()).font(record.batchNumber == nil ? .title : .callout)
                        Text("\(record.state.rawValue.capitalized) · \(record.images.count) saved images · \(record.request.dpi) DPI").foregroundStyle(.secondary)
                    }
                    Spacer()
                    if !record.state.terminal { ProgressView(); Button("Cancel", role: .destructive) { model.cancel(record.id) }.disabled(model.busy) }
                }
                Text(record.device.name + " · " + (record.profile?.name ?? "Custom settings")).font(.callout)
                ForEach(record.errors, id: \.message) { Text($0.message).foregroundStyle(.red).textSelection(.enabled) }
                ForEach(record.warnings, id: \.self) { Text($0).foregroundStyle(.orange) }
                HStack {
                    Menu("Export") {
                        Button("Batch manifest…") { exportJSON(record, name: "manifest-\(record.id.uuidString.prefix(8)).json", model: model) }
                        Button("Entire batch…") { exportBatch(record, model: model) }.disabled(!record.state.terminal)
                    }.disabled(model.busy)
                    Spacer()
                    Toggle("Compare adjacent sides", isOn: $compare).toggleStyle(.checkbox)
                }
                if compare { Text("Adjacent images are shown in acquisition order. Front/back identity and pairing have not been verified for this batch.").font(.caption).foregroundStyle(.orange) }
                if record.images.isEmpty { ContentUnavailableView("No images saved yet", systemImage: "doc.text.image", description: Text(record.state.terminal ? "This job did not produce any saved images." : "Previews appear after each original has been saved.")) }
                LazyVGrid(columns: compare ? [GridItem(.flexible()), GridItem(.flexible())] : [GridItem(.adaptive(minimum: 220))], spacing: 18) {
                    ForEach(record.images, id: \.sequence) { image in
                        VStack(alignment: .leading, spacing: 8) {
                            ScanThumbnail(model: model, job: record.id, sequence: image.sequence).frame(height: 230)
                            Text("C-\(BatchNaming.number(image.sequence)) · original").font(.headline)
                            Text("\(image.width) × \(image.height) · side \(image.side)").font(.caption).foregroundStyle(.secondary)
                            HStack {
                                Button("Edit…") { editing = EditingImage(image: image) }.disabled(!record.state.terminal || model.busy)
                                Button("Export…") { exportImage(record: record, image: image, model: model) }.disabled(model.busy)
                            }
                        }.padding(10).background(.background, in: RoundedRectangle(cornerRadius: 10))
                    }
                }
                if let edits = record.processedImages, !edits.isEmpty {
                    Text("Processed PNGs").font(.title2.bold())
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 220))], spacing: 18) {
                        ForEach(edits, id: \.id) { edit in
                            VStack(alignment: .leading, spacing: 8) {
                                ScanThumbnail(model: model, job: record.id, sequence: edit.request.sequence, edit: edit.id).frame(height: 230)
                                Text("C-\(BatchNaming.number(edit.request.sequence)) · edited").font(.headline)
                                Text("\(edit.image.width) × \(edit.image.height) · \(edit.createdAt.formatted(date: .omitted, time: .shortened))").font(.caption)
                                Button("Export PNG…") { exportImage(record: record, image: edit.image, editID: edit.id, model: model) }.disabled(model.busy)
                            }.padding(10)
                        }
                    }
                }
                Text(record.outputDirectory).font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
            }.padding(24)
        }.sheet(item: $editing) { ImageEditor(model: model, job: record.id, image: $0.image) }
    }
}
