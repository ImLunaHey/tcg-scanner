//! In-memory camera lookup using the same conservative catalogue matchers.
use crate::{
    finish::FinishAnalyzer, ocr::Tesseract, orientation::Orientation, other_games::OtherGames,
    pokemon::PokemonRecognizer, readings::Options,
};
use scanner_types::{
    camera::{CameraIdentification, CameraState},
    cards::{CardGame, CardLanguage},
    error::{Result, ScannerError},
    images::CardPreparation,
    recognition::RecognitionEvidence,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

pub struct CameraRecognizer {
    library: PathBuf,
    data: PathBuf,
    workers: Arc<Semaphore>,
    pokemon: PokemonRecognizer,
    other: OtherGames,
    finish: FinishAnalyzer,
}
impl CameraRecognizer {
    pub fn with_visual(mut self, visual: Arc<crate::embedding::VisualRetriever>) -> Self {
        self.pokemon = self.pokemon.with_visual(visual);
        self
    }
    pub fn new(library: PathBuf, data: PathBuf) -> Result<Self> {
        Ok(Self {
            pokemon: PokemonRecognizer::new(library.clone(), data.clone())?,
            other: OtherGames::new(library.clone(), data.clone())?,
            finish: FinishAnalyzer::new(library.clone(), data.clone())?,
            library,
            data,
            workers: Arc::new(Semaphore::new(1)),
        })
    }
    pub fn with_documents(
        mut self,
        documents: Arc<scanner_catalogue::persistent::Documents>,
    ) -> Self {
        self.pokemon = self.pokemon.with_documents(documents.clone());
        self.other = self.other.with_documents(documents.clone());
        self.finish = self.finish.with_documents(documents);
        self
    }
    /// One operation at a time; the native worker keeps its permit even if the
    /// caller disappears. A shared deadline bounds all camera OCR passes.
    pub async fn recognize(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
    ) -> Result<CameraIdentification> {
        let permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ScannerError::new("busy", "Another camera lookup is running."))?;
        let deadline = Instant::now() + Duration::from_secs(60);
        let (prepared, permit) = tokio::task::spawn_blocking(move || (prepare(bytes), permit))
            .await
            .map_err(worker)?;
        let _permit = permit;
        let Some((bytes, outlined)) = prepared? else {
            return Ok(CameraIdentification::empty(CameraState::NoCard));
        };
        tokio::time::timeout_at(
            deadline.into(),
            self.identify_prepared(bytes, language, outlined, deadline),
        )
        .await
        .map_err(|_| {
            ScannerError::new(
                "timeout",
                "Camera identification timed out. Try a clearer view.",
            )
        })?
    }
    async fn identify_prepared(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
        outlined: bool,
        deadline: Instant,
    ) -> Result<CameraIdentification> {
        if let Some(evidence) = self
            .pokemon
            .camera_identity(bytes.clone(), language, deadline)
            .await?
        {
            // Return identity immediately; finish is optional and must not delay this path.
            return Ok(CameraIdentification::from_evidence(
                evidence,
                CardGame::Pokemon,
            ));
        }
        let (library, data, source) = (self.library.clone(), self.data.clone(), bytes.clone());
        let orientation = tokio::task::spawn_blocking(move || {
            let pixels = scanner_images::decode::decode(&source)?.to_rgb8();
            let mut ocr = Tesseract::new(&library, &data, language.as_str())?;
            ocr.set_deadline(deadline);
            ocr.orientation(&pixels)
        })
        .await
        .map_err(worker)??;
        if let Some(state) = side(&orientation) {
            return Ok(CameraIdentification::empty(state));
        }
        self.resolve(bytes, language, orientation, outlined, deadline)
            .await
    }
    async fn resolve(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
        orientation: Orientation,
        outlined: bool,
        deadline: Instant,
    ) -> Result<CameraIdentification> {
        let options = Options {
            preferred: orientation
                .verified
                .then_some(u32::from(orientation.quarter_turns)),
            deadline: Some(deadline),
            allow_visual: outlined
                && orientation.verified
                && orientation.basis.starts_with("pokemon-"),
        };
        if language == CardLanguage::English && orientation.basis.starts_with("starwars-") {
            return self
                .other_result(CardGame::StarwarsUnlimited, bytes, language, options)
                .await;
        }
        let evidence = self
            .pokemon
            .recognize_with_options(bytes.clone(), language, options)
            .await?;
        if meaningful(&evidence) {
            let mut result =
                CameraIdentification::from_evidence(evidence.clone(), CardGame::Pokemon);
            // An unrectified guide can confirm printed identity, but cannot
            // support crop-dependent finish analysis.
            if !outlined {
                return Ok(result);
            }
            let preparation = CardPreparation {
                orientation_verified: orientation.verified,
                orientation_basis: Some(orientation.basis.into()),
                method: "camera-perspective-rust-v1".into(),
                crop: Default::default(),
                quarter_turns: 0,
                warnings: vec![],
                source_crop: None,
                deskew_degrees: None,
                extensions: Default::default(),
            };
            let mut estimate = self
                .finish
                .estimate(bytes, evidence, Some(preparation), language, true)
                .await?;
            estimate.limitations.push("Camera estimate: lighting, glare and exposure can alter printed markers. Scanner foil-texture thresholds are not used. Check the physical card under light.".into());
            result.finish_estimate = Some(estimate);
            return Ok(result);
        }
        if language == CardLanguage::English {
            let result = self
                .other_result(CardGame::Yugioh, bytes.clone(), language, options)
                .await?;
            if !result.candidates.is_empty() {
                return Ok(result);
            }
            let result = self
                .other_result(CardGame::StarwarsUnlimited, bytes, language, options)
                .await?;
            if !result.candidates.is_empty() || result.uncatalogued_card.is_some() {
                return Ok(result);
            }
        }
        Ok(CameraIdentification::empty(CameraState::NoMatch))
    }
    async fn other_result(
        &self,
        game: CardGame,
        bytes: Vec<u8>,
        language: CardLanguage,
        options: Options,
    ) -> Result<CameraIdentification> {
        self.other
            .recognize_with_options(game, bytes, language, options)
            .await
            .map(|e| CameraIdentification::from_evidence(e, game))
    }
}
fn prepare(bytes: Vec<u8>) -> Result<Option<(Vec<u8>, bool)>> {
    let extracted = scanner_images::camera::extract(&bytes)?;
    let outlined = extracted.is_some();
    let card = match extracted {
        Some(card) => card,
        None => match scanner_images::camera::guide_for_ocr(&bytes)? {
            Some(guide) => guide,
            None => return Ok(None),
        },
    };
    Ok(Some((card.bytes, outlined)))
}

