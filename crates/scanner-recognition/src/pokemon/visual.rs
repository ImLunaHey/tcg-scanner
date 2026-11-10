use crate::embedding::VisualRetriever;
use scanner_catalogue::TcgDex;
use scanner_types::{
    cards::CardLanguage,
    error::Result,
    recognition::{CardCandidate, RecognitionEvidence},
    visual::VisualMatch,
};

/// Preserve nearest references as explicitly separate suggestions. No cosine
/// threshold here pretends to verify a collector number, foil or exact printing.
pub(super) async fn attach(
    evidence: &mut RecognitionEvidence,
    bytes: Vec<u8>,
    language: CardLanguage,
    retriever: &VisualRetriever,
    catalogue: &TcgDex,
) -> Result<()> {
    let ranked = retriever
        .search(bytes, evidence.orientation, language)
        .await?;
    let mut matches = vec![];
    for result in ranked {
        let detail = catalogue.detail(&result.id).await?;
        let candidate=CardCandidate {id:detail.id.clone(),name:detail.name.clone(),local_id:detail.local_id.clone(),set_id:detail.set.id.clone(),set_name:detail.set.name.clone(),
            official_set_count:detail.set.card_count.as_ref().and_then(|c|c.official),category:detail.category.clone(),types:detail.types.clone(),
            evidence:vec!["Similar public artwork (DINOv3). Read the printed set and number before confirming; similarity does not establish exact printing, finish or price.".into()],available_finishes:vec![],extensions:Default::default()};
        matches.push(VisualMatch {
            candidate,
            similarity: result.similarity,
        });
    }
    if !matches.is_empty() {
        evidence
            .extensions
            .insert("visualMatches".into(), serde_json::json!(matches));
        evidence
            .extensions
            .insert("visualModel".into(), serde_json::json!(retriever.model()));
        evidence.extensions.insert(
            "visualReferenceCount".into(),
            serde_json::json!(retriever.reference_count()),
        );
        evidence.warnings.push("Visual matches are artwork suggestions from a partial reference index. Confirm the printed set and number; nearest artwork is not an exact printing match.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};
    struct Saved(PathBuf);
    #[async_trait::async_trait]
    impl scanner_catalogue::JsonTransport for Saved {
        async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
            let file = if path == "en/cards" {
                self.0.join("catalogue.json")
            } else {
                self.0.join(format!(
                    "metadata-{}.json",
                    path.strip_prefix("en/cards/").unwrap()
                ))
            };
            if file.exists() {
                return std::fs::read(file).map_err(super::super::error);
            }
            // Opt-in integration benchmark: only missing public catalogue IDs
            // are fetched; saved card pixels never leave the process.
            let detail = TcgDex::new(CardLanguage::English)?
                .detail(path.strip_prefix("en/cards/").unwrap())
                .await?;
            let bytes = serde_json::to_vec(detail.as_ref()).map_err(super::super::error)?;
            std::fs::write(file, &bytes).map_err(super::super::error)?;
            Ok(bytes)
        }
    }
    #[tokio::test]
    async fn fresh_saved_visual_recognizer_uses_portable_ocr_and_keeps_artwork_separate() {
        if std::env::var_os("SCANNER_FRESH_VISUAL").is_none() {
            return;
        }
        let path = |name| PathBuf::from(std::env::var_os(name).unwrap());
        let directory = path("SCANNER_BENCH_DIR");
        let visual = Arc::new(
            VisualRetriever::load(
                &path("SCANNER_ORT_LIBRARY"),
                &path("SCANNER_DINO_MODEL"),
                &path("SCANNER_VISUAL_INDEX"),
            )
            .unwrap(),
        );
        let mut recognizer = super::super::PokemonRecognizer::new(
            path("SCANNER_TESSERACT_LIBRARY"),
            path("SCANNER_TESSERACT_DATA"),
        )
        .unwrap()
        .with_visual(visual);
        recognizer.english =
            TcgDex::with_transport(CardLanguage::English, Arc::new(Saved(directory.clone())));
        let input: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("input.json")).unwrap()).unwrap();
        for id in ["current-010", "regression-016", "hard-046"] {
            let fixture = input
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["id"] == id)
                .unwrap();
            let evidence = recognizer
                .recognize(
                    std::fs::read(fixture["path"].as_str().unwrap()).unwrap(),
                    CardLanguage::English,
                )
                .await
                .unwrap();
            let visual = evidence.visual_matches();
            eprintln!(
                "Fresh visual {id}: orientation={}, printed={:?}, visual={:?}",
                evidence.orientation,
                evidence
                    .candidates
                    .iter()
                    .map(|c| &c.id)
                    .collect::<Vec<_>>(),
                visual.first().map(|v| &v.candidate.id)
            );
            assert_eq!(
                visual.first().map(|v| v.candidate.id.as_str()),
                fixture["expected"].as_str()
            );
            assert!(visual[0].candidate.available_finishes.is_empty());
        }
    }
    #[tokio::test]
    async fn saved_production_visual_index_adds_review_options_without_changing_printed_candidates()
    {
        let Some(index) = std::env::var_os("SCANNER_VISUAL_INDEX") else {
            return;
        };
        let directory = PathBuf::from(std::env::var_os("SCANNER_BENCH_DIR").unwrap());
        let runtime = PathBuf::from(std::env::var_os("SCANNER_ORT_LIBRARY").unwrap());
        let model = PathBuf::from(std::env::var_os("SCANNER_DINO_MODEL").unwrap());
        let retriever = VisualRetriever::load(&runtime, &model, &PathBuf::from(&index)).unwrap();
        assert!(
            retriever
                .search(vec![], 0, CardLanguage::Korean)
                .await
                .unwrap()
                .is_empty()
        );
        let catalogue =
            TcgDex::with_transport(CardLanguage::English, Arc::new(Saved(directory.clone())));
        let inputs: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("input.json")).unwrap()).unwrap();
        let readings: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("rust-regional.json")).unwrap())
                .unwrap();
        let document = crate::embedding::VisualIndexDocument::load(&PathBuf::from(index)).unwrap();
        let (mut text_correct, mut covered, mut visual_correct, mut assisted) = (0, 0, 0, 0);
        let mut results = vec![];
        for input in inputs
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["expected"].is_string())
        {
            let case = readings["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == input["id"])
                .unwrap();
            let (text_ok, visual_ok, matches) = measure(input, case, &retriever, &catalogue).await;
            text_correct += usize::from(text_ok);
            let is_covered = document
                .references
                .iter()
                .any(|r| r.id == input["expected"].as_str().unwrap());
            covered += usize::from(is_covered);
            visual_correct += usize::from(visual_ok);
            assisted += usize::from(text_ok || visual_ok);
            results.push(serde_json::json!({"id":input["id"],"expected":input["expected"],"textCorrectUnique":text_ok,"covered":is_covered,"visualTop1Correct":visual_ok,"visual":matches.iter().map(|m|serde_json::json!({"id":m.candidate.id,"similarity":m.similarity})).collect::<Vec<_>>()}));
        }
        eprintln!(
            "Production visual review: text unique={text_correct}, reference coverage={covered}, correct visual top1={visual_correct}, text-or-visual correct={assisted}/{}",
            results.len()
        );
        assert_eq!(covered, 48);
        assert_eq!(visual_correct, covered);
        assert!(assisted > text_correct);
        if let Some(output) = std::env::var_os("SCANNER_VISUAL_REPORT") {
            std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"textCorrectUnique":text_correct,"covered":covered,"visualTop1Correct":visual_correct,"textOrVisualCorrect":assisted,"results":results})).unwrap()).unwrap();
        }
    }
    async fn measure(
        input: &serde_json::Value,
        case: &serde_json::Value,
        retriever: &VisualRetriever,
        catalogue: &TcgDex,
    ) -> (bool, bool, Vec<VisualMatch>) {
        let regional = ["title", "footer", "left", "right"]
            .into_iter()
            .flat_map(|k| {
                serde_json::from_value::<Vec<scanner_types::recognition::RecognizedLine>>(
                    case["readings"][k].clone(),
                )
                .unwrap()
            })
            .collect();
        let mut evidence = super::super::resolve(
            vec![
                crate::readings::Reading {
                    turns: 0,
                    lines: regional,
                },
                crate::readings::Reading {
                    turns: 0,
                    lines: serde_json::from_value(case["readings"]["whole"].clone()).unwrap(),
                },
            ],
            CardLanguage::English,
            catalogue,
        )
        .await
        .unwrap();
        let text_ok = evidence.candidates.len() == 1
            && evidence.candidates[0].id == input["expected"].as_str().unwrap();
        let before = serde_json::to_value(&evidence.candidates).unwrap();
        let bytes = std::fs::read(input["path"].as_str().unwrap()).unwrap();
        use sha2::Digest;
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(&bytes)),
            input["sha256"].as_str().unwrap()
        );
        attach(
            &mut evidence,
            bytes,
            CardLanguage::English,
            retriever,
            catalogue,
        )
        .await
        .unwrap();
        assert_eq!(serde_json::to_value(&evidence.candidates).unwrap(), before);
        let matches = evidence.visual_matches();
        let visual_ok = matches
            .first()
            .is_some_and(|v| v.candidate.id == input["expected"].as_str().unwrap());
        (text_ok, visual_ok, matches)
    }
}
