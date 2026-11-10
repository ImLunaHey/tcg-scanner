//! Preserve all reviewer-owned changes, including partially updated cards.
use crate::contracts::{CardInput, CardReply, Photo};

/// Expected immutable image identity, independent of filesystem representation.
#[derive(Clone, Copy)]
pub struct ImageIdentity<'a> {
    pub sha256: &'a str,
    pub bytes: u64,
}
#[derive(Clone, Copy)]
pub struct Pair<'a> {
    pub front: ImageIdentity<'a>,
    pub back: ImageIdentity<'a>,
}
pub fn photos_match(remote: &CardReply, pair: Pair<'_>) -> bool {
    fn matches(photo: Option<&Photo>, image: ImageIdentity<'_>) -> bool {
        photo.is_some_and(|p| p.sha256 == image.sha256 && p.size_bytes == image.bytes)
    }
    matches(remote.front.as_ref(), pair.front) && matches(remote.back.as_ref(), pair.back)
}
pub fn metadata_matches(remote: &CardReply, input: &CardInput) -> bool {
    remote.review.as_ref() == Some(&input.review)
        && remote.game == input.game
        && remote.language == input.language
        && remote.game_source == input.game_source
        && remote.language_source == input.language_source
}
/// A baseline is the last acknowledged scanner-owned value, never a remote copy.
pub fn can_sync(
    remote: &CardReply,
    input: &CardInput,
    pair: Pair<'_>,
    baseline: Option<(&CardInput, Pair<'_>)>,
) -> bool {
    if remote.status != "pending"
        || remote.confirmed.is_some()
        || remote.notes.as_deref() != Some("")
        || remote.game_source.as_deref() == Some("manual")
        || remote.language_source.as_deref() == Some("manual")
    {
        return false;
    }
    let metadata = metadata_matches(remote, input)
        || baseline.is_some_and(|(previous, _)| scanner_owned(remote, input, previous));
    let photos = (remote.front.is_none() && remote.back.is_none())
        || photos_match(remote, pair)
        || baseline.is_some_and(|(_, previous)| photos_match(remote, previous));
    metadata && photos
}
fn scanner_owned(remote: &CardReply, target: &CardInput, previous: &CardInput) -> bool {
    let Some(review) = &remote.review else {
        return false;
    };
    fn owned<T: PartialEq>(current: &T, new: &T, old: &T) -> bool {
        current == new || current == old
    }
    owned(&remote.game, &target.game, &previous.game)
        && owned(&remote.language, &target.language, &previous.language)
        && owned(
            &remote.game_source,
            &target.game_source,
            &previous.game_source,
        )
        && owned(
            &remote.language_source,
            &target.language_source,
            &previous.language_source,
        )
        && owned(
            &review.recognition_suggestions,
            &target.review.recognition_suggestions,
            &previous.review.recognition_suggestions,
        )
        && owned(
            &review.finish_estimate,
            &target.review.finish_estimate,
            &previous.review.finish_estimate,
        )
        && owned(
            &review.condition_estimate,
            &target.review.condition_estimate,
            &previous.review.condition_estimate,
        )
        && owned(
            &review.border_findings,
            &target.review.border_findings,
            &previous.review.border_findings,
        )
        && owned(
            &review.suggested_price_gbp,
            &target.review.suggested_price_gbp,
            &previous.review.suggested_price_gbp,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::Review;
    use uuid::Uuid;

    fn fixture() -> (CardInput, CardInput, Pair<'static>, CardReply) {
        let previous = CardInput {
            card_number: "C-001".into(),
            review: Review::default(),
            game: Some("pokemon".into()),
            language: Some("en".into()),
            game_source: Some("recognition".into()),
            language_source: Some("scannerConfiguration".into()),
        };
        let mut target = previous.clone();
        target.review.finish_estimate = Some("holo".into());
        target.review.suggested_price_gbp = Some(113);
        let pair = Pair {
            front: ImageIdentity {
                sha256: "front",
                bytes: 10,
            },
            back: ImageIdentity {
                sha256: "back",
                bytes: 20,
            },
        };
        let remote = CardReply {
            id: Uuid::new_v4(),
            scan_batch_id: Uuid::new_v4(),
            card_number: "C-001".into(),
            revision: 3,
            status: "pending".into(),
            front: None,
            back: None,
            review: Some(previous.review.clone()),
            confirmed: None,
            notes: Some(String::new()),
            game: previous.game.clone(),
            language: previous.language.clone(),
            game_source: previous.game_source.clone(),
            language_source: previous.language_source.clone(),
        };
        (previous, target, pair, remote)
    }
    #[test]
    fn retries_allow_mixed_acknowledged_values_but_preserve_every_reviewer_change() {
        let (previous, target, pair, remote) = fixture();
        assert!(!can_sync(&remote, &target, pair, None));
        assert!(can_sync(&remote, &target, pair, Some((&previous, pair))));
        let mut partial = remote.clone();
        partial.review.as_mut().unwrap().finish_estimate = target.review.finish_estimate.clone();
        assert!(can_sync(&partial, &target, pair, Some((&previous, pair))));
        for field in [
            "notes",
            "confirmed",
            "status",
            "gameSource",
            "languageSource",
            "language",
            "review",
            "front",
        ] {
            let mut changed = serde_json::to_value(&partial).unwrap();
            changed[field] = match field {
                "confirmed" => serde_json::json!({"approved":true}),
                "review" => {
                    serde_json::json!({"recognitionSuggestions":[],"borderFindings":["Reviewer mark"]})
                }
                "front" => {
                    serde_json::json!({"id":Uuid::new_v4(),"sha256":"reviewer-photo","sizeBytes":10})
                }
                "gameSource" | "languageSource" => serde_json::json!("manual"),
                _ => serde_json::json!("reviewer"),
            };
            let changed: CardReply = serde_json::from_value(changed).unwrap();
            assert!(
                !can_sync(&changed, &target, pair, Some((&previous, pair))),
                "{field}"
            );
        }
        let mut same = remote;
        same.review = Some(target.review.clone());
        same.front = Some(Photo {
            id: Uuid::new_v4(),
            sha256: "front".into(),
            size_bytes: 10,
        });
        same.back = Some(Photo {
            id: Uuid::new_v4(),
            sha256: "back".into(),
            size_bytes: 20,
        });
        assert!(can_sync(&same, &target, pair, None));
        same.back.as_mut().unwrap().size_bytes += 1;
        assert!(!can_sync(&same, &target, pair, None));
    }
}
