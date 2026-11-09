use scanner_recognition::{
    matching::{CatalogueCard, PokemonMatcher},
    ocr::RecognizedLine,
};
use scanner_types::cards::CardLanguage;
use serde_json::{Value, json};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("usage: match_saved CATALOGUE_JSON METADATA_DIRECTORY READINGS_JSON".into());
    }
    let cards: Vec<CatalogueCard> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let readings: Value = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let cases = readings
        .as_array()
        .or_else(|| readings["results"].as_array())
        .ok_or("Missing readings")?;
    let matcher = PokemonMatcher::new(cards, CardLanguage::English);
    let mut results = vec![];
    for case in cases {
        let lines: Vec<RecognizedLine> = serde_json::from_value(
            case.get("lines")
                .unwrap_or(&case["evidence"]["lines"])
                .clone(),
        )?;
        let mut matches = vec![];
        let mut missing = vec![];
        for rank in matcher.rank(&lines).into_iter().take(8) {
            if !rank
                .card
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"+.-".contains(&c))
            {
                continue;
            }
            let path = args[1].join(format!("metadata-{}.json", rank.card.id));
            if !path.exists() {
                missing.push(rank.card.id);
                continue;
            }
            let detail: Value = serde_json::from_slice(&std::fs::read(path)?)?;
            if detail["id"] != rank.card.id {
                return Err("Wrong detail metadata".into());
            }
            if let Some(total) = detail["set"]["cardCount"]["official"].as_u64()
                && !rank.totals.is_empty()
                && !rank.totals.contains(&(total as u32))
            {
                continue;
            }
            matches.push(rank.card.id);
            if matches.len() == 5 {
                break;
            }
        }
        results.push(json!({"id":case["id"],"matches":matches,"missingMetadata":missing}));
    }
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}
