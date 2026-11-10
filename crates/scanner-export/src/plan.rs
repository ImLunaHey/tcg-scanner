use crate::error;
use scanner_types::{error::Result, images::OriginalImage, scan::ScanRecord};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone)]
pub struct Entry {
    pub image: OriginalImage,
    pub edit_id: Option<Uuid>,
    pub name: String,
}
#[derive(Clone)]
pub struct Batch {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) entries: Vec<Entry>,
    pub(crate) manifest: Vec<u8>,
    pub(crate) total: u64,
}
impl Batch {
    /// Derive every destination name locally. Server paths remain provenance only.
    pub fn new(record: &ScanRecord) -> Result<Self> {
        if record.images.len() > 1000
            || record.processed_images.as_deref().unwrap_or_default().len() > 8000
        {
            return Err(error("Batch contains too many images."));
        }
        let mut manifest = record.clone();
        let mut entries = Vec::new();
        let mut sequences = BTreeSet::new();
        let mut edits = BTreeSet::new();
        for image in &mut manifest.images {
            if !sequences.insert(image.sequence) {
                return Err(error("Duplicate original sequence in batch."));
            }
            let name = original_name(image)?;
            entries.push(Entry {
                image: image.clone(),
                edit_id: None,
                name: name.clone(),
            });
            image.relative_path = name;
        }
        for edit in manifest.processed_images.as_mut().into_iter().flatten() {
            if !edits.insert(edit.request.id)
                || edit.request.job_id != record.id
                || edit.image.sequence != edit.request.sequence
                || !sequences.contains(&edit.request.sequence)
                || edit.image.uniform_type != "public.png"
            {
                return Err(error("Invalid or duplicate edited image in batch."));
            }
            validate(&edit.image)?;
            let name = format!("edit-{}.png", edit.request.id);
            entries.push(Entry {
                image: edit.image.clone(),
                edit_id: Some(edit.request.id),
                name: name.clone(),
            });
            edit.image.relative_path = name;
        }
        let total = entries
            .iter()
            .try_fold(0u64, |total, e| total.checked_add(e.image.bytes))
            .ok_or_else(|| error("Batch byte count overflow."))?;
        let manifest = serde_json::to_vec_pretty(&manifest)
            .map_err(|_| error("Could not encode export manifest."))?;
        if manifest.len() > 16 * 1024 * 1024 {
            return Err(error("Export manifest exceeds 16 MiB."));
        }
        Ok(Self {
            id: record.id,
            name: record.batch_label(),
            entries,
            manifest,
            total,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn total_bytes(&self) -> u64 {
        self.total
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
}
pub fn original_name(image: &OriginalImage) -> Result<String> {
    validate(image)?;
    let extension = match image.uniform_type.as_str() {
        "public.png" => "png",
        "public.tiff" => "tiff",
        _ => return Err(error("Unsupported saved image format.")),
    };
    Ok(format!("C-{:03}.{extension}", image.sequence))
}
fn validate(image: &OriginalImage) -> Result<()> {
    if !(1..=1000).contains(&image.sequence) || !(1..=512 * 1024 * 1024).contains(&image.bytes) {
        return Err(error("Saved image sequence or size exceeds export limits."));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destination_names_never_interpret_server_paths_and_manifest_edits_are_rewritten() {
        let mut record = ScanRecord::new(
            Default::default(),
            Default::default(),
            "/original/location".into(),
        );
        let image:OriginalImage=serde_json::from_value(serde_json::json!({"sequence":1,"relativePath":"../../evil.png","bytes":5,"sha256":"a".repeat(64),"width":100,"height":150,"imageCount":1,"uniformType":"public.png","side":"unknown"})).unwrap();
        record.images.push(image.clone());
        let edit = Uuid::new_v4();
        record.processed_images=Some(vec![serde_json::from_value(serde_json::json!({"request":{"id":edit,"jobID":record.id,"sequence":1,"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0},"createdAt":"2026-11-10T00:00:00Z","sourceSHA256":image.sha256,"image":image})).unwrap()]);
        let plan = Batch::new(&record).unwrap();
        assert_eq!(plan.entries[0].name, "C-001.png");
        assert_eq!(plan.entries[1].name, format!("edit-{edit}.png"));
        let manifest: ScanRecord = serde_json::from_slice(plan.manifest()).unwrap();
        assert_eq!(manifest.images[0].relative_path, "C-001.png");
        assert_eq!(
            manifest.processed_images.unwrap()[0].image.relative_path,
            plan.entries[1].name
        );
        assert_eq!(record.images[0].relative_path, "../../evil.png");
        record.images.push(image);
        assert!(Batch::new(&record).is_err());
    }
}
