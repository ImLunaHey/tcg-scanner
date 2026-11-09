use scanner_types::error::{Result, ScannerError};
use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OnceCell};

type Entry<T> = Arc<OnceCell<(Instant, Arc<T>)>>;
pub(crate) struct Cache<T> {
    entries: Mutex<HashMap<String, Entry<T>>>,
    capacity: usize,
    ttl: Duration,
}
impl<T> Cache<T> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            capacity,
            ttl,
        }
    }
    pub async fn get<F, Fut>(&self, key: &str, load: F) -> Result<Arc<T>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let entry = {
            let mut entries = self.entries.lock().await;
            entries.retain(|_, entry| entry.get().is_none_or(|(at, _)| at.elapsed() < self.ttl));
            if let Some(entry) = entries.get(key) {
                entry.clone()
            } else {
                if entries.len() >= self.capacity {
                    let oldest = entries
                        .iter()
                        .filter_map(|(key, entry)| entry.get().map(|(at, _)| (key.clone(), *at)))
                        .min_by_key(|(_, at)| *at)
                        .map(|(key, _)| key);
                    if let Some(oldest) = oldest {
                        entries.remove(&oldest);
                    } else {
                        return Err(ScannerError::new(
                            "resourceLimit",
                            "Too many catalogue lookups in progress.",
                        ));
                    }
                }
                let entry = Arc::new(OnceCell::new());
                entries.insert(key.into(), entry.clone());
                entry
            }
        };
        let result = entry
            .get_or_try_init(|| async {
                let value = load().await?;
                Ok::<_, ScannerError>((Instant::now(), Arc::new(value)))
            })
            .await;
        match result {
            Ok((_, value)) => Ok(value.clone()),
            Err(error) => {
                let mut entries = self.entries.lock().await;
                if entries
                    .get(key)
                    .is_some_and(|current| Arc::ptr_eq(current, &entry))
                {
                    entries.remove(key);
                }
                Err(error)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn concurrent_requests_coalesce_and_errors_can_be_retried() {
        let cache = Cache::new(2, Duration::from_secs(60));
        let calls = AtomicUsize::new(0);
        let load = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok(42)
        };
        let (a, b) = tokio::join!(cache.get("x", load), cache.get("x", load));
        assert_eq!(*a.unwrap(), 42);
        assert_eq!(*b.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            cache
                .get("bad", || async { Err(ScannerError::new("test", "retry")) })
                .await
                .is_err()
        );
        assert_eq!(*cache.get("bad", || async { Ok(8) }).await.unwrap(), 8);
    }
    #[tokio::test]
    async fn expired_entries_are_loaded_again() {
        let cache = Cache::new(1, Duration::ZERO);
        assert_eq!(*cache.get("x", || async { Ok(1) }).await.unwrap(), 1);
        assert_eq!(*cache.get("x", || async { Ok(2) }).await.unwrap(), 2);
    }
}
