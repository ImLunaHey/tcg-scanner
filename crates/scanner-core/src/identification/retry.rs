use crate::{Engine, image_access::worker};
use chrono::{DateTime, Utc};
use scanner_types::{
    error::Result,
    identification::{CardIdentification, IdentificationRequest, IdentificationState},
    images::OriginalImage,
    scan::ScanRecord,
};
use std::sync::Arc;
use tokio::sync::oneshot;
impl Engine {
    /// Admit one persisted due retry while idle. Accepted work retains the same
    /// publication/recognition ownership as explicit identification.
    pub async fn retry_awaiting_catalogue(
        self: &Arc<Self>,
        now: DateTime<Utc>,
    ) -> Result<Option<CardIdentification>> {
        if self.recognizer.is_none() {
            return Ok(None);
        }
        let Ok(permit) = self.recognition_workers.clone().try_acquire_owned() else {
            return Ok(None);
        };
        let engine = self.clone();
        let (reply, response) = oneshot::channel();
        tokio::spawn(async move {
            let _permit = permit;
            let Ok(_reservation) = engine.submissions.try_lock() else {
                let _ = reply.send(Ok(None));
                return;
            };
            match engine.begin_catalogue_retry(now).await {
                Ok(Some((pending, Some((record, image))))) => {
                    let _ = reply.send(Ok(Some(pending.clone())));
                    engine.complete_identification(pending, record, image).await;
                }
                Ok(_) => {
                    let _ = reply.send(Ok(None));
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                }
            }
        });
        response.await.map_err(worker)?
    }
    async fn begin_catalogue_retry(
        self: &Arc<Self>,
        now: DateTime<Utc>,
    ) -> Result<Option<(CardIdentification, Option<(ScanRecord, OriginalImage)>)>> {
        if !self.retry_idle().await {
            return Ok(None);
        }
        let database = self.database.clone();
        let record = tokio::task::spawn_blocking(move || database.next_catalogue_retry(now))
            .await
            .map_err(worker)??;
        let Some(record) = record else {
            return Ok(None);
        };
        let Some(due) = record
            .identifications()?
            .into_iter()
            .filter(|v| {
                matches!(
                    v.state,
                    IdentificationState::AwaitingCatalogue | IdentificationState::Failed
                ) && v.catalogue_retry_after.is_some_and(|date| date <= now)
            })
            .min_by_key(|v| v.catalogue_retry_after)
        else {
            return Ok(None);
        };
        if !self.retry_idle().await {
            return Ok(None);
        }
        let request = IdentificationRequest {
            id: uuid::Uuid::new_v4(),
            job_id: record.id,
            sequence: due.request.sequence,
            language: due.request.language,
            extensions: Default::default(),
        };
        let engine = self.clone();
        tokio::task::spawn_blocking(move || engine.begin_identification(request))
            .await
            .map_err(worker)?
            .map(Some)
    }
    async fn retry_idle(&self) -> bool {
        let state = self.state.lock().await;
        state.accepting().is_ok()
            && state.active.is_none()
            && state.queue.is_empty()
            && !state.camera
            && !state.inspecting
    }
    pub(super) async fn complete_identification(
        self: &Arc<Self>,
        pending: CardIdentification,
        record: ScanRecord,
        image: OriginalImage,
    ) {
        let result = self.recognize(pending, &record, &image).await;
        let publisher = self.clone();
        let saved = tokio::task::spawn_blocking(move || publisher.publish_identification(&result))
            .await
            .map_err(worker)
            .and_then(|v| v);
        if let Err(error) = saved {
            self.state.lock().await.persistence_error = Some(error);
        }
    }
}
