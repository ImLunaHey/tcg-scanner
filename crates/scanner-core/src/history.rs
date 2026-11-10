use crate::Engine;
use scanner_store::EventPage;
use scanner_types::{
    error::{Result, ScannerError},
    service::{JobPage, JobSummary},
};
use std::time::Duration;

impl Engine {
    pub async fn jobs(&self, limit: usize, offset: usize) -> Result<JobPage> {
        if !(1..=100).contains(&limit) || offset > 1_000_000 {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid history pagination.",
            ));
        }
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let records = database.jobs(limit + 1, offset)?;
            Ok(JobPage {
                schema_version: 1,
                next_offset: (records.len() > limit).then_some(offset + limit),
                jobs: records.iter().take(limit).map(JobSummary::from).collect(),
            })
        })
        .await
        .map_err(storage)?
    }
    pub async fn events(&self, after: i64, limit: usize, wait_seconds: u32) -> Result<EventPage> {
        if wait_seconds > 20 {
            return Err(ScannerError::new(
                "invalidParams",
                "Event wait must be 0–20 seconds.",
            ));
        }
        let _permit = self.event_waiters.try_acquire().map_err(|_| {
            ScannerError::new(
                "resourceLimit",
                "At most eight event subscriptions may wait concurrently.",
            )
        })?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_seconds.into());
        loop {
            let database = self.database.clone();
            let page = tokio::task::spawn_blocking(move || database.events(after, limit))
                .await
                .map_err(storage)??;
            if !page.events.is_empty()
                || page.reset_required
                || self.state.lock().await.stopping
                || tokio::time::Instant::now() >= deadline
            {
                return Ok(page);
            }
            tokio::time::sleep_until(
                deadline.min(tokio::time::Instant::now() + Duration::from_millis(200)),
            )
            .await;
        }
    }
}
fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}
