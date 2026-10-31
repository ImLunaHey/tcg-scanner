import SwiftUI
import ScannerAPI

struct ConditionReviewView: View {
    @Environment(\.dismiss) private var dismiss
    let model: ScannerModel
    let job: UUID
    let sequence: Int
    @State private var record: ScanRecord?
    @State private var result: CardIdentification?
    @State private var busy = false
    @State private var error: String?
    private var source: OriginalImage? {
        if let edit = result?.sourceEditID { return record?.processedImages?.first { $0.id == edit }?.image }
        return record?.images.first { $0.sequence == sequence }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("Visible condition checks").font(.title2.bold())
                Spacer(); Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            Text("Candidate marks in the blue back border. Inspect the physical card; this does not assign a selling grade.").foregroundStyle(.secondary)
            if let error { Text(error).foregroundStyle(.red) }
            Text(result?.conditionCheck?.summary ?? "Visible marks: not checked").font(.headline)
            Button(busy ? "Checking…" : "Check visible marks locally") { check() }.disabled(busy || result == nil || result?.state == .processing)
            ScrollView {
                HStack(alignment: .top, spacing: 20) {
                    if let source, source.sha256 == result?.sourceSHA256 {
                        ScanThumbnail(model: model, job: job, sequence: sequence, edit: result?.sourceEditID)
                            .frame(width: 330, height: 330 * CGFloat(source.height) / CGFloat(max(1, source.width)))
                            .overlay {
                                GeometryReader { geometry in
                                    ForEach(Array((result?.conditionCheck?.findings ?? []).enumerated()), id: \.offset) { index, finding in
                                        let r = finding.region
                                        Rectangle().stroke(.red, lineWidth: 2).background(.red.opacity(0.15))
                                            .frame(width: geometry.size.width * r.width, height: geometry.size.height * r.height)
                                            .position(x: geometry.size.width * (r.x + r.width / 2), y: geometry.size.height * (r.y + r.height / 2))
                                        Text("\(index + 1)").font(.caption.bold()).foregroundStyle(.white).padding(2).background(.black)
                                            .position(x: geometry.size.width * min(0.95, r.x + 0.02), y: geometry.size.height * max(0.02, r.y))
                                    }
                                }
                            }
                    }
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(Array((result?.conditionCheck?.findings ?? []).enumerated()), id: \.offset) { index, finding in
                            Text("\(index + 1). \(finding.explanation)")
                        }
                        ForEach(result?.conditionCheck?.limitations ?? [], id: \.self) { Text($0).font(.callout).foregroundStyle(.secondary) }
                        Text("Condition remains your manual assessment. No flagged marks does not mean Near Mint.").font(.callout)
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }.padding(24).frame(width: 850, height: 700)
        .task {
            do {
                let record = try await model.client.job(job)
                self.record = record
                result = record.cardIdentifications?.first { $0.request.sequence == sequence }
            } catch { self.error = error.localizedDescription }
        }
    }
    private func check() {
        guard let result else { return }
        busy = true; error = nil
        Task {
            defer { busy = false }
            do {
                self.result = try await model.client.checkCondition(jobID: job, sequence: sequence, identificationID: result.request.id)
                try await model.refresh()
            } catch { self.error = error.localizedDescription }
        }
    }
}

extension ConditionCheck {
    var summary: String {
        guard state == .checked else { return "Visible marks: needs manual inspection" }
        return findings.isEmpty ? "No border marks flagged · not a grade" : "Visible marks: \(findings.count) candidate areas"
    }
}
