import AppKit
import SwiftUI
import ScannerAPI
import ScannerClient

actor ThumbnailLoader {
    private var tail: Task<Void, Never>?
    private var cache: [String: Data] = [:]
    func load(client: ScannerClient, job: UUID, sequence: Int, edit: UUID?) async throws -> Data {
        let key = "\(job)/\(sequence)/\(edit?.uuidString ?? "original")"
        if let cached = cache[key] { return cached }
        let previous = tail
        let task = Task {
            await previous?.value
            return try await client.thumbnail(jobID: job, sequence: sequence, editID: edit)
        }
        tail = Task { _ = try? await task.value }
        let result = try await task.value
        if cache.count >= 50 { cache.removeAll(keepingCapacity: true) }
        cache[key] = result
        return result
    }
}
struct ScanThumbnail: View {
    let model: ScannerModel
    let job: UUID
    let sequence: Int
    var edit: UUID?
    @State private var image: NSImage?
    @State private var failure: String?
    @State private var attempt = 0
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 8).fill(.quaternary.opacity(0.4))
            if let image { Image(nsImage: image).resizable().scaledToFit() }
            else if let failure {
                VStack { Image(systemName: "photo.badge.exclamationmark"); Text(failure).font(.caption); Button("Retry preview") { attempt += 1 } }.padding()
            } else { ProgressView() }
        }
        .task(id: "\(job)/\(sequence)/\(edit?.uuidString ?? "original")/\(attempt)") {
            image = nil; failure = nil
            do {
                let data = try await model.thumbnails.load(client: model.client, job: job, sequence: sequence, edit: edit)
                try Task.checkCancellation()
                image = NSImage(data: data)
            } catch is CancellationError { } catch { failure = error.localizedDescription }
        }
    }
}

struct ImageEditor: View {
    @Environment(\.dismiss) private var dismiss
    let model: ScannerModel
    let job: UUID
    let image: OriginalImage
    @State private var crop = ImageCrop()
    @State private var turns = 0
    @State private var saving = false
    @State private var failure: String?
    @State private var pending: ImageEditRequest?
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Crop and rotate · image \(image.sequence)").font(.title2.bold())
            Text("Drag a rectangle over the original. The selection is cropped first, then rotated into a new PNG.").foregroundStyle(.secondary)
            ScanThumbnail(model: model, job: job, sequence: image.sequence)
                .aspectRatio(CGFloat(image.width) / CGFloat(max(image.height, 1)), contentMode: .fit)
                .overlay {
                    GeometryReader { geometry in
                        Rectangle().stroke(.orange, lineWidth: 2)
                            .background(.orange.opacity(0.12))
                            .frame(width: geometry.size.width * crop.width, height: geometry.size.height * crop.height)
                            .position(x: geometry.size.width * (crop.x + crop.width / 2), y: geometry.size.height * (crop.y + crop.height / 2))
                        Color.clear.contentShape(Rectangle()).gesture(DragGesture(minimumDistance: 2).onChanged { value in
                            guard pending == nil else { return }
                            let w = geometry.size.width; let h = geometry.size.height
                            let x1 = min(max(value.startLocation.x / w, 0), 1); let x2 = min(max(value.location.x / w, 0), 1)
                            let y1 = min(max(value.startLocation.y / h, 0), 1); let y2 = min(max(value.location.y / h, 0), 1)
                            crop = ImageCrop(x: min(x1, x2), y: min(y1, y2), width: max(abs(x2 - x1), 0.001), height: max(abs(y2 - y1), 0.001))
                        })
                    }
                }.frame(maxWidth: .infinity, maxHeight: 430)
            HStack {
                Button("Reset crop") { crop = ImageCrop() }.disabled(pending != nil)
                Text("\(Int(crop.width * 100))% × \(Int(crop.height * 100))% of original").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Picker("Clockwise rotation", selection: $turns) { ForEach(0..<4) { Text("\($0 * 90)°").tag($0) } }.frame(width: 220).disabled(pending != nil)
            }
            if let failure { Text(failure).foregroundStyle(.red) }
            HStack {
                Button("Close") { dismiss() }.keyboardShortcut(.cancelAction).disabled(saving)
                Spacer()
                if saving { ProgressView().controlSize(.small) }
                Button(pending == nil ? "Save new PNG" : "Retry same edit") {
                    saving = true
                    let request = pending ?? ImageEditRequest(jobID: job, sequence: image.sequence, crop: crop, quarterTurns: turns)
                    pending = request
                    Task {
                        defer { saving = false }
                        do { _ = try await model.client.edit(request); try await model.refresh(); dismiss() }
                        catch { failure = error.localizedDescription }
                    }
                }.buttonStyle(.borderedProminent).disabled(saving || crop.x + crop.width > 1 || crop.y + crop.height > 1)
            }
        }.padding(24).frame(width: 650, height: 680).interactiveDismissDisabled(saving)
        .onAppear {
            if let prepared = model.detail?.processedImages?.last(where: { $0.request.sequence == image.sequence && $0.cardPreparation != nil }) {
                crop = prepared.request.crop; turns = prepared.request.quarterTurns
            }
        }
    }
}
