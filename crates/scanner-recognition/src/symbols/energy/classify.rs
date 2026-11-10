use serde::Deserialize;
use std::{collections::BTreeMap, sync::LazyLock};

#[derive(Deserialize)]
struct Template {
    name: String,
    ratio: f64,
    mask: String,
}
// Exact masks ported from the existing Swift references; provenance is in
// Docs/energy-symbols.md. No card-specific identities are encoded here.
static TEMPLATES: LazyLock<Vec<Template>> =
    LazyLock::new(|| serde_json::from_str(include_str!("templates.json")).unwrap());

pub(super) fn classify(mask: &[bool], ratio: f64) -> Option<(String, f64)> {
    if mask.len() != 576 || !ratio.is_finite() || ratio <= 0. {
        return None;
    }
    let mut types = BTreeMap::<&str, f64>::new();
    for template in TEMPLATES.iter() {
        if (ratio / template.ratio).ln().abs() >= 0.28 || template.mask.len() != 576 {
            continue;
        }
        let mut best = 0f64;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (mut intersection, mut union) = (0, 0);
                for y in 0..24 {
                    for x in 0..24 {
                        let (tx, ty) = (x + dx, y + dy);
                        let expected = (0..24).contains(&tx)
                            && (0..24).contains(&ty)
                            && template.mask.as_bytes()[(ty * 24 + tx) as usize] == b'#';
                        let actual = mask[(y * 24 + x) as usize];
                        intersection += usize::from(expected && actual);
                        union += usize::from(expected || actual);
                    }
                }
                best = best.max(intersection as f64 / union.max(1) as f64);
            }
        }
        let score = types.entry(&template.name).or_default();
        *score = score.max(best);
    }
    let mut ranked: Vec<_> = types.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (name, score) = *ranked.first()?;
    (score >= 0.70 && score - ranked.get(1).map_or(0., |r| r.1) >= 0.10)
        .then(|| (name.into(), score))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uninformative_shapes_are_not_energy_and_templates_have_valid_dimensions() {
        assert!(classify(&[false; 576], 1.).is_none());
        assert!(classify(&[true; 576], 1.).is_none());
        let circle: Vec<_> = (0..576)
            .map(|i| {
                let (x, y) = ((i % 24) as f64 / 23. - 0.5, (i / 24) as f64 / 23. - 0.5);
                x * x + y * y <= 0.25
            })
            .collect();
        assert!(classify(&circle, 1.).is_none());
        for template in TEMPLATES.iter() {
            assert_eq!(template.mask.len(), 576);
        }
    }
}
