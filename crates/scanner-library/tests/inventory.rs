use scanner_library::Inventory;
use scanner_types::{
    cards::CardLanguage,
    identification::{CardIdentification, IdentificationState},
    inventory::InventoryCatalogueCard,
    scan::{ScanRecord, ScanRequest, ScanState},
};
use serde_json::json;
use uuid::Uuid;
fn record(number: u64, language: CardLanguage) -> ScanRecord {
    let mut record = ScanRecord::new(
        Default::default(),
        ScanRequest {
            duplex: true,
            prepare_cards: Some(true),
            card_language: Some(language),
            ..Default::default()
        },
        format!("B-{number:03}"),
    );
    record.batch_number = Some(number);
    record.state = ScanState::Completed;
    for sequence in 1..=2 {
        let image=serde_json::from_value(json!({"sequence":sequence,"relativePath":format!("C-{sequence:03}.png"),"bytes":1,"sha256":format!("source-{sequence}"),"width":100,"height":150,"imageCount":1,"uniformType":"public.png","side":"unknown"})).unwrap();
        record.images.push(image);
        let edit=serde_json::from_value(json!({"request":{"id":Uuid::new_v4(),"jobID":record.id,"sequence":sequence,"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0},"createdAt":"2026-11-09T12:00:00Z","sourceSHA256":format!("source-{sequence}"),"image":record.images.last().unwrap(),"cardPreparation":{"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0,"orientationVerified":true,"orientationBasis":if sequence==2 {"pokemon-back-pattern"}else{"text"},"warnings":[],"method":"fixture"}})).unwrap();
        record
            .processed_images
            .get_or_insert_with(Vec::new)
            .push(edit);
    }
    let identity:CardIdentification=serde_json::from_value(json!({"request":{"id":Uuid::new_v4(),"jobID":record.id,"sequence":1,"language":language},"sourceSHA256":"source-1","state":"reviewRequired","createdAt":"2026-11-09T12:00:00Z","method":"fixture","assessmentSource":"manual-review","evidence":{"lines":[],"candidates":[{"id":"sv09-177","name":"Salamence ex","localID":"177","setID":"sv09","setName":"Journey Together","officialSetCount":159,"evidence":[],"availableFinishes":[]}],"warnings":[],"orientation":0}})).unwrap();
    record.replace_identification(&identity).unwrap();
    record
}
#[test]
fn counts_physical_copies_and_keeps_reviews_languages_filters_and_totals_separate() {
    let first = record(2, CardLanguage::English);
    let mut second = record(1, CardLanguage::English);
    let korean = record(3, CardLanguage::Korean);
    let mut identity = second.identifications().unwrap().remove(0);
    identity.state = IdentificationState::Confirmed;
    identity.selected_card_id = Some("sv09-177".into());
    second.replace_identification(&identity).unwrap();
    let mut inventory = Inventory::default();
    for r in [&first, &second, &korean] {
        inventory.add(r).unwrap();
    }
    let page = inventory.page(1, 0, "", "").unwrap();
    assert_eq!(page.total_copies, 3);
    assert_eq!(page.catalogue_copies, 3);
    assert_eq!(page.matching_entries, 2);
    assert_eq!(page.next_offset, Some(1));
    let entry = &page.entries[0];
    assert_eq!(entry.quantity, 2);
    assert_eq!(entry.confirmed_quantity, 1);
    assert_eq!(entry.copies.len(), 2);
    assert_eq!(entry.preview.as_ref().unwrap().image.sequence, 1);
    assert_eq!(entry.copies[0].batch_number, Some(2));
    assert_eq!(entry.copies[1].batch_number, Some(1));
    let filtered = inventory.page(100, 0, "KO", &entry.set_key).unwrap();
    assert_eq!(filtered.entries.len(), 1);
    assert_eq!(filtered.total_copies, 3);
    assert_eq!(filtered.sets[0].quantity, 3);
    let empty = inventory.page(100, 0, "not-a-card", "").unwrap();
    assert!(empty.entries.is_empty());
    assert_eq!(empty.total_copies, 3);
    let source = inventory.catalogue_source(&entry.set_key).unwrap().unwrap();
    assert_eq!(source.language, CardLanguage::English);
    let missing = inventory.missing(
        vec![
            InventoryCatalogueCard {
                id: "sv09-177".into(),
                name: "Owned".into(),
                number: "177".into(),
                language: "en".into(),
                thumbnail_key: None,
            },
            InventoryCatalogueCard {
                id: "sv09-178".into(),
                name: "Missing".into(),
                number: "178".into(),
                language: "en".into(),
                thumbnail_key: None,
            },
        ],
        &source,
    );
    assert_eq!(missing.catalogue_count, Some(2));
    assert_eq!(missing.cards.len(), 1);
    assert_eq!(missing.cards[0].id, "sv09-178");
}
#[test]
fn excludes_code_inserts_and_keeps_rejected_or_incomplete_pairs_unresolved() {
    let mut rejected = record(1, CardLanguage::English);
    let mut identity = rejected.identifications().unwrap().remove(0);
    identity.state = IdentificationState::Rejected;
    rejected.replace_identification(&identity).unwrap();
    let mut incomplete = record(2, CardLanguage::English);
    incomplete.images.pop();
    let mut code = record(3, CardLanguage::English);
    for edit in code.processed_images.as_mut().unwrap() {
        edit.card_preparation.as_mut().unwrap().orientation_basis =
            Some("pokemon-code-text".into());
    }
    let mut raw = record(4, CardLanguage::English);
    raw.request.prepare_cards = None;
    raw.processed_images = None;
    let mut active = record(5, CardLanguage::English);
    active.state = ScanState::Scanning;
    let mut inventory = Inventory::default();
    for r in [&rejected, &incomplete, &code, &raw, &active] {
        inventory.add(r).unwrap();
    }
    let page = inventory.page(100, 0, "", "").unwrap();
    assert_eq!(page.total_copies, 2);
    assert_eq!(page.unresolved_copies, 2);
    assert_eq!(page.excluded_code_cards, 1);
    assert!(
        page.entries
            .iter()
            .any(|e| e.name == "Incomplete card pair")
    );
    assert!(inventory.page(101, 0, "", "").is_err());
    assert!(inventory.page(1, 0, &"x".repeat(161), "").is_err());
}
#[test]
fn saved_manifest_projection_counts_pairs_without_changing_saved_data() {
    let Some(path) = std::env::var_os("SCANNER_COMPAT_MANIFEST") else {
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let record: ScanRecord = serde_json::from_slice(&bytes).unwrap();
    let mut inventory = Inventory::default();
    inventory.add(&record).unwrap();
    let page = inventory.page(100, 0, "", "").unwrap();
    assert_eq!(
        page.total_copies + page.excluded_code_cards,
        record.images.len().div_ceil(2)
    );
    assert_eq!(
        page.entries.iter().map(|e| e.quantity).sum::<usize>(),
        page.total_copies
    );
    assert!(page.catalogue_copies > 0);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
