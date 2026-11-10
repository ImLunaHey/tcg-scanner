use crate::Extensions;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueCard {
    pub id: String,
    pub local_id: String,
    pub name: String,
    pub image: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardCounts {
    pub official: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSet {
    pub id: String,
    pub name: String,
    pub card_count: Option<CardCounts>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedVariant {
    pub variant_id: Option<String>,
    #[serde(rename = "type")]
    pub variant_type: String,
    pub size: Option<String>,
    pub stamp: Option<Vec<String>>,
    pub subtype: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetail {
    pub id: String,
    pub local_id: String,
    pub name: String,
    pub set: CardSet,
    pub image: Option<String>,
    pub variants: Option<std::collections::BTreeMap<String, bool>>,
    pub rarity: Option<String>,
    pub category: Option<String>,
    pub types: Option<Vec<String>>,
    #[serde(rename = "variants_detailed")]
    pub variants_detailed: Option<Vec<DetailedVariant>>,
    #[serde(flatten)]
    pub extensions: Extensions,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDetail {
    pub id: String,
    pub release_date: Option<String>,
    pub cards: Vec<CatalogueCard>,
}

impl CardDetail {
    /// Standard-size, fully specified physical variants only.
    pub fn printing_variants(&self) -> Vec<crate::edition::PrintingVariant> {
        use crate::recognition::CardFinish;
        self.variants_detailed
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| {
                let id = v.variant_id.as_ref().filter(|s| !s.is_empty())?;
                if v.size.as_deref() != Some("standard") || v.subtype.is_some() {
                    return None;
                }
                let finish = match v.variant_type.as_str() {
                    "normal" => CardFinish::Normal,
                    "holo" => CardFinish::Holo,
                    "reverse" => CardFinish::ReverseHolo,
                    _ => return None,
                };
                Some(crate::edition::PrintingVariant {
                    id: id.clone(),
                    source: "tcgdex".into(),
                    finish,
                    stamps: v.stamp.clone().unwrap_or_default(),
                    years: vec![],
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn printing_variants_exclude_jumbo_subtypes_unknown_finish_and_missing_ids() {
        let mut detail: CardDetail = serde_json::from_value(
            serde_json::json!({"id":"sv01-104","name":"Greavard","localId":"104",
            "set":{"id":"sv01","name":"Scarlet & Violet"},"variants_detailed":[
                {"variantId":"1","type":"reverse","size":"standard","stamp":["trick-or-trade"]},
                {"variantId":"2","type":"holo","size":"jumbo"},
                {"variantId":"3","type":"holo","size":"standard","subtype":"cosmos"},
                {"variantId":"4","type":"other","size":"standard"},
                {"type":"normal","size":"standard"}]}),
        )
        .unwrap();
        let variants = detail.printing_variants();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].id, "1");
        assert_eq!(
            variants[0].finish,
            crate::recognition::CardFinish::ReverseHolo
        );
        detail.variants_detailed = None;
        assert!(detail.printing_variants().is_empty());
    }
}
