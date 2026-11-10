//! Bounded native OCR and public catalogue matching for English YGO and SWU.
use crate::{
    readings::{self, Reading},
    star_wars, yugioh,
};
use scanner_catalogue::{persistent::Documents, star_wars::StarWars, yugioh::Yugioh};
use scanner_types::{
    cards::{CardGame, CardLanguage},
    error::{Result, ScannerError},
    recognition::RecognitionEvidence,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
pub struct OtherGames {
    library: PathBuf,
    data: PathBuf,
    workers: Arc<Semaphore>,
    yugioh: Yugioh,
    star_wars: StarWars,
}
impl OtherGames {
    pub fn new(library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            library,
            data,
            workers: Arc::new(Semaphore::new(1)),
            yugioh: Yugioh::new()?,
            star_wars: StarWars::new()?,
        })
    }
    pub fn with_documents(mut self, documents: Arc<Documents>) -> Self {
        self.yugioh = self.yugioh.with_documents(documents.clone());
        self.star_wars = self.star_wars.with_documents(documents);
        self
    }
    pub async fn recognize(
        &self,
        game: CardGame,
        bytes: Vec<u8>,
        language: CardLanguage,
    ) -> Result<RecognitionEvidence> {
        self.recognize_with_options(game, bytes, language, readings::Options::default())
            .await
    }
    pub(crate) async fn recognize_with_options(
        &self,
        game: CardGame,
        bytes: Vec<u8>,
        language: CardLanguage,
        options: readings::Options,
    ) -> Result<RecognitionEvidence> {
        if language != CardLanguage::English {
            return Err(ScannerError::new(
                "unsupportedLanguage",
                "Yu-Gi-Oh! and Star Wars Unlimited catalogue matching supports English only.",
            ));
        }
        if !matches!(game, CardGame::Yugioh | CardGame::StarwarsUnlimited) {
            return Err(ScannerError::new(
                "unsupported",
                "No recognizer for this game.",
            ));
        }
        let permit = self.workers.clone().acquire_owned().await.map_err(error)?;
        let (library, data) = (self.library.clone(), self.data.clone());
        let (readings, permit) = tokio::task::spawn_blocking(move || {
            (
                readings::read_options(&bytes, &library, &data, language, game, options),
                permit,
            )
        })
        .await
        .map_err(error)?;
        let _permit = permit;
        let readings = readings?;
        if game == CardGame::Yugioh {
            let cards = self.yugioh.cards().await?;
            tokio::task::spawn_blocking(move || {
                let _permit = _permit;
                resolve_yugioh(readings, &cards)
            })
            .await
            .map_err(error)
        } else {
            resolve_star_wars(readings, &self.star_wars).await
        }
    }
}
fn error(e: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognition", e.to_string())
}
fn resolve_yugioh(
    readings: Vec<Reading>,
    cards: &[scanner_catalogue::yugioh::Card],
) -> RecognitionEvidence {
    let mut results: Vec<_> = readings
        .into_iter()
        .map(|r| {
            let result = yugioh::match_card(&r.lines, cards);
            (r, result)
        })
        .collect();
    results.sort_by_key(|(_, m)| std::cmp::Reverse(m.score));
    let mut results = results.into_iter();
    let Some((reading, mut best)) = results.next() else {
        return RecognitionEvidence::default();
    };
    for (_, m) in results.filter(|(_, m)| m.score == best.score && m.score > 0) {
        for candidate in m.candidates {
            if !best.candidates.iter().any(|c| c.id == candidate.id) && best.candidates.len() < 5 {
                best.candidates.push(candidate);
            }
        }
    }
    if best.candidates.len() > 1 {
        best.warnings.push("Independent OCR passes disagree; verify the printed name, passcode and set before confirming.".into());
    }
    best.warnings
        .push("Catalogue suggestions require manual confirmation.".into());
    RecognitionEvidence {
        lines: reading.lines,
        candidates: best.candidates,
        warnings: best.warnings,
        orientation: reading.turns,
        extensions: Default::default(),
    }
}
async fn resolve_star_wars(
    readings: Vec<Reading>,
    catalogue: &StarWars,
) -> Result<RecognitionEvidence> {
    let fallback = readings
        .iter()
        .max_by_key(|r| {
            r.lines
                .iter()
                .filter(|l| l.confidence >= 0.5)
                .map(|l| l.text.chars().filter(|c| c.is_alphanumeric()).count())
                .filter(|n| *n >= 3)
                .sum::<usize>()
        })
        .cloned();
    let identified: Vec<_> = readings
        .into_iter()
        .filter_map(|reading| {
            let ids = star_wars::printed::identifiers(
                reading
                    .lines
                    .iter()
                    .filter(|l| l.confidence >= 0.5)
                    .map(|l| l.text.as_str()),
            );
            if ids.len() != 1 {
                return None;
            }
            Some((reading, ids.into_iter().next().unwrap()))
        })
        .collect();
    let mut matches = vec![];
    if !identified.is_empty() {
        let sets = catalogue.sets().await?;
        for (reading, id) in identified {
            let Some(set) = sets
                .iter()
                .find(|s| s.set_id == id.set && id.total.is_none_or(|t| t == s.number_cards))
            else {
                continue;
            };
            let cards = catalogue.cards(&set.set_id).await?;
            let candidates = star_wars::match_card(&reading.lines, &id, set, &cards);
            if !candidates.is_empty() {
                matches.push((reading, candidates));
            }
        }
    }
    let ids: std::collections::BTreeSet<_> = matches
        .iter()
        .flat_map(|(_, cs)| cs.iter().map(|c| &c.id))
        .collect();
    if ids.len() == 1 {
        let (reading, candidates) = matches.remove(0);
        return Ok(RecognitionEvidence{lines:reading.lines,candidates,warnings:vec!["Catalogue match requires review. Foil finish and condition are not assessed for Star Wars Unlimited.".into()],orientation:reading.turns,extensions:Default::default()});
    }
    let reading = fallback.unwrap_or(Reading {
        turns: 0,
        lines: vec![],
    });
    Ok(RecognitionEvidence{lines:reading.lines,orientation:reading.turns,warnings:vec!["No unique SWU printing matched the printed set, number, total and name. Tokens and unreadable or conflicting identifiers remain unresolved.".into()],..Default::default()})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn line(text: &str, y: f64) -> scanner_types::recognition::RecognizedLine {
        scanner_types::recognition::RecognizedLine {
            text: text.into(),
            confidence: 0.9,
            x: 0.,
            y,
            width: 0.8,
            height: 0.05,
        }
    }
    #[test]
    fn equally_supported_yugioh_passes_remain_ambiguous() {
        let card=serde_json::from_value(serde_json::json!({"id":39706423,"name":"GMX Suppression Squad","type":"Effect Monster","card_sets":[
            {"set_name":"First","set_code":"BLZD-EN091","set_rarity":"Common"},
            {"set_name":"Second","set_code":"UP02-EN008","set_rarity":"Rare"}
        ]})).unwrap();
        let readings = ["BLZD-EN091", "UP02-EN008"]
            .into_iter()
            .map(|code| Reading {
                turns: 0,
                lines: vec![
                    line("GMX Suppression Squad", 0.9),
                    line(code, 0.3),
                    line("39706423", 0.02),
                ],
            })
            .collect();
        let result = resolve_yugioh(readings, &[card]);
        assert_eq!(result.candidates.len(), 2);
        assert!(result.warnings.iter().any(|w| w.contains("disagree")));
    }
    struct Fixture;
    #[async_trait::async_trait]
    impl scanner_catalogue::JsonTransport for Fixture {
        async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
            Ok(serde_json::to_vec(&match path {
                "sets"=>serde_json::json!([{"setId":"ASH","fullName":"Ashes of the Empire","numberCards":264}]),
                "cards/ash"=>serde_json::json!({"data":[
                    {"Set":"ASH","Number":"206","Name":"First Card","Type":"Unit"},
                    {"Set":"ASH","Number":"192","Name":"Second Card","Type":"Unit"}
                ]}),
                _=>panic!("unexpected catalogue request"),
            }).unwrap())
        }
    }
    #[tokio::test]
    async fn conflicting_swu_printings_do_not_select_a_winner() {
        let catalogue = StarWars::with_transport(Arc::new(Fixture));
        let first = Reading {
            turns: 0,
            lines: vec![line("First Card", 0.9), line("ASH EN 206/264", 0.05)],
        };
        let result = resolve_star_wars(vec![first.clone()], &catalogue)
            .await
            .unwrap();
        assert_eq!(result.candidates[0].id, "swu-db:ASH-206");
        let second = Reading {
            turns: 2,
            lines: vec![line("Second Card", 0.9), line("ASH EN 192/264", 0.05)],
        };
        let result = resolve_star_wars(vec![first, second], &catalogue)
            .await
            .unwrap();
        assert!(result.candidates.is_empty());
    }
    #[tokio::test]
    async fn unsupported_language_fails_before_loading_native_ocr() {
        let recognizer =
            OtherGames::new("/missing/native-library".into(), "/missing/data".into()).unwrap();
        for game in [CardGame::Yugioh, CardGame::StarwarsUnlimited] {
            assert_eq!(
                recognizer
                    .recognize(game, vec![], CardLanguage::Korean)
                    .await
                    .unwrap_err()
                    .kind,
                "unsupportedLanguage"
            );
        }
    }
}
