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
    @State private var identifying: EditingImage?
    @State private var checkingCondition: EditingImage?
    @State private var compare = false
    @State private var preparing = false
    @State private var preparationIDs: [Int: UUID] = [:]
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    VStack(alignment: .leading, spacing: 6) {
                        if let number = record.batchNumber { Text(BatchNaming.batchFolder(number)).font(.title.bold()) }
                        Text(record.createdAt, format: .dateTime.month().day().hour().minute()).font(record.batchNumber == nil ? .title : .callout)
                        Text("\((record.state == .scanning ? "Receiving images" : record.state.rawValue.capitalized)) · \(record.images.count) saved images · \(record.request.dpi) DPI").foregroundStyle(.secondary)
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
                if compare { Text("Adjacent images are shown in acquisition order. Recognized card backs determine side labels; unknown pairs keep numbered sides.").font(.caption).foregroundStyle(.orange) }
                if record.images.isEmpty { ContentUnavailableView("No images saved yet", systemImage: "doc.text.image", description: Text(record.state.terminal ? "This job did not produce any saved images." : "Previews appear after each original has been saved.")) }
                LazyVGrid(columns: compare ? [GridItem(.flexible()), GridItem(.flexible())] : [GridItem(.adaptive(minimum: 220))], spacing: 18) {
                    ForEach(record.images, id: \.sequence) { image in
                        let prepared = record.processedImages?.last(where: { $0.request.sequence == image.sequence && $0.cardPreparation != nil })
                        let game = record.detectedGame(for: image.sequence)
                        let pair = record.cardGames?.first { $0.number == (image.sequence + 1) / 2 }
                        let side = pair?.frontSequence == image.sequence ? "Front" : pair?.backSequence == image.sequence ? "Back" : "Side \((image.sequence - 1) % 2 + 1)"
                        VStack(alignment: .leading, spacing: 8) {
                            ScanThumbnail(model: model, job: record.id, sequence: image.sequence, edit: prepared?.id).frame(height: 230)
                            Text("C-\(BatchNaming.number(record.request.duplex ? (image.sequence + 1) / 2 : image.sequence)) · \(prepared == nil ? "original" : "cropped card")").font(.headline)
                            Text("\(prepared?.image.width ?? image.width) × \(prepared?.image.height ?? image.height) · \(side)").font(.caption).foregroundStyle(.secondary)
                            Text(pair?.contentKind == .pokemonCodeCard ? "Pokémon online/code card" : game == .pokemon ? "Pokémon" : game == .yugioh ? "Yu-Gi-Oh!" : game == .starwarsUnlimited ? "Star Wars: Unlimited" : "Unknown game").font(.caption)
                            if game != .unknown {
                                if let identification = record.cardIdentifications?.first(where: { $0.request.sequence == image.sequence }) {
                                    CardIdentitySummary(identification: identification)
                                    if let check = identification.conditionCheck { Button(check.summary) { checkingCondition = EditingImage(image: image) }.font(.caption).buttonStyle(.link) }
                                }
                                if game == .pokemon { Button("Check condition…") { checkingCondition = EditingImage(image: image) }.disabled(!record.state.terminal) }
                                if game == .pokemon || pair?.frontSequence == image.sequence || game == .starwarsUnlimited {
                                    Button("Identify / review card…") { identifying = EditingImage(image: image) }.disabled(!record.state.terminal)
                                }
                            } else {
                                Text(pair?.contentKind == .pokemonCodeCard ? "Redemption card; playable-card identity, finish and condition checks do not apply." : "Card identity, finish and condition checks are unavailable for this game.").font(.caption).foregroundStyle(.secondary)
                            }
                            if let prepared {
                                Button("Export prepared card…") { exportImage(record: record, image: prepared.image, editID: prepared.id, model: model) }
                                ForEach(prepared.cardPreparation?.warnings ?? [], id: \.self) { Text($0).font(.caption).foregroundStyle(.orange) }
                            } else {
                                Button(preparing ? "Preparing…" : "Prepare card") {
                                    let id = preparationIDs[image.sequence] ?? UUID(); preparationIDs[image.sequence] = id
                                    preparing = true
                                    Task {
                                        defer { preparing = false }
                                        do { _ = try await model.client.prepareCard(jobID: record.id, sequence: image.sequence, id: id); try await model.refresh() }
                                        catch { model.error = error.localizedDescription }
                                    }
                                }.disabled(preparing || !record.state.terminal)
                            }
                            HStack {
                                Button("Edit…") { editing = EditingImage(image: image) }.disabled(!record.state.terminal || model.busy)
                                Button(prepared == nil ? "Export…" : "Export original…") { exportImage(record: record, image: image, model: model) }.disabled(model.busy)
                            }
                        }.padding(10).background(.background, in: RoundedRectangle(cornerRadius: 10))
                    }
                }
                if let edits = record.processedImages?.filter({ $0.cardPreparation == nil }), !edits.isEmpty {
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
        .sheet(item: $checkingCondition) { ConditionReviewView(model: model, job: record.id, sequence: $0.image.sequence) }
        .sheet(item: $identifying) { CardIdentificationView(model: model, job: record.id, sequence: $0.image.sequence) }
    }
}

private struct CardIdentitySummary: View {
    let identification: CardIdentification
    private var card: CardCandidate? {
        if identification.state == .confirmed {
            return identification.evidence?.candidates.first { $0.id == identification.selectedCardID }
        }
        if identification.state == .reviewRequired, identification.evidence?.candidates.count == 1 {
            return identification.evidence?.candidates.first
        }
        return nil
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            if let card {
                Text(identification.state == .confirmed ? "Confirmed" : "Suggested · review required").foregroundStyle(.secondary)
                Text(card.name).bold()
                Text(([card.category == "Pokemon" ? "Pokémon" : card.category].compactMap { $0 } + (card.types ?? [])).joined(separator: " · "))
                Text(card.setName + " · " + card.localID + (card.officialSetCount.map { "/\($0)" } ?? ""))
                if let rarities = card.rarities, !rarities.isEmpty { Text("Catalogue rarities: " + rarities.joined(separator: " / ")) }
                if card.provider != "ygoprodeck" { Text("Finish: " + identification.finishDescription) }
            } else {
                switch identification.state {
                case .processing: Text("Identifying card…")
                case .reviewRequired: Text("\(identification.evidence?.candidates.count ?? 0) possible matches · review identity")
                case .noMatch:
                    if let energy = identification.evidence?.energySymbol {
                        Text(energy.displayName).bold()
                        Text("Exact printing unknown · type recognised from the corner symbol.")
                    } else { Text("No card match found") }
                case .failed: Text("Identification failed · retry in review")
                case .rejected: Text("Suggestions rejected")
                case .confirmed: Text("Identity unavailable")
                }
            }
            if let edition = identification.editionPresentation {
                Text(edition.label).foregroundStyle(.secondary).help(edition.detail)
            }
        }.font(.caption)
    }
}
