import AppKit
import Foundation
import ScannerAPI
import UniformTypeIdentifiers

@MainActor func exportJSON<T: Encodable>(_ value: T, name: String, model: ScannerModel) {
    let panel = NSSavePanel(); panel.nameFieldStringValue = name; panel.allowedContentTypes = [.json]
    panel.begin { response in
        guard response == .OK, let url = panel.url else { return }
        do {
            // Explicitly refuse replacement even after the panel's overwrite confirmation.
            try Wire.encoder().encode(value).write(to: url, options: .withoutOverwriting)
            model.notice = "Exported \(url.lastPathComponent)."
        } catch { model.error = error.localizedDescription }
    }
}
@MainActor func exportImage(record: ScanRecord, image: OriginalImage, editID: UUID? = nil, model: ScannerModel) {
    let panel = NSSavePanel(); panel.nameFieldStringValue = "\(record.id.uuidString.prefix(8))-" + URL(fileURLWithPath: image.relativePath).lastPathComponent
    panel.allowedContentTypes = image.uniformType == "public.png" ? [.png] : [.tiff]
    panel.begin { response in
        guard response == .OK, let url = panel.url else { return }
        model.perform {
            try await model.client.export(jobID: record.id, sequence: image.sequence, editID: editID, to: url)
            model.notice = "Exported \(url.lastPathComponent)."
        }
    }
}
@MainActor func exportBatch(_ record: ScanRecord, model: ScannerModel) {
    let panel = NSOpenPanel(); panel.canChooseDirectories = true; panel.canChooseFiles = false
    panel.canCreateDirectories = true; panel.prompt = "Export here"
    panel.begin { response in
        guard response == .OK, let parent = panel.url else { return }
        model.perform {
            let root = parent.appendingPathComponent("scan-\(record.id.uuidString.lowercased())")
            guard !FileManager.default.fileExists(atPath: root.path) else { throw ScannerError("conflict", "An export already exists here. Choose a different folder.") }
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            do {
                try FileManager.default.createDirectory(at: root.appendingPathComponent("originals"), withIntermediateDirectories: false)
                try FileManager.default.createDirectory(at: root.appendingPathComponent("processed"), withIntermediateDirectories: false)
                for image in record.images {
                    // Construct names locally; never interpret server paths as export paths.
                    let name = String(format: "page-%06d.%@", image.sequence, record.request.format.rawValue)
                    try await model.client.export(jobID: record.id, sequence: image.sequence, to: root.appendingPathComponent("originals").appendingPathComponent(name))
                }
                for edit in record.processedImages ?? [] {
                    try await model.client.export(jobID: record.id, sequence: edit.request.sequence, editID: edit.id,
                        to: root.appendingPathComponent("processed").appendingPathComponent(edit.id.uuidString.lowercased() + ".png"))
                }
                try Wire.encoder().encode(record).write(to: root.appendingPathComponent("manifest.json"), options: .withoutOverwriting)
                model.notice = "Batch exported to \(root.path)."
            } catch {
                throw ScannerError("export", "Export incomplete at \(root.path). Verified files are retained; no completion manifest was written. \(error.localizedDescription)")
            }
        }
    }
}
