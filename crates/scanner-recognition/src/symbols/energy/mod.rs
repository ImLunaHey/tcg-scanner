mod classify;
mod shapes;
use image::RgbImage;
use scanner_types::{printed::EnergySymbol, recognition::RecognizedLine};

pub fn has_heading(lines: &[RecognizedLine]) -> bool {
    let title = lines.iter().any(|l| {
        l.confidence >= 0.7
            && l.y > 0.8
            && l.x < 0.7
            && matches!(
                l.text
                    .chars()
                    .filter(|c| c.is_alphabetic())
                    .flat_map(char::to_uppercase)
                    .collect::<String>()
                    .as_str(),
                "ENERGY" | "BASICENERGY" | "기본에너지" | "에너지"
            )
    });
    let body = lines.iter().any(|l| {
        l.confidence >= 0.6
            && l.y > 0.12
            && l.y < 0.8
            && l.text.chars().filter(|c| c.is_alphabetic()).count() > 3
    });
    title && !body
}

/// Requires an Energy heading, no readable body rules, and agreement between
/// multiple thresholds. Only neutral ink in the small corner is examined.
pub fn analyze(upright: &RgbImage, lines: &[RecognizedLine]) -> Option<EnergySymbol> {
    if !has_heading(lines)
        || upright.width() < 400
        || !(0.60..=0.80).contains(&(upright.width() as f64 / upright.height() as f64))
    {
        return None;
    }
    let image = crate::preprocess::thumbnail(upright, 2400);
    let mut readings = vec![];
    for threshold in [25, 35, 45, 65, 90, 120, 150] {
        let mut matches: Vec<_> = [0., 0.003]
            .into_iter()
            .flat_map(|grouping| shapes::collect(&image, threshold, grouping))
            .filter_map(|s| {
                classify::classify(&s.mask, s.ratio).map(|(name, score)| (name, score, s.region))
            })
            .collect();
        if matches.iter().any(|m| m.0 != matches[0].0) {
            return None;
        }
        matches.sort_by(|a, b| b.1.total_cmp(&a.1));
        if let Some(best) = matches.into_iter().next() {
            readings.push(best);
        }
    }
    let best = readings.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
    if readings.len() < 2
        || readings.iter().any(|r| {
            r.0 != best.0
                || (r.2.x + r.2.width / 2. - best.2.x - best.2.width / 2.).abs() >= 0.015
                || (r.2.y + r.2.height / 2. - best.2.y - best.2.height / 2.).abs() >= 0.015
        })
    {
        return None;
    }
    Some(EnergySymbol {
        method: "energy-corner-shape-v2".into(),
        energy_type: best.0.clone(),
        similarity: readings.iter().map(|r| r.1).fold(1., f64::min),
        region: best.2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_corner_measurements_agree_and_covering_the_corner_removes_evidence() {
        let Some(path) = std::env::var_os("SCANNER_ENERGY_FIXTURES") else {
            return;
        };
        let fixtures: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for fixture in fixtures.as_array().unwrap() {
            let bytes = std::fs::read(fixture["path"].as_str().unwrap()).unwrap();
            let pixels = scanner_images::decode::decode(&bytes).unwrap().to_rgb8();
            let mut pixels = match fixture["turns"].as_u64().unwrap() {
                1 => image::imageops::rotate90(&pixels),
                2 => image::imageops::rotate180(&pixels),
                3 => image::imageops::rotate270(&pixels),
                _ => pixels,
            };
            let lines =
                serde_json::from_value::<Vec<RecognizedLine>>(fixture["lines"].clone()).unwrap();
            let result = analyze(&pixels, &lines);
            eprintln!(
                "{} expected={} measured={:?}",
                fixture["id"], fixture["name"], result
            );
            assert_eq!(
                result.map(|r| r.energy_type).as_deref(),
                fixture["name"].as_str()
            );
            for y in 0..(pixels.height() as f64 * 0.18).ceil() as u32 {
                for x in (pixels.width() as f64 * 0.82).floor() as u32..pixels.width() {
                    pixels.put_pixel(x, y, image::Rgb([255; 3]));
                }
            }
            assert!(analyze(&pixels, &lines).is_none());
        }
    }
    #[test]
    fn special_energy_and_pokemon_rules_cannot_become_basic_energy() {
        let mut lines = vec![RecognizedLine {
            text: "Basic Energy".into(),
            confidence: 0.9,
            x: 0.1,
            y: 0.9,
            width: 0.5,
            height: 0.04,
        }];
        assert!(has_heading(&lines));
        lines.push(RecognizedLine {
            text: "Attach this card".into(),
            y: 0.4,
            ..lines[0].clone()
        });
        assert!(!has_heading(&lines));
        lines.pop();
        lines[0].text = "Special Energy".into();
        assert!(!has_heading(&lines));
        lines[0].text = "기본 에너지".into();
        assert!(has_heading(&lines));
        assert!(
            analyze(
                &RgbImage::from_pixel(630, 880, image::Rgb([128, 0, 128])),
                &lines
            )
            .is_none()
        );
    }
}