fn side(orientation: &Orientation) -> Option<CameraState> {
    if !orientation.verified {
        return None;
    }
    if orientation.basis.ends_with("-back-pattern") {
        Some(CameraState::CardBack)
    } else if matches!(orientation.basis, "pokemon-code-text" | "pokemon-code-url") {
        Some(CameraState::CodeCard)
    } else {
        None
    }
}
fn meaningful(evidence: &RecognitionEvidence) -> bool {
    !evidence.candidates.is_empty()
        || !evidence.visual_matches().is_empty()
        || ["uncataloguedCard", "energySymbol"]
            .iter()
            .any(|k| evidence.extensions.get(*k).is_some_and(|v| !v.is_null()))
}
fn worker(e: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognition", e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn empty_scene_never_loads_native_ocr_or_contacts_a_catalogue() {
        let recognizer =
            CameraRecognizer::new("missing-library".into(), "missing-data".into()).unwrap();
        let mut jpeg = vec![];
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&image::RgbImage::new(300, 400))
            .unwrap();
        let result = recognizer
            .recognize(jpeg, CardLanguage::English)
            .await
            .unwrap();
        assert_eq!(result.state, CameraState::NoCard);
        assert!(result.candidates.is_empty());
    }
    #[test]
    fn side_guidance_requires_verified_evidence_and_keeps_swu_leaders_as_fronts() {
        for (basis, expected) in [
            ("pokemon-back-pattern", Some(CameraState::CardBack)),
            ("yugioh-back-pattern", Some(CameraState::CardBack)),
            ("pokemon-code-url", Some(CameraState::CodeCard)),
            ("starwars-leader-unit-text", None),
        ] {
            let mut orientation = Orientation {
                basis,
                verified: true,
                quarter_turns: 0,
            };
            assert_eq!(side(&orientation), expected);
            orientation.verified = false;
            assert_eq!(side(&orientation), None);
        }
    }
}
