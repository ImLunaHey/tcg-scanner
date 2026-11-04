import SwiftUI
import ScannerAPI

struct CardIdentificationView: View {
    @Environment(\.dismiss) private var dismiss
    let model: ScannerModel
    let job: UUID
    let sequence: Int
    @State private var result: CardIdentification?
    @State private var loading = true
    @State private var saving = false
    @State private var error: String?
    @State private var requestID = UUID()
    @State private var selected = ""
    @State private var finish = CardFinish.unverified
    @State private var condition = CardCondition.unassessed
    @State private var language: CardLanguage = .english
    @State private var game: CardGame = .unknown
    private var processing: Bool { result?.state == .processing }
    private var candidates: [CardCandidate] { result?.evidence?.candidates ?? [] }
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("Identify C-\(BatchNaming.number(sequence))").font(.title2.bold())
                Spacer(); Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            Text("\(language.displayName) card fronts. OCR runs on this Mac; \(game == .yugioh ? "YGOPRODeck" : "TCGdex") supplies catalogue data. Your image is not uploaded.").font(.callout).foregroundStyle(.secondary)
            if let error { Text(error).foregroundStyle(.red).textSelection(.enabled) }
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if loading || processing { ProgressView(processing ? "Reading text and checking the catalogue…" : "Loading…") }
                    if let result {
                        Text("Identification: " + result.state.rawValue).font(.headline)
                        if let error = result.error { Text(error.message).foregroundStyle(.red) }
                    }
                    if !loading && (result == nil || [.failed, .noMatch, .rejected].contains(result!.state)) {
                        Button(result == nil ? "Identify this image" : "Try identification again") { identify() }.buttonStyle(.borderedProminent).disabled(saving)
                    }
                    if let edition = result?.editionPresentation(for: selected.isEmpty ? nil : selected) {
                        Text(edition.label).font(.callout).help(edition.detail)
                    }
                    if game == .pokemon, result?.state == .noMatch, result?.evidence != nil {
                        Button("Refresh local stamp check") { estimateFinish() }.disabled(saving || processing)
                    }
                    if !candidates.isEmpty, result?.state != .rejected {
                        Picker("Card and set", selection: $selected) {
                            Text("Choose a candidate…").tag("")
                            ForEach(candidates, id: \.id) { card in Text("\(card.name) · \(card.setName) · \(card.localID)").tag(card.id) }
                        }
                        if let card = candidates.first(where: { $0.id == selected }) {
                            Text(card.evidence.joined(separator: " · ")).font(.caption)
                            if let rarities = card.rarities { Text("Catalogue rarities: " + rarities.joined(separator: " / ")).font(.caption) }
                            else { Text("Catalogue finishes: " + card.availableFinishes.map(\.displayName).joined(separator: ", ")).font(.caption).foregroundStyle(.secondary) }
                        }
                        if let estimate = result?.finishEstimate {
                            Text("Finish: " + (result?.finishDescription ?? "Needs review")).font(.headline)
                            DisclosureGroup("Why this finish estimate") {
                                ForEach(estimate.evidence + estimate.limitations, id: \.self) { Text($0).font(.caption).foregroundStyle(.secondary) }
                            }
                            if estimate.state == .estimated, let suggested = estimate.suggestedFinish, selected == estimate.candidateID {
                                Button("Use finish estimate in review") { finish = suggested }.disabled(saving || processing)
                            }
                        }
                        if game == .pokemon {
                            Button(result?.finishEstimate == nil ? "Estimate finish locally" : "Refresh finish estimate") { estimateFinish() }
                                .disabled(saving || processing || candidates.count != 1)
                        }
                        Picker("Finish · manual", selection: $finish) {
                            ForEach(CardFinish.allCases, id: \.self) { Text($0.displayName).tag($0) }
                        }
                        Picker("Condition · manual", selection: $condition) {
                            ForEach(CardCondition.allCases, id: \.self) { Text($0.displayName).tag($0) }
                        }
                        Text("Confirm the printed set and number. Inspect the physical card for finish, dents and scratches. Condition is your assessment, not an automatic grade.").font(.callout).foregroundStyle(.secondary)
                        HStack {
                            Button("Save reviewed identity") { review(reject: false) }.buttonStyle(.borderedProminent).disabled(selected.isEmpty || saving || processing)
                            Button("Reject suggestions") { review(reject: true) }.disabled(saving || processing)
                        }
                    }
                    if let evidence = result?.evidence {
                        ForEach(evidence.warnings, id: \.self) { Text($0).font(.caption).foregroundStyle(.secondary) }
                        DisclosureGroup("OCR evidence") { Text(evidence.lines.map(\.text).joined(separator: "\n")).font(.caption.monospaced()).textSelection(.enabled) }
                    }
                }
            }
        }.padding(24).frame(width: 660, height: 580)
        .task {
            while !Task.isCancelled {
                do {
                    let record = try await model.client.job(job)
                    language = record.request.resolvedCardLanguage
                    game = record.detectedGame(for: sequence)
                    accept(record.cardIdentifications?.first { $0.request.sequence == sequence })
                    loading = false
                    try await Task.sleep(for: .seconds(2))
                } catch is CancellationError { return }
                catch { self.error = error.localizedDescription; loading = false; return }
            }
        }
    }
    private func accept(_ value: CardIdentification?) {
        if result?.request.id != value?.request.id || result?.reviewedAt != value?.reviewedAt {
            selected = value?.selectedCardID ?? ""; finish = value?.finish ?? .unverified; condition = value?.condition ?? .unassessed
        }
        result = value
    }
    private func identify() {
        saving = true; error = nil
        Task {
            defer { saving = false }
            do {
                accept(try await model.client.identify(IdentificationRequest(id: requestID, jobID: job, sequence: sequence, language: language.rawValue)))
                requestID = UUID()
            } catch { self.error = error.localizedDescription }
        }
    }
    private func estimateFinish() {
        guard let result else { return }
        saving = true; error = nil
        Task {
            defer { saving = false }
            do {
                accept(try await model.client.estimateFinish(jobID: job, sequence: sequence, identificationID: result.request.id))
                try await model.refresh()
            } catch { self.error = error.localizedDescription }
        }
    }
    private func review(reject: Bool) {
        guard let result else { return }
        saving = true; error = nil
        Task {
            defer { saving = false }
            do {
                accept(try await model.client.review(jobID: job, sequence: sequence, identificationID: result.request.id,
                    cardID: reject ? nil : selected, finish: finish, condition: condition))
                try await model.refresh()
            } catch { self.error = error.localizedDescription }
        }
    }
}

