//! Native OCR and conservative catalogue suggestions. No automatic confirmation.
mod candidates;
mod printed;
mod supplement;
use crate::matching::{PokemonMatcher, footer_orientation_score};
use crate::readings;
use scanner_catalogue::TcgDex;
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
    recognition::RecognitionEvidence,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
pub struct PokemonRecognizer {
    library: PathBuf,
    data: PathBuf,
    english: TcgDex,
    korean: TcgDex,
    workers: Arc<Semaphore>,
}
impl PokemonRecognizer {
    pub fn new(library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            library,
            data,
            english: TcgDex::new(CardLanguage::English)?,
            korean: TcgDex::new(CardLanguage::Korean)?,
            workers: Arc::new(Semaphore::new(1)),
        })
    }
    pub fn with_documents(
        mut self,
        documents: Arc<scanner_catalogue::persistent::Documents>,
    ) -> Self {
        self.english = self.english.with_documents(documents.clone());
        self.korean = self.korean.with_documents(documents);
        self
    }
    pub async fn recognize(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
    ) -> Result<RecognitionEvidence> {
        self.recognize_with_options(bytes, language, readings::Options::default())
            .await
    }
    pub(crate) async fn recognize_with_options(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
        options: readings::Options,
    ) -> Result<RecognitionEvidence> {
        let permit = self.workers.clone().acquire_owned().await.map_err(error)?;
        let library = self.library.clone();
        let data = self.data.clone();
        let (readings, bytes, permit) = tokio::task::spawn_blocking(move || {
            let result = readings::read_options(
                &bytes,
                &library,
                &data,
                language,
                scanner_types::cards::CardGame::Pokemon,
                options,
            );
            (result, bytes, permit)
        })
        .await
        .map_err(error)?;
        let readings = readings?;
        if readings.iter().all(|r| r.lines.is_empty()) {
            return Ok(RecognitionEvidence {
                warnings: vec!["No readable card text. Select the front image.".into()],
                ..Default::default()
            });
        }
        let catalogue = match language {
            CardLanguage::English => &self.english,
            CardLanguage::Korean => &self.korean,
        };
        let evidence = resolve(readings, language, catalogue).await?;
        let (library, data) = (self.library.clone(), self.data.clone());
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            supplement::apply(evidence, &bytes, language, &library, &data, options)
        })
        .await
        .map_err(error)
    }
}
async fn resolve(
    readings: Vec<readings::Reading>,
    language: CardLanguage,
    catalogue: &TcgDex,
) -> Result<RecognitionEvidence> {
    let matcher = PokemonMatcher::new(catalogue.catalogue().await?.as_ref().clone(), language);
    let mut ranked: Vec<_> = readings
        .into_iter()
        .map(|r| {
            let ranks = matcher.rank(&r.lines);
            (r, ranks)
        })
        .collect();
    ranked.sort_by_key(|(r, ranks)| {
        std::cmp::Reverse((
            footer_orientation_score(&r.lines),
            ranks.first().map_or(0, |r| r.score),
        ))
    });
    let mut best = ranked.remove(0);
    let mut matches = candidates::resolve(&best.1, catalogue).await?;
    // Evaluate whole-card and regional evidence independently at the selected
    // orientation. Never merge OCR strings into an invented collector number.
    // If valid passes disagree, retain both suggestions for manual review.
    let turns = best.0.turns;
    let mut strength = retain_supported(&mut matches);
    let mut disagreement = false;
    for (reading, ranks) in ranked.iter().filter(|(r, _)| r.turns == turns) {
        let mut additional = candidates::resolve(ranks, catalogue).await?;
        let support = retain_supported(&mut additional);
        if additional.is_empty() || support < strength {
            continue;
        }
        if matches.is_empty() || support > strength {
            best = (reading.clone(), ranks.clone());
            matches = additional;
            strength = support;
            disagreement = false;
        } else {
            for candidate in additional {
                if !matches.iter().any(|c| c.id == candidate.id) {
                    disagreement = true;
                    if matches.len() < 5 {
                        matches.push(candidate);
                    }
                }
            }
        }
    }
    let mut warnings = warnings(language, matches.is_empty(), best.1.len());
    if disagreement {
        warnings.push("Independent OCR passes suggest different printings; verify the printed set and number before confirming.".into());
    }
    Ok(RecognitionEvidence {
        lines: best.0.lines,
        candidates: matches,
        warnings,
        orientation: best.0.turns,
        extensions: Default::default(),
    })
}
fn retain_supported(candidates: &mut Vec<scanner_types::recognition::CardCandidate>) -> u32 {
    let support = |candidate: &scanner_types::recognition::CardCandidate| {
        u32::from(
            candidate
                .evidence
                .iter()
                .any(|e| e.starts_with("Collector number ")),
        ) + 2 * u32::from(
            candidate
                .evidence
                .iter()
                .any(|e| e.starts_with("Printed set total ") || e.starts_with("Printed set code ")),
        )
    };
    let maximum = candidates.iter().map(support).max().unwrap_or(0);
    candidates.retain(|c| support(c) == maximum);
    maximum
}
fn error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognition", error.to_string())
}

