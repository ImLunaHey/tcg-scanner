pub use scanner_types::recognition::RecognizedLine;
use std::collections::BTreeMap;

#[derive(Default)]
struct Line {
    words: Vec<String>,
    confidence: f64,
    weight: usize,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

pub(super) fn parse(tsv: &str, width: u32, height: u32) -> Vec<RecognizedLine> {
    let mut lines: BTreeMap<(u32, u32, u32, u32), Line> = BTreeMap::new();
    for row in tsv.lines().skip(1) {
        let fields: Vec<_> = row.splitn(12, '\t').collect();
        if fields.len() != 12 || fields[0] != "5" || fields[11].trim().is_empty() {
            continue;
        }
        let number = |i: usize| fields[i].parse::<u32>().ok();
        let Some((page, block, para, line, left, top, w, h, confidence)) = (|| {
            Some((
                number(1)?,
                number(2)?,
                number(3)?,
                number(4)?,
                number(6)?,
                number(7)?,
                number(8)?,
                number(9)?,
                fields[10].parse::<f64>().ok()?,
            ))
        })() else {
            continue;
        };
        if !confidence.is_finite() || confidence < 0. || left >= width || top >= height {
            continue;
        }
        let entry = lines
            .entry((page, block, para, line))
            .or_insert_with(|| Line {
                left,
                top,
                ..Line::default()
            });
        let word = fields[11].trim();
        let weight = word.chars().count();
        entry.confidence += confidence.clamp(0., 100.) * weight as f64;
        entry.weight += weight;
        entry.words.push(word.into());
        entry.left = entry.left.min(left);
        entry.top = entry.top.min(top);
        entry.right = entry.right.max(left.saturating_add(w).min(width));
        entry.bottom = entry.bottom.max(top.saturating_add(h).min(height));
    }
    lines
        .into_values()
        .map(|line| RecognizedLine {
            text: line.words.join(" "),
            confidence: (line.confidence / line.weight as f64 / 100.) as f32,
            x: line.left as f64 / width as f64,
            y: 1. - line.bottom as f64 / height as f64,
            width: (line.right - line.left) as f64 / width as f64,
            height: (line.bottom - line.top) as f64 / height as f64,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assembles_words_and_converts_top_left_to_bottom_left() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t10\t20\t30\t10\t90\tBasic\n5\t1\t1\t1\t1\t2\t45\t20\t40\t10\t80\tEnergy\n";
        let lines = parse(tsv, 100, 200);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Basic Energy");
        assert_eq!(lines[0].y, 0.85);
        assert_eq!(lines[0].width, 0.75);
        assert!((0.8..0.9).contains(&lines[0].confidence));
    }
}
