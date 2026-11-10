use super::LibraryCatalogue;
use scanner_types::inventory::InventoryCatalogueSource as Source;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct ReleaseDates {
    pub dates: BTreeMap<Source, String>,
    pub pending: bool,
}
struct Entry {
    date: Option<String>,
    expires: Instant,
}
#[derive(Default)]
pub(super) struct State {
    entries: BTreeMap<Source, Entry>,
    queued: VecDeque<Source>,
    active: BTreeSet<Source>,
}
impl LibraryCatalogue {
    /// Never waits for remote metadata. At most three background requests, 2000
    /// queued sources and 2000 cached dates. Errors retry after five minutes.
    pub fn release_dates(self: &Arc<Self>, sources: &[Source]) -> ReleaseDates {
        let now = Instant::now();
        let snapshot = {
            let mut state = self.dates.lock().unwrap_or_else(|p| p.into_inner());
            for source in sources {
                if source.provider == "tcgdex"
                    && state.entries.get(source).is_none_or(|e| e.expires < now)
                    && !state.active.contains(source)
                    && !state.queued.contains(source)
                    && state.queued.len() < 2000
                {
                    state.queued.push_back(source.clone());
                }
            }
            ReleaseDates {
                dates: sources
                    .iter()
                    .filter_map(|s| Some((s.clone(), state.entries.get(s)?.date.clone()?)))
                    .collect(),
                pending: sources
                    .iter()
                    .any(|s| state.active.contains(s) || state.queued.contains(s)),
            }
        };
        self.pump_dates();
        snapshot
    }
    fn pump_dates(self: &Arc<Self>) {
        loop {
            let source = {
                let mut state = self.dates.lock().unwrap_or_else(|p| p.into_inner());
                if state.active.len() >= 3 {
                    return;
                }
                let Some(source) = state.queued.pop_front() else {
                    return;
                };
                state.active.insert(source.clone());
                source
            };
            let catalogue = self.clone();
            tokio::spawn(async move {
                let result = catalogue.pokemon(source.language).set(&source.set_id).await;
                {
                    let mut state = catalogue.dates.lock().unwrap_or_else(|p| p.into_inner());
                    let (date, ttl) = match result {
                        Ok(set) => (set.release_date.as_deref().and_then(normalized), 86400),
                        Err(_) => (state.entries.get(&source).and_then(|e| e.date.clone()), 300),
                    };
                    if state.entries.len() >= 2000
                        && !state.entries.contains_key(&source)
                        && let Some(oldest) = state
                            .entries
                            .iter()
                            .min_by_key(|(_, e)| e.expires)
                            .map(|(s, _)| s.clone())
                    {
                        state.entries.remove(&oldest);
                    }
                    state.entries.insert(
                        source.clone(),
                        Entry {
                            date,
                            expires: Instant::now() + Duration::from_secs(ttl),
                        },
                    );
                    state.active.remove(&source);
                }
                catalogue.pump_dates();
            });
        }
    }
}
fn normalized(value: &str) -> Option<String> {
    let date = if value.len() == 10 && value.as_bytes().get(4) == Some(&b'-') {
        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?
    } else {
        let parts: Vec<_> = value.split('/').map(|v| v.parse::<u32>().ok()).collect();
        if parts.len() != 3 {
            return None;
        }
        let year = parts[2]?;
        chrono::NaiveDate::from_ymd_opt(
            if year < 100 {
                2000 + year as i32
            } else {
                year as i32
            },
            parts[0]?,
            parts[1]?,
        )?
    };
    use chrono::Datelike;
    (1990..=2100)
        .contains(&date.year())
        .then(|| date.format("%Y-%m-%d").to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_reject_impossible_days_and_outside_years() {
        assert_eq!(normalized("2/29/24").as_deref(), Some("2024-02-29"));
        assert_eq!(normalized("2026-03-28").as_deref(), Some("2026-03-28"));
        for date in [
            "2025-02-29",
            "2/31/2024",
            "1888-01-01",
            "2101-01-01",
            "garbage",
        ] {
            assert!(normalized(date).is_none());
        }
    }
}
