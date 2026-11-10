use scanner_staging::contracts::{CardInput, Review, Suggestion};
use scanner_types::{
    cards::CardLanguage,
    condition::{ConditionCheck, ConditionState},
    error::{Result, ScannerError},
    finish::{FinishEstimate, FinishState},
    identification::{CardIdentification, IdentificationState},
    pricing::{BatchValuation, ValuationState},
    recognition::{CardCandidate, CardFinish},
};

pub(super) fn finish(identity: &CardIdentification) -> Result<Option<FinishEstimate>> {
    identity
        .extensions
        .get("finishEstimate")
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|_| {
                ScannerError::new(
                    "storeNotReady",
                    "Finish evidence is unreadable; refresh its assessment before sending.",
                )
            })
        })
        .transpose()
}
fn candidate(identity: &CardIdentification) -> Option<&CardCandidate> {
    let candidates = &identity.evidence.as_ref()?.candidates;
    match identity.state {
        IdentificationState::Confirmed => candidates
            .iter()
            .find(|c| Some(&c.id) == identity.selected_card_id.as_ref()),
        IdentificationState::ReviewRequired if candidates.len() == 1 => candidates.first(),
        _ => None,
    }
}
pub(super) fn input(
    number: u32,
    language: CardLanguage,
    front: Option<&CardIdentification>,
    back: Option<&CardIdentification>,
    valuation: Option<&BatchValuation>,
) -> Result<CardInput> {
    let mut review = match front {
        Some(front) => front_review(number, language, front, valuation)?,
        None => Review::default(),
    };
    back_findings(&mut review, back)?;
    Ok(CardInput {
        card_number: format!("C-{number:03}"),
        review,
        game: Some("pokemon".into()),
        language: Some(language.as_str().into()),
        game_source: Some("recognition".into()),
        language_source: Some("scannerConfiguration".into()),
    })
}
fn front_review(
    number: u32,
    language: CardLanguage,
    front: &CardIdentification,
    valuation: Option<&BatchValuation>,
) -> Result<Review> {
    let mut review = Review::default();
    let estimate = finish(front)?;
    let selected = candidate(front);
    if matches!(
        front.state,
        IdentificationState::Confirmed | IdentificationState::ReviewRequired
    ) {
        review.recognition_suggestions = front
            .evidence
            .iter()
            .flat_map(|e| &e.candidates)
            .take(10)
            .map(|c| Suggestion {
                provider: Some("tcgdex".into()),
                external_id: Some(c.id.clone()),
                name: c.name.clone(),
                set_name: Some(c.set_name.clone()),
                collector_number: Some(c.local_id.clone()),
                language: Some(language.as_str().into()),
                catalogue_id: None,
                confidence: None,
            })
            .collect();
    }
    let suggested = if front.finish != CardFinish::Unverified {
        Some(front.finish)
    } else if language == CardLanguage::English {
        estimate
            .as_ref()
            .filter(|e| {
                e.state == FinishState::Estimated
                    && e.candidate_id.as_deref() == selected.map(|c| c.id.as_str())
            })
            .and_then(|e| e.suggested_finish)
    } else {
        None
    };
    review.finish_estimate = suggested.map(|s| {
        serde_json::to_value(s)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    });
    let edition = estimate
        .as_ref()
        .and_then(|e| e.edition.as_ref())
        .map(|e| e.pricing_key.as_str());
    review.suggested_price_gbp = valuation
        .filter(|_| language == CardLanguage::English)
        .and_then(|v| {
            v.cards.iter().find(|c| {
                c.number == number
                    && selected.is_some()
                    && c.card_id.as_deref() == selected.map(|c| c.id.as_str())
                    && c.edition_key.as_deref() == edition
                    && c.state == ValuationState::Priced
                    && !c.stale
                    && c.low_pence == c.high_pence
            })
        })
        .and_then(|c| c.low_pence)
        .filter(|v| *v > 0);
    Ok(review)
}
fn back_findings(review: &mut Review, back: Option<&CardIdentification>) -> Result<()> {
    if let Some(value) = back.and_then(|b| b.extensions.get("conditionCheck")) {
        let check: ConditionCheck = serde_json::from_value(value.clone()).map_err(|_| {
            ScannerError::new(
                "storeNotReady",
                "Condition evidence is unreadable; refresh its assessment before sending.",
            )
        })?;
        if check.state == ConditionState::Checked {
            for finding in check.findings {
                let note: String = format!("Back: {}", finding.explanation)
                    .chars()
                    .take(500)
                    .collect();
                if !review.border_findings.contains(&note) {
                    review.border_findings.push(note);
                }
                if review.border_findings.len() == 30 {
                    break;
                }
            }
        }
    }
    Ok(())
}
