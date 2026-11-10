use scanner_catalogue::reference::Artwork;
use scanner_types::edition::{ReferenceEvidence, ReferenceState, StampState};
pub(super) async fn compare(
    artwork: &Artwork,
    id: &str,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> ReferenceEvidence {
    let reference = match artwork.for_card(id).await {
        Ok(value) => value,
        Err(_) => return unavailable(),
    };
    let Some(reference) = reference.as_ref() else {
        return ReferenceEvidence {
            state: ReferenceState::Missing,
            url: None,
            sha256: None,
            stamp_state: None,
            explanation: "The catalogue has no image reference for this base card.".into(),
        };
    };
    let reference = reference.clone();
    tokio::task::spawn_blocking(move || {
        let _permit=permit;
        match super::stamp::pumpkin(&reference.bytes,0) {
            Ok(stamp)=>ReferenceEvidence {state:ReferenceState::Compared,url:Some(reference.url),sha256:Some(reference.sha256),stamp_state:Some(stamp.state),
                explanation:if stamp.state==StampState::Absent {
                    "The pumpkin mark detected in the scan is absent from the public catalogue image. This compares the mark, not whole-card identity, physical finish or release year."
                } else {"The catalogue image does not provide a clear unstamped control. Its mark check cannot establish finish or release year."}.into()},
            Err(_)=>unavailable(),
        }
    }).await.unwrap_or_else(|_|unavailable())
}
fn unavailable() -> ReferenceEvidence {
    ReferenceEvidence {state:ReferenceState::Unavailable,url:None,sha256:None,stamp_state:None,
        explanation:"Catalogue image comparison was unavailable; no finish was inferred from a missing reference.".into()}
}