extension CardFinish {
    var displayName: String {
        switch self { case .unverified: "Not verified"; case .normal: "Non-holo"; case .holo: "Holo"; case .reverseHolo: "Reverse holo"; case .other: "Other finish" }
    }
}
private extension CardCondition {
    var displayName: String {
        switch self { case .unassessed: "Not assessed"; case .nearMint: "Near mint"; case .lightlyPlayed: "Lightly played"; case .moderatelyPlayed: "Moderately played"; case .heavilyPlayed: "Heavily played"; case .damaged: "Damaged" }
    }
}

extension CardIdentification {
    var editionPresentation: (label: String, detail: String)? {
        editionPresentation(for: nil)
    }

    func editionPresentation(for selection: String?) -> (label: String, detail: String)? {
        guard let edition = finishEstimate?.edition,
              [.present, .uncertain].contains(edition.stamp.state) else { return nil }
        let candidate: CardCandidate?
        if let selection, [.confirmed, .reviewRequired].contains(state) {
            candidate = evidence?.candidates.first { $0.id == selection }
        } else if state == .confirmed {
            candidate = evidence?.candidates.first { $0.id == selectedCardID }
        } else {
            candidate = state == .reviewRequired && evidence?.candidates.count == 1 ? evidence?.candidates.first : nil
        }
        let name = edition.name?.trimmingCharacters(in: .whitespacesAndNewlines)
        let matched = edition.stamp.state == .present && edition.state == .matched &&
            name?.isEmpty == false && candidate != nil && edition.candidateID == candidate?.id
        let label = matched ? name! : edition.stamp.state == .uncertain
            ? "Possible pumpkin stamp" : "Pumpkin stamp · edition unresolved"
        let limitation = matched
            ? "The stamp and catalogue support this edition label. The stamp alone does not establish a printing year or finish."
            : "The stamped edition is unresolved; its ordinary-printing finish and price cannot establish this variant."
        return (label, (edition.stamp.evidence + edition.evidence + [limitation]).joined(separator: " "))
    }

    var finishDescription: String {
        if finish != .unverified { return finish.displayName + " (reviewed)" }
        let candidate = selectedCardID ?? (evidence?.candidates.count == 1 ? evidence?.candidates.first?.id : nil)
        if let estimate = finishEstimate, estimate.state == .estimated, let value = estimate.suggestedFinish, estimate.candidateID == candidate {
            return value.displayName + " (estimated)"
        }
        return finishEstimate == nil ? "Not verified" : "Needs review"
    }
}