fn warnings(language: CardLanguage, no_match: bool, count: usize) -> Vec<String> {
    let mut warnings=vec!["Suggestions require manual confirmation. A base-card match does not establish finish, stamped edition or condition.".into()];
    if language == CardLanguage::Korean {
        warnings.push(
            "Korean catalogue coverage is limited; no English printing is substituted.".into(),
        );
    }
    if no_match {
        warnings.push(
            "No supported text match. Review orientation, text clarity and catalogue coverage."
                .into(),
        );
    }
    if count > 5 {
        warnings.push("Showing at most five candidates; other printings may match.".into());
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_catalogue::JsonTransport;
    struct Saved(std::path::PathBuf);
    #[async_trait::async_trait]
    impl JsonTransport for Saved {
        async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
            let file = if path == "en/cards" {
                self.0.join("catalogue.json")
            } else {
                self.0.join(format!(
                    "metadata-{}.json",
                    path.strip_prefix("en/cards/").unwrap()
                ))
            };
            std::fs::read(file).map_err(error)
        }
    }
    #[tokio::test]
    async fn saved_independent_ocr_passes_never_create_a_wrong_unique_suggestion() {
        let Some(directory) = std::env::var_os("SCANNER_BENCH_DIR") else {
            return;
        };
        let directory = PathBuf::from(directory);
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("rust-regional.json")).unwrap())
                .unwrap();
        let catalogue = TcgDex::with_transport(CardLanguage::English, Arc::new(Saved(directory)));
        let mut correct = 0;
        let mut ambiguous = 0;
        let mut empty = 0;
        let mut wrong = vec![];
        for case in document["results"].as_array().unwrap() {
            let whole = serde_json::from_value(case["readings"]["whole"].clone()).unwrap();
            let regional: Vec<_> = ["title", "footer", "left", "right"]
                .into_iter()
                .flat_map(|key| {
                    serde_json::from_value::<Vec<scanner_types::recognition::RecognizedLine>>(
                        case["readings"][key].clone(),
                    )
                    .unwrap()
                })
                .collect();
            let result = resolve(
                vec![
                    readings::Reading {
                        turns: 0,
                        lines: regional,
                    },
                    readings::Reading {
                        turns: 0,
                        lines: whole,
                    },
                ],
                CardLanguage::English,
                &catalogue,
            )
            .await
            .unwrap();
            match result.candidates.as_slice() {
                [] => empty += 1,
                [only] if only.id == case["expected"] => correct += 1,
                [only] => wrong.push((case["id"].clone(), only.id.clone())),
                _ => ambiguous += 1,
            }
        }
        eprintln!(
            "Saved OCR pipeline: {correct} correct unique, {ambiguous} ambiguous, {empty} unresolved, {} wrong unique",
            wrong.len()
        );
        assert!(wrong.is_empty(), "{wrong:?}");
        assert!(correct >= 35);
    }
}
