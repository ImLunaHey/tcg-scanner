use super::background::Background;
use scanner_types::{
    finish::{
        BorderFinishEvidence, BorderState, FinishEstimate, FinishState, FinishUncertainty as Reason,
    },
    recognition::{CardCandidate, CardFinish},
};

pub(super) fn contrast(
    card: &CardCandidate,
    median: f64,
    dark: f64,
    light: f64,
    background: &Background,
) -> FinishEstimate {
    let mut result = FinishEstimate::new(
        FinishState::Uncertain,
        Some(card.id.clone()),
        vec!["Set-code contrast is inconclusive.".into()],
        Some(Reason::MarkerContrast),
    );
    if dark <= 0.08 || light <= 0.08 {
        return result;
    }
    let overall = if median > 0.65 && light > 0.40 {
        Some("light")
    } else if median < 0.45 && dark > 0.30 {
        Some("dark")
    } else {
        None
    };
    if let (Some(overall), Some(spatial)) = (overall, background.polarity())
        && overall != spatial
    {
        result.evidence = vec!["Set-code background and overall contrast disagree.".into()];
        result.uncertainty_reason = Some(Reason::ConflictingContrast);
        return result;
    }
    match background.polarity().or(overall) {
        Some("light") => {
            result.state = FinishState::Estimated;
            result.suggested_finish = Some(CardFinish::ReverseHolo);
            result.uncertainty_reason = None;
            result.marker_polarity = Some("light".into());
            result.evidence = vec![
                "Light set-code box with dark lettering suggests a reverse-holo printing.".into(),
            ];
        }
        Some("dark") => regular(card, &mut result),
        _ => (),
    }
    if background.polarity().is_some() {
        result.evidence.push(
            "Broad clear background patches were measured separately from the set-code lettering."
                .into(),
        );
    }
    result
}
fn regular(card: &CardCandidate, result: &mut FinishEstimate) {
    result.marker_polarity = Some("dark".into());
    let normal = card.available_finishes.contains(&CardFinish::Normal);
    let holo = card.available_finishes.contains(&CardFinish::Holo);
    if normal != holo {
        result.state = FinishState::Estimated;
        result.uncertainty_reason = None;
        result.suggested_finish = Some(if normal {
            CardFinish::Normal
        } else {
            CardFinish::Holo
        });
        result.evidence = vec![
            "Dark set-code box suggests the regular printing.".into(),
            "The catalogue lists one regular finish for this printing.".into(),
        ];
    } else if !normal {
        result.evidence=vec!["Dark set-code box suggests the regular printing, but the catalogue does not list its finish.".into()];
        result.uncertainty_reason = Some(Reason::MissingRegularFinish);
    } else {
        result.evidence=vec!["Dark set-code box suggests the regular printing, but it does not distinguish holo from non-holo.".into()];
        result.uncertainty_reason = Some(Reason::MultipleRegularFinishes);
    }
}
pub(super) fn combine(
    mut result: FinishEstimate,
    card: &CardCandidate,
    border: BorderFinishEvidence,
) -> FinishEstimate {
    if result.candidate_id.as_ref() != Some(&card.id)
        || result.marker_polarity.as_deref() != Some("dark")
        || !(result.state == FinishState::Estimated
            || result.uncertainty_reason == Some(Reason::MultipleRegularFinishes))
    {
        return result;
    }
    result.limitations.push("Border texture was calibrated on a small set of modern cards. Printed patterns can resemble foil; absent texture does not establish non-holo or rule out reverse holo.".into());
    let message = match border.state {
        BorderState::Present => {
            if card.available_finishes.contains(&CardFinish::Holo) {
                result.state = FinishState::Estimated;
                result.suggested_finish = Some(CardFinish::Holo);
                result.uncertainty_reason = None;
                "Independent brightness texture on both borders exceeds the fixed foil threshold in all three measurement variants; the dark set code and catalogue support holo."
            } else {
                result.state = FinishState::Uncertain;
                result.suggested_finish = None;
                result.uncertainty_reason = Some(Reason::ConflictingFinishEvidence);
                "Border texture suggests foil but the catalogue does not list a regular holo printing. The conflict needs review."
            }
        }
        BorderState::Absent => {
            "No stable border foil signal was found; this does not prove non-holo. The printed-marker/catalogue inference is retained."
        }
        BorderState::Unstable => {
            "Border measurements disagree across strip widths or resolution. They cannot resolve the finish; the printed-marker/catalogue inference is retained."
        }
        BorderState::Unsupported => {
            "The prepared image is outside the border check's size or shape limits; only printed-marker/catalogue evidence was used."
        }
        BorderState::Failed => {
            "Border analysis was unavailable; only printed-marker/catalogue evidence was used."
        }
    };
    result.evidence.push(message.into());
    result.border_evidence = Some(border);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(finishes: Vec<CardFinish>) -> CardCandidate {
        serde_json::from_value(serde_json::json!({"id":"sv01-001","name":"Example","setID":"sv01","setName":"Example","localID":"001","evidence":[],"availableFinishes":finishes})).unwrap()
    }
    #[test]
    fn ambiguous_regular_finishes_and_conflicting_evidence_do_not_become_confirmed_holo() {
        let card = card(vec![
            CardFinish::Normal,
            CardFinish::Holo,
            CardFinish::ReverseHolo,
        ]);
        let marker = contrast(
            &card,
            0.2,
            0.7,
            0.2,
            &Background {
                dark: 0.2,
                light: 0.,
            },
        );
        assert_eq!(
            marker.uncertainty_reason,
            Some(Reason::MultipleRegularFinishes)
        );
        let absent = combine(
            marker.clone(),
            &card,
            super::super::border::unavailable(BorderState::Absent),
        );
        assert_eq!(absent.suggested_finish, None);
        let positive = combine(
            marker,
            &card,
            super::super::border::unavailable(BorderState::Present),
        );
        assert_eq!(positive.suggested_finish, Some(CardFinish::Holo));
        let conflict = contrast(
            &card,
            0.8,
            0.2,
            0.7,
            &Background {
                dark: 0.2,
                light: 0.,
            },
        );
        assert_eq!(
            conflict.uncertainty_reason,
            Some(Reason::ConflictingContrast)
        );
        assert_eq!(conflict.suggested_finish, None);
    }
    #[test]
    fn reverse_marker_stays_independent_and_border_cannot_invent_catalogue_finish() {
        let card = card(vec![CardFinish::Normal, CardFinish::ReverseHolo]);
        let reverse = contrast(
            &card,
            0.8,
            0.2,
            0.7,
            &Background {
                dark: 0.,
                light: 0.2,
            },
        );
        let unchanged = combine(
            reverse,
            &card,
            super::super::border::unavailable(BorderState::Present),
        );
        assert_eq!(unchanged.suggested_finish, Some(CardFinish::ReverseHolo));
        assert!(unchanged.border_evidence.is_none());
        let marker = contrast(
            &card,
            0.2,
            0.7,
            0.2,
            &Background {
                dark: 0.2,
                light: 0.,
            },
        );
        let conflict = combine(
            marker,
            &card,
            super::super::border::unavailable(BorderState::Present),
        );
        assert_eq!(
            conflict.uncertainty_reason,
            Some(Reason::ConflictingFinishEvidence)
        );
        assert_eq!(conflict.suggested_finish, None);
    }
}
