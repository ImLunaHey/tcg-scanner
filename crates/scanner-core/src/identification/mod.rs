mod persistence;
mod pipeline;
use crate::{
    Engine,
    image_access::{selected, worker},
};
use scanner_types::{
    error::{Result, ScannerError},
    identification::{CardIdentification, IdentificationRequest, IdentificationState},
    images::OriginalImage,
    scan::ScanRecord,
};
use std::sync::Arc;
use tokio::sync::oneshot;
impl Engine {
    pub async fn identify_card(
        self: &Arc<Self>,
        request: IdentificationRequest,
    ) -> Result<CardIdentification> {
        request.validate()?;
        if let Some(current) = self
            .card_identification(request.job_id, request.sequence)
            .await?
            && current.request.id == request.id
        {
            if current.request != request {
                return Err(conflict());
            }
            if current.state == IdentificationState::Processing {
                return Ok(current);
            }
            return self.retry_identification(request).await;
        }
        if self.recognizer.is_none() {
            return Err(ScannerError::new(
                "configuration",
                "Native recognition is not configured.",
            ));
        }
        let permit = self
            .recognition_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ScannerError::new("busy", "Another identification is running."))?;
        let engine = self.clone();
        let (reply, response) = oneshot::channel();
        tokio::spawn(async move {
            let _permit = permit;
            let _reservation = engine.submissions.lock().await;
            if let Err(error) = engine.require_saved_job(request.job_id).await {
                let _ = reply.send(Err(error));
                return;
            }
            let started = engine.clone();
            let prepared =
                tokio::task::spawn_blocking(move || started.begin_identification(request))
                    .await
                    .map_err(worker)
                    .and_then(|v| v);
            match prepared {
                Err(error) => {
                    let _ = reply.send(Err(error));
                }
                Ok((pending, None)) => {
                    let _ = reply.send(Ok(pending));
                }
                Ok((pending, Some((record, image)))) => {
                    let _ = reply.send(Ok(pending.clone()));
                    let result = engine.recognize(pending, &record, &image).await;
                    let publisher = engine.clone();
                    let saved = tokio::task::spawn_blocking(move || {
                        publisher.publish_identification(&result)
                    })
                    .await
                    .map_err(worker)
                    .and_then(|v| v);
                    if let Err(error) = saved {
                        engine.state.lock().await.persistence_error = Some(error);
                    }
                }
            }
        });
        response.await.map_err(worker)?
    }
    fn begin_identification(
        &self,
        request: IdentificationRequest,
    ) -> Result<(CardIdentification, Option<(ScanRecord, OriginalImage)>)> {
        let mut record = self.scan_status(request.job_id)?;
        if request.language != record.request.card_language.unwrap_or_default() {
            return Err(ScannerError::new(
                "invalidParams",
                "Identification language must match the batch's physical card language.",
            ));
        }
        if !record.state.terminal() {
            return Err(ScannerError::new("busy", "Wait for scanning to finish."));
        }
        selected(&record, request.sequence, None)?;
        let previous = record.identifications()?;
        if let Some(existing) = previous.iter().find(|i| i.request.id == request.id) {
            if existing.request != request {
                return Err(conflict());
            }
            self.root.save_manifest(&record)?;
            return Ok((existing.clone(), None));
        }
        let previous = previous
            .iter()
            .find(|i| i.request.sequence == request.sequence);
        if previous.is_some_and(|i| {
            !matches!(
                i.state,
                IdentificationState::Failed
                    | IdentificationState::NoMatch
                    | IdentificationState::Rejected
                    | IdentificationState::AwaitingCatalogue
            )
        }) {
            return Err(ScannerError::new(
                "conflict",
                "Review or reject the existing identification before trying again.",
            ));
        }
        let prepared = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .rev()
            .find(|e| e.request.sequence == request.sequence && e.card_preparation.is_some());
        let image = prepared
            .map(|e| &e.image)
            .unwrap_or(selected(&record, request.sequence, None)?)
            .clone();
        if image.bytes > 32 * 1024 * 1024 {
            return Err(ScannerError::new(
                "resourceLimit",
                "OCR source exceeds 32 MiB.",
            ));
        }
        self.root.verify_image(&record, &image)?;
        let mut pending = CardIdentification::pending(
            request,
            image.sha256.clone(),
            prepared.map(|e| e.request.id),
        );
        if let Some(previous) = previous.filter(|p| {
            p.source_sha256 == pending.source_sha256 && p.source_edit_id == pending.source_edit_id
        }) && let Some(estimate) = previous.extensions.get("finishEstimate")
        {
            pending
                .extensions
                .insert("finishEstimate".into(), estimate.clone());
        }
        self.persist_pending(&mut record, &pending)?;
        Ok((pending, Some((record, image))))
    }
    pub(crate) async fn recognize(
        &self,
        mut pending: CardIdentification,
        record: &ScanRecord,
        image: &OriginalImage,
    ) -> CardIdentification {
        let result = match &self.recognizer {
            Some(recognizer) => {
                pending.method = recognizer.method(record, image).into();
                recognizer.recognize(record, image).await
            }
            None => Err(ScannerError::new(
                "configuration",
                "Recognition is not configured.",
            )),
        };
        match result {
            Ok(evidence) => {
                pending.state = if evidence.candidates.is_empty() {
                    IdentificationState::NoMatch
                } else {
                    IdentificationState::ReviewRequired
                };
                pending.evidence = Some(evidence);
            }
            Err(error) => {
                pending.state = IdentificationState::Failed;
                pending.error = Some(error);
            }
        }
        pending.completed_at = Some(chrono::Utc::now());
        self.enrich_condition(record, &mut pending).await;
        self.enrich_finish(record, &mut pending).await;
        pending
    }
}
fn conflict() -> ScannerError {
    ScannerError::new(
        "conflict",
        "Identification ID already has different inputs.",
    )
}
