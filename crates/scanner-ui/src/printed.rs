//! Printed cues remain visibly separate from catalogue identity.
use crate::state::State;
use rune::{
    El, col, text,
    tok::{self, type_},
};
use scanner_types::printed::{EnergySymbol, UncataloguedCard};
use serde_json::Value;

pub fn title(evidence: &Value) -> Option<(String, String)> {
    if let Ok(card) =
        serde_json::from_value::<UncataloguedCard>(evidence["uncataloguedCard"].clone())
    {
        return Some((
            card.name,
            format!(
                "{} · {} · awaiting catalogue",
                card.set_code, card.collector_number
            ),
        ));
    }
    let energy: EnergySymbol = serde_json::from_value(evidence["energySymbol"].clone()).ok()?;
    Some((
        format!("{} Energy", energy.energy_type),
        "Printed symbol · exact printing unknown".into(),
    ))
}
pub fn panel(s: &State, t: tok::Theme) -> El {
    let mut pane = col().gap(6.);
    let Some(evidence) = s.ops.review.as_ref().and_then(|r| r.evidence.as_ref()) else {
        return pane;
    };
    if let Some(value) = evidence.extensions.get("uncataloguedCard")
        && let Ok(card) = serde_json::from_value::<UncataloguedCard>(value.clone())
    {
        pane = pane.child(line(format!("Printed text: {} · {} · {}", card.name, card.set_code, card.collector_number), t))
            .child(line("OCR transcription; catalogue identity remains unverified. Lookup retries automatically.".into(), t));
    }
    if let Some(value) = evidence.extensions.get("energySymbol")
        && let Ok(symbol) = serde_json::from_value::<EnergySymbol>(value.clone())
    {
        pane = pane.child(line(
            format!(
                "Printed Energy symbol: {}. The symbol does not establish set, price or finish.",
                symbol.energy_type
            ),
            t,
        ));
    }
    pane
}
fn line(value: String, t: tok::Theme) -> El {
    text(value).font(type_::BODY).color(t.text_secondary).wrap()
}
