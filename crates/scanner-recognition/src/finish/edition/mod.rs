//! Stamped finish decisions require independent catalogue identity and mark evidence.
mod registry;
use scanner_types::{
    edition::{EditionEvidence, EditionState, PrintingVariant, StampEvidence, StampState},
    finish::{
        BorderFinishEvidence, BorderState, FinishEstimate, FinishState, FinishUncertainty as Reason,
    },
    recognition::{CardCandidate, CardFinish},
};
fn numbered(card: &CardCandidate) -> bool {
    card.id == format!("{}-{}", card.set_id, card.local_id)
        && card
            .evidence
            .contains(&format!("Collector number {}", card.local_id))
}
fn stamped(variants: &[PrintingVariant], kind: &str) -> Vec<PrintingVariant> {
    variants
        .iter()
        .filter(|v| {
            v.source == "tcgdex"
                && !v.id.is_empty()
                && !v.stamps.is_empty()
                && v.stamps.iter().all(|s| s == kind)
                && matches!(
                    v.finish,
                    CardFinish::Normal | CardFinish::Holo | CardFinish::ReverseHolo
                )
        })
        .cloned()
        .collect()
}
pub(super) fn resolve(
    stamp: StampEvidence,
    card: Option<&CardCandidate>,
    variants: &[PrintingVariant],
) -> EditionEvidence {
    if stamp.kind == "pikachu-30th-anniversary" {
        return anniversary(stamp, card, variants);
    }
    let id = card.map(|c| c.id.clone());
    let Some(card) = card.filter(|c| numbered(c) && stamp.state == StampState::Present) else {
        return EditionEvidence::new(
            EditionState::Uncertain,
            id,
            stamp,
            None,
            vec![],
            vec![
                "A clear pumpkin mark and consistent numbered catalogue identity are required."
                    .into(),
            ],
        );
    };
    let mut matches = stamped(variants, "trick-or-trade");
    matches.extend(registry::matches(card));
    let found = !matches.is_empty();
    EditionEvidence::new(if found { EditionState::Matched } else { EditionState::Unlisted },id,stamp,
        found.then(||"Trick or Trade".into()),matches,vec![if found {
            "The pumpkin mark and numbered base card match stamped catalogue printings. Listed years are candidates, not proof of the physical release year. Coverage is limited to known variants and the 2022–2024 registry."
        } else { "A pumpkin stamp is visible, but no supported stamped catalogue printing matches this base card." }.into()])
}
fn anniversary(
    stamp: StampEvidence,
    card: Option<&CardCandidate>,
    variants: &[PrintingVariant],
) -> EditionEvidence {
    let matches = stamped(variants, "30th-anniversary");
    let matched =
        stamp.state == StampState::Present && card.is_some_and(numbered) && !matches.is_empty();
    EditionEvidence::new(if matched { EditionState::Matched } else { EditionState::Uncertain },card.map(|c|c.id.clone()),stamp,
        matched.then(||"30th Anniversary".into()),if matched { matches } else { vec![] },vec![if matched {
            "The visible 30 mark and independently numbered card match an anniversary catalogue variant."
        } else { "A visible 30 mark and matching numbered anniversary catalogue variant are both required." }.into()])
}
pub(super) fn needs_border(edition: &EditionEvidence) -> bool {
    let finishes = finishes(edition);
    finishes.len() == 2
        && finishes.contains(&CardFinish::Normal)
        && finishes.contains(&CardFinish::Holo)
        && edition.stamp.kind == "pikachu-pumpkin"
}
fn finishes(edition: &EditionEvidence) -> Vec<CardFinish> {
    let mut values = vec![];
    for v in &edition.variants {
        if !values.contains(&v.finish) {
            values.push(v.finish);
        }
    }
    values
}
pub(super) fn decision(
    edition: EditionEvidence,
    border: Option<BorderFinishEvidence>,
) -> FinishEstimate {
    let anniversary = edition.stamp.kind == "pikachu-30th-anniversary";
    let mut result = FinishEstimate::new(
        FinishState::Uncertain,
        edition.candidate_id.clone(),
        edition.evidence.clone(),
        Some(Reason::EditionUnavailable),
    );
    result.method = if anniversary {
        "anniversary-stamped-printing-rust-v1"
    } else {
        "pumpkin-stamped-printing-rust-v1"
    }
    .into();
    result.border_evidence = border;
    result.limitations=vec!["Catalogue-based stamped-printing estimate, not measured reflectivity or authenticity. Metadata can be incomplete or incorrect.".into(),
        "The same mark can be reused. Exact release year and other stamp families are not established.".into()];
    if edition.stamp.state == StampState::Present && edition.state == EditionState::Matched {
        let finishes = finishes(&edition);
        if anniversary && (finishes.len() != 1 || finishes[0] == CardFinish::Normal) {
            result.uncertainty_reason = Some(Reason::ConflictingFinishEvidence);
            result.evidence.push("Anniversary edition resolved; catalogue finish metadata does not establish a reliable physical finish.".into());
        } else if finishes.len() == 1 {
            result.state = FinishState::Estimated;
            result.suggested_finish = Some(finishes[0]);
            result.uncertainty_reason = None;
        } else if needs_border(&edition)
            && result
                .border_evidence
                .as_ref()
                .is_some_and(|b| b.state == BorderState::Present)
        {
            result.state = FinishState::Estimated;
            result.suggested_finish = Some(CardFinish::Holo);
            result.uncertainty_reason = None;
            result.evidence.push(
                "Stamped entries disagree, but stable positive foil-border evidence supports holo."
                    .into(),
            );
        } else {
            result.uncertainty_reason = Some(Reason::MultipleEditionFinishes);
        }
    } else if edition.stamp.state == StampState::Uncertain {
        result.uncertainty_reason = Some(Reason::StampUncertain);
    }
    result.edition = Some(edition);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card() -> CardCandidate {
        serde_json::from_value(serde_json::json!({"id":"sv02-097","name":"Mimikyu","localID":"097",
            "setID":"sv02","setName":"Paldea Evolved","officialSetCount":193,"evidence":["Collector number 097"]})).unwrap()
    }
    fn stamp(kind: &str, state: StampState) -> StampEvidence {
        StampEvidence {
            state,
            kind: kind.into(),
            method: "test".into(),
            region: None,
            source_quarter_turns: 0,
            similarity: None,
            baseline_similarity: None,
            downsampled_similarity: None,
            evidence: vec![],
        }
    }
    fn variant(finish: CardFinish) -> PrintingVariant {
        PrintingVariant {
            id: "variant-1".into(),
            source: "tcgdex".into(),
            finish,
            stamps: vec!["30th-anniversary".into()],
            years: vec![],
        }
    }
    #[test]
    fn conflicting_stamped_finishes_need_positive_border_not_absence() {
        let card = card();
        let stamp = stamp("pikachu-pumpkin", StampState::Present);
        for border_state in [
            BorderState::Absent,
            BorderState::Unstable,
            BorderState::Present,
        ] {
            let edition = resolve(stamp.clone(), Some(&card), &[]);
            assert_eq!(edition.state, EditionState::Matched);
            assert_eq!(edition.variants.len(), 2);
            let result = decision(
                edition,
                Some(crate::finish::border::unavailable(border_state)),
            );
            if border_state == BorderState::Present {
                assert_eq!(result.suggested_finish, Some(CardFinish::Holo));
            } else {
                assert_eq!(result.state, FinishState::Uncertain);
                assert!(result.suggested_finish.is_none());
                assert_eq!(
                    result.uncertainty_reason,
                    Some(Reason::MultipleEditionFinishes)
                );
            }
        }
        let mut wrong = card.clone();
        wrong.id = "sv02-97".into();
        assert_eq!(
            resolve(stamp, Some(&wrong), &[]).state,
            EditionState::Uncertain
        );
    }
    #[test]
    fn anniversary_normal_discrepancy_and_uncertain_marks_never_confirm_a_finish() {
        let card = card();
        let stamp = stamp("pikachu-30th-anniversary", StampState::Present);
        for finish in [CardFinish::Normal, CardFinish::Holo] {
            let result = decision(
                resolve(stamp.clone(), Some(&card), &[variant(finish)]),
                None,
            );
            assert_eq!(
                result.edition.as_ref().unwrap().state,
                EditionState::Matched
            );
            if finish == CardFinish::Normal {
                assert_eq!(
                    result.uncertainty_reason,
                    Some(Reason::ConflictingFinishEvidence)
                );
            } else {
                assert_eq!(result.suggested_finish, Some(finish));
            }
        }
        let uncertain = StampEvidence {
            state: StampState::Uncertain,
            ..stamp.clone()
        };
        assert!(
            decision(
                resolve(uncertain, Some(&card), &[variant(CardFinish::Holo)]),
                None
            )
            .suggested_finish
            .is_none()
        );
        let mut mixed = variant(CardFinish::Holo);
        mixed.stamps.push("other-promotion".into());
        assert_ne!(
            resolve(stamp, Some(&card), &[mixed]).state,
            EditionState::Matched
        );
    }
}
