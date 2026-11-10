//! Reads return cached data immediately; at most two public fetches run in the background.
use scanner_pricing::{provider::PricingProvider, valuation::Snapshot};
use scanner_store::Database;
use scanner_types::{
    error::{Result, ScannerError},
    pricing::{CardMarketPrices, EuroSterlingRate},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore, watch};
const TTL: chrono::Duration = chrono::Duration::hours(6);
pub(crate) struct Cache {
    database: Arc<Database>,
    provider: Arc<dyn PricingProvider>,
    state: Mutex<State>,
    readers: Arc<Semaphore>,
    running: watch::Sender<usize>,
}
#[derive(Default)]
struct State {
    queued: VecDeque<String>,
    active: BTreeSet<String>,
    retries: BTreeMap<String, Instant>,
    workers: usize,
    closed: bool,
}
impl Cache {
    pub fn new(database: Arc<Database>, provider: Arc<dyn PricingProvider>) -> Arc<Self> {
        Arc::new(Self {
            database,
            provider,
            state: Mutex::new(State::default()),
            readers: Arc::new(Semaphore::new(4)),
            running: watch::channel(0).0,
        })
    }
    pub async fn snapshot(self: &Arc<Self>, ids: BTreeSet<String>) -> Result<Snapshot> {
        if ids.len() > 500 || ids.iter().any(|id| id.is_empty() || id.len() > 100) {
            return Err(error("Too many pricing identities."));
        }
        let permit = self
            .readers
            .clone()
            .try_acquire_owned()
            .map_err(|_| error("Pricing snapshots are busy."))?;
        // Serialize snapshot reads with completion bookkeeping so a stale read
        // cannot requeue a fetch that completed during the database operation.
        let mut state = self.state.lock().await;
        let database = self.database.clone();
        let requested = ids.clone();
        let mut snapshot = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut snapshot = Snapshot {
                rate: database
                    .cached_value("eur-gbp")?
                    .and_then(|v| serde_json::from_value::<EuroSterlingRate>(v).ok()),
                ..Default::default()
            };
            for id in requested {
                if let Some(value) = database
                    .cached_value(&format!("card-variants-v2:{id}"))?
                    .and_then(|v| serde_json::from_value::<CardMarketPrices>(v).ok())
                    .filter(|v| v.card_id == id && v.variants.len() <= 100)
                {
                    snapshot.prices.insert(id, value);
                }
            }
            Ok::<_, ScannerError>(snapshot)
        })
        .await
        .map_err(error)??;
        let now = chrono::Utc::now();
        let fresh = |date| now - date <= TTL && date <= now + chrono::Duration::days(1);
        state.retries.retain(|_, until| *until > Instant::now());
        if !ids.is_empty() && snapshot.rate.as_ref().is_none_or(|r| !fresh(r.fetched_at)) {
            state.enqueue("eur-gbp".into());
        }
        for id in &ids {
            if snapshot.prices.get(id).is_none_or(|p| !fresh(p.fetched_at)) {
                state.enqueue(format!("card:{id}"));
            }
        }
        let pending: BTreeSet<_> = state
            .active
            .iter()
            .chain(state.queued.iter())
            .cloned()
            .collect();
        snapshot.pending = ids
            .into_iter()
            .filter(|id| pending.contains(&format!("card:{id}")))
            .collect();
        snapshot.rate_pending = pending.contains("eur-gbp");
        while state.workers < 2 && !state.queued.is_empty() && !state.closed {
            state.workers += 1;
            self.running.send_replace(state.workers);
            let cache = self.clone();
            tokio::spawn(async move {
                cache.worker().await;
            });
        }
        Ok(snapshot)
    }
    async fn worker(self: Arc<Self>) {
        loop {
            let key = {
                let mut state = self.state.lock().await;
                let next = if state.closed {
                    None
                } else {
                    state.queued.pop_front()
                };
                if let Some(key) = next {
                    state.active.insert(key.clone());
                    key
                } else {
                    state.workers -= 1;
                    self.running.send_replace(state.workers);
                    return;
                }
            };
            let result = self.fetch(&key).await;
            let mut state = self.state.lock().await;
            state.active.remove(&key);
            if result.is_err() {
                state
                    .retries
                    .insert(key, Instant::now() + Duration::from_secs(60));
            }
        }
    }
    async fn fetch(&self, key: &str) -> Result<()> {
        let (key, value) = if key == "eur-gbp" {
            (
                key.to_owned(),
                serde_json::to_value(self.provider.sterling_rate().await?).map_err(error)?,
            )
        } else {
            let id = key
                .strip_prefix("card:")
                .ok_or_else(|| error("Invalid pricing key."))?;
            let prices = self.provider.prices(id).await?;
            if prices.card_id != id || prices.variants.len() > 100 {
                return Err(error("Mismatched or oversized market quote."));
            }
            (
                format!("card-variants-v2:{id}"),
                serde_json::to_value(prices).map_err(error)?,
            )
        };
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || database.cache_value(&key, &value))
            .await
            .map_err(error)?
    }
    pub async fn shutdown(&self) {
        let mut running = self.running.subscribe();
        {
            let mut state = self.state.lock().await;
            state.closed = true;
            state.queued.clear();
        }
        let _ = running.wait_for(|count| *count == 0).await;
    }
}
impl State {
    fn enqueue(&mut self, key: String) {
        if !self.closed
            && !self.active.contains(&key)
            && !self.queued.contains(&key)
            && !self.retries.contains_key(&key)
            && self.queued.len() < 200
            && self.retries.len() < 1000
        {
            self.queued.push_back(key);
        }
    }
}
fn error(value: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("pricingUnavailable", value.to_string())
}
