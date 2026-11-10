use crate::{
    api,
    connection_view::button,
    state::State,
    task::{self, Task},
};
use rune::{
    El, col, text,
    tok::{self, type_},
};
use scanner_types::pricing::BatchValuation;
#[derive(Default)]
pub struct Valuation {
    pub data: Option<BatchValuation>,
    pub task: Task<BatchValuation>,
    pub error: Option<String>,
}
impl Valuation {
    pub fn clear(&mut self) {
        self.data = None;
        self.task.cancel();
        self.error = None;
    }
}
impl State {
    pub(crate) fn update_valuation(&mut self) {
        self.valuation.clear();
        self.request_valuation(0);
    }
    pub fn refresh_valuation(&mut self) {
        self.request_valuation(0);
    }
    fn request_valuation(&mut self, delay: u32) {
        let (Some(client), Some(record)) = (self.client.clone(), self.record.as_ref()) else {
            return;
        };
        if self.ops.busy() {
            return;
        }
        let id = record.id;
        self.valuation.error = None;
        self.valuation.task.request(async move {
            if delay > 0 {
                task::delay(delay).await;
            }
            let value: BatchValuation =
                api::call(&client, "batches.valuation", serde_json::json!({"id":id})).await?;
            if value.batch_id != id {
                return Err(scanner_types::error::ScannerError::new(
                    "protocol",
                    "Valuation belongs to another batch.",
                ));
            }
            Ok(value)
        });
    }
    pub(crate) fn poll_valuation(&mut self) -> bool {
        let Some(result) = self.valuation.task.take() else {
            return false;
        };
        match result {
            Ok(value) if self.record.as_ref().is_some_and(|r| r.id == value.batch_id) => {
                let again = value.refreshing;
                self.valuation.data = Some(value);
                if again {
                    self.request_valuation(3000);
                }
            }
            Ok(_) => {}
            Err(error) => self.valuation.error = Some(error.to_string()),
        }
        true
    }
}
pub fn panel(s: &State, t: tok::Theme) -> El {
    let mut panel = col().gap(6.).child(
        text("Saved batch market guide")
            .font(type_::LABEL)
            .color(t.text),
    );
    if let Some(value) = &s.valuation.data {
        let total = range(value.low_pence, value.high_pence);
        panel = panel.child(note(
            format!(
                "{total} · {} of {} cards priced{}",
                value.priced_count,
                value.total_count,
                if value.stale { " · stale" } else { "" }
            ),
            t,
        ));
        let paired = s.record.as_ref().is_some_and(|r| {
            r.request.duplex
                && (r.request.prepare_cards == Some(true)
                    || r.processed_images
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .any(|e| e.card_preparation.is_some()))
        });
        let number = s.cards.get(s.selected).map(|c| {
            if paired {
                c.sequence.div_ceil(2)
            } else {
                c.sequence
            }
        });
        if let Some(card) = value.cards.iter().find(|c| Some(c.number) == number) {
            panel = panel.child(note(
                format!(
                    "This card: {}. {}",
                    range(card.low_pence, card.high_pence),
                    card.reason
                ),
                t,
            ));
        }
        panel = panel.child(note(value.warning.clone(), t));
        if value.refreshing {
            panel = panel.child(note("Refreshing public prices…".into(), t));
        }
    } else if let Some(error) = &s.valuation.error {
        panel = panel.child(note(error.clone(), t));
    } else {
        panel = panel.child(note("Market guide unavailable while loading.".into(), t));
    }
    panel.child(button(
        "Refresh prices",
        "refresh-prices",
        s.client.is_some() && !s.ops.busy() && !s.valuation.task.pending(),
        t,
        State::refresh_valuation,
    ))
}
fn note(value: String, t: tok::Theme) -> El {
    text(value)
        .font(type_::LABEL)
        .color(t.text_secondary)
        .wrap()
}
fn range(low: Option<i64>, high: Option<i64>) -> String {
    let Some((low, high)) = low.zip(high) else {
        return "unavailable".into();
    };
    let pounds = |p: i64| format!("£{}.{:02}", p / 100, p % 100);
    if low == high {
        pounds(low)
    } else {
        format!("{}–{}", pounds(low), pounds(high))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_quotes_never_render_as_zero_and_integer_pence_keep_their_precision() {
        assert_eq!(range(None, None), "unavailable");
        assert_eq!(range(Some(113), None), "unavailable");
        assert_eq!(range(Some(113), Some(113)), "£1.13");
        assert_eq!(range(Some(1), Some(225)), "£0.01–£2.25");
    }
}
