import ScannerAPI

// Service tests exercise routing with explicit mock back evidence. Pixel-level
// detection is tested separately against saved scans and rotated image fixtures.
func addPokemonBack(to record: inout ScanRecord, sequence: Int) {
    guard let original = record.images.first(where: { $0.sequence == sequence }) else { return }
    var edit = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: sequence), sourceSHA256: original.sha256, image: original)
    edit.image.relativePath = "edit-\(edit.id.uuidString.lowercased()).png"
    edit.cardPreparation = CardPreparation(crop: ImageCrop(), quarterTurns: 0, orientationVerified: true, warnings: [], orientationBasis: "pokemon-back-pattern")
    record.processedImages = (record.processedImages ?? []) + [edit]
}
