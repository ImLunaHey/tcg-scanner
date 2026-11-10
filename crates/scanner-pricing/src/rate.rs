//! Fixed ECB daily XML: bounded input, no DTD/entity resolution, one dated GBP rate.
use chrono::{DateTime, Utc};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use rust_decimal::Decimal;
use scanner_types::{
    error::{Result, ScannerError},
    pricing::EuroSterlingRate,
};
use std::str::FromStr;
#[derive(Default)]
struct Parsed {
    date: Option<String>,
    rate: Option<Decimal>,
    date_depth: Option<usize>,
}
pub fn decode(bytes: &[u8], now: DateTime<Utc>) -> Result<EuroSterlingRate> {
    if bytes.len() > 64 * 1024 {
        return Err(invalid());
    }
    let mut reader = Reader::from_reader(bytes);
    let (mut parsed, mut depth, mut root) = (Parsed::default(), 0usize, false);
    loop {
        match reader.read_event().map_err(|_| invalid())? {
            Event::DocType(_) => return Err(invalid()),
            Event::Start(ref tag) => {
                if depth == 0 {
                    if root || tag.local_name().as_ref() != "Envelope" {
                        return Err(invalid());
                    }
                    root = true;
                }
                depth += 1;
                if depth > 32 {
                    return Err(invalid());
                }
                parsed.cube(tag, depth)?;
            }
            Event::Empty(ref tag) => {
                if depth == 0 {
                    return Err(invalid());
                }
                parsed.cube(tag, depth + 1)?;
            }
            Event::End(_) => {
                if parsed.date_depth == Some(depth) {
                    parsed.date_depth = None;
                }
                depth = depth.checked_sub(1).ok_or_else(invalid)?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let date = parsed.date.ok_or_else(invalid)?;
    let rate = parsed.rate.ok_or_else(invalid)?;
    let day = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| invalid())?;
    if depth != 0
        || date.len() != 10
        || day.format("%Y-%m-%d").to_string() != date
        || day > now.date_naive() + chrono::Duration::days(1)
        || day <= now.date_naive() - chrono::Duration::days(14)
        || rate <= Decimal::ZERO
        || rate >= Decimal::TEN
    {
        return Err(invalid());
    }
    Ok(EuroSterlingRate {
        pounds_per_euro: rate,
        date,
        fetched_at: now,
    })
}
impl Parsed {
    fn cube(&mut self, tag: &BytesStart<'_>, depth: usize) -> Result<()> {
        if tag.local_name().as_ref() != "Cube" {
            return Ok(());
        }
        let (mut currency, mut amount) = (None, None);
        for attr in tag.attributes() {
            let attr = attr.map_err(|_| invalid())?;
            let text = attr.value.as_ref();
            match attr.key.as_ref() {
                "time" => {
                    if self.date.is_some() {
                        return Err(invalid());
                    }
                    self.date = Some(text.into());
                    self.date_depth = Some(depth);
                }
                "currency" => currency = Some(text.to_owned()),
                "rate" => amount = Some(text.to_owned()),
                _ => {}
            }
        }
        if currency.as_deref() == Some("GBP") {
            if self.rate.is_some() || !self.date_depth.is_some_and(|d| depth > d) {
                return Err(invalid());
            }
            self.rate = Some(
                Decimal::from_str(amount.as_deref().ok_or_else(invalid)?).map_err(|_| invalid())?,
            );
        }
        Ok(())
    }
}
fn invalid() -> ScannerError {
    ScannerError::new(
        "pricingUnavailable",
        "Invalid or outdated ECB exchange rate.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn now() -> DateTime<Utc> {
        "2026-11-10T02:00:00Z".parse().unwrap()
    }
    fn xml(date: &str, rate: &str) -> String {
        format!(
            "<Envelope><Cube><Cube time='{date}'><Cube currency='USD' rate='1.1'/><Cube currency='GBP' rate='{rate}'/></Cube></Cube></Envelope>"
        )
    }
    #[test]
    fn daily_rate_rejects_stale_future_duplicate_and_entity_inputs() {
        let value = decode(xml("2026-11-09", "0.87215").as_bytes(), now()).unwrap();
        assert_eq!(value.pounds_per_euro, Decimal::new(87215, 5));
        for (date, rate) in [
            ("2026-10-01", "0.8"),
            ("2026-11-30", "0.8"),
            ("2026-11-09", "NaN"),
            ("2026-11-09", "0"),
            ("2026-11-09", "10"),
            ("2026-02-30", "0.8"),
        ] {
            assert!(decode(xml(date, rate).as_bytes(), now()).is_err());
        }
        let duplicate = xml("2026-11-09", "0.8").replace("currency='USD'", "currency='GBP'");
        assert!(decode(duplicate.as_bytes(), now()).is_err());
        let entity = format!(
            "<!DOCTYPE x [<!ENTITY rate SYSTEM 'file:///private'>]>{}",
            xml("2026-11-09", "&rate;")
        );
        assert!(decode(entity.as_bytes(), now()).is_err());
        assert!(
            decode(
                b"<Envelope><Cube time='2026-11-09'/><Cube currency='GBP' rate='0.8'/></Envelope>",
                now()
            )
            .is_err()
        );
        assert!(decode(&vec![b'x'; 64 * 1024 + 1], now()).is_err());
    }
}
