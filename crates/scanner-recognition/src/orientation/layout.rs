use image::{RgbImage, imageops};

/// Colour/layout is supporting evidence only; the caller must corroborate logos.
pub(crate) fn pokemon(image: &RgbImage) -> Option<u8> {
    let small = crate::preprocess::thumbnail(image, 256);
    let matches: Vec<_> = (0..4)
        .filter(|&turns| pokemon_upright(&rotate(&small, turns)))
        .collect();
    (matches.len() == 1).then(|| matches[0])
}
pub(super) fn rotate(image: &RgbImage, turns: u8) -> RgbImage {
    match turns % 4 {
        1 => imageops::rotate90(image),
        2 => imageops::rotate180(image),
        3 => imageops::rotate270(image),
        _ => image.clone(),
    }
}
fn pokemon_upright(image: &RgbImage) -> bool {
    let (w, h) = image.dimensions();
    if h == 0 || !(0.65..=0.80).contains(&(f64::from(w) / f64::from(h))) {
        return false;
    }
    let (mut border, mut blue, mut top, mut top_yellow, mut bottom, mut bottom_yellow) =
        (0., 0., 0., 0., 0., 0.);
    let (mut center, mut red, mut red_y, mut white, mut lower) = (0., 0., 0., 0., 0.);
    for (x, y, pixel) in image.enumerate_pixels() {
        let [r, g, b] = pixel.0.map(f64::from);
        let nx = f64::from(x) / f64::from(w);
        let ny = f64::from(y) / f64::from(h);
        if nx < 0.045 || nx > 0.955 || ny < 0.035 || ny > 0.965 {
            border += 1.;
            if b > r * 1.3 && b > g * 1.15 && b > 25. {
                blue += 1.;
            }
        }
        let yellow = r > 120. && g > 90. && b < r.min(g) * 0.65;
        if nx > 0.08 && nx < 0.92 && ny > 0.07 && ny < 0.28 {
            top += 1.;
            if yellow {
                top_yellow += 1.;
            }
        }
        if nx > 0.08 && nx < 0.92 && ny > 0.72 && ny < 0.93 {
            bottom += 1.;
            if yellow {
                bottom_yellow += 1.;
            }
        }
        if nx > 0.25 && nx < 0.75 && ny > 0.30 && ny < 0.70 {
            center += 1.;
            if r > 95. && r > g * 1.4 && r > b * 1.4 {
                red += 1.;
                red_y += ny;
            }
        }
        if nx > 0.30 && nx < 0.65 && ny > 0.53 && ny < 0.66 {
            lower += 1.;
            if r.min(g).min(b) > 110. && r.max(g).max(b) - r.min(g).min(b) < 65. {
                white += 1.;
            }
        }
    }
    blue / border > 0.65
        && top_yellow / top > 0.08
        && bottom_yellow / bottom > 0.08
        && red / center > 0.07
        && (0.34..=0.48).contains(&(red_y / red))
        && white / lower > 0.35
}

pub(super) fn yugioh(image: &RgbImage) -> bool {
    let small = crate::preprocess::thumbnail(image, 256);
    let (w, h) = small.dimensions();
    if w.max(h) == 0 || !(0.63..=0.77).contains(&(f64::from(w.min(h)) / f64::from(w.max(h)))) {
        return false;
    }
    let (mut center, mut dark, mut ring, mut warm) = (0., 0., 0., 0.);
    for (x, y, pixel) in small.enumerate_pixels() {
        let [r, g, b] = pixel.0.map(f64::from);
        let nx = f64::from(x) / f64::from(w);
        let ny = f64::from(y) / f64::from(h);
        if (nx - 0.5).abs() < 0.12 && (ny - 0.5).abs() < 0.12 {
            center += 1.;
            if r.max(g).max(b) < 70. {
                dark += 1.;
            }
        } else if nx > 0.035 && nx < 0.965 && ny > 0.035 && ny < 0.965 {
            ring += 1.;
            if r > 65. && r > g * 1.18 && g > b * 1.15 {
                warm += 1.;
            }
        }
    }
    dark / center > 0.65 && warm / ring > 0.15
}

pub(super) fn starwars(image: &RgbImage) -> bool {
    let small = crate::preprocess::thumbnail(image, 200);
    let (w, h) = small.dimensions();
    if w.max(h) == 0 || !(0.62..=0.79).contains(&(f64::from(w.min(h)) / f64::from(w.max(h)))) {
        return false;
    }
    let (mut dark, mut white, mut center) = (0., 0., 0.);
    for (x, y, pixel) in small.enumerate_pixels() {
        let [r, g, b] = pixel.0;
        if r.max(g).max(b) < 55 {
            dark += 1.;
        }
        if (f64::from(x) / f64::from(w) - 0.5).abs() < 0.3
            && (f64::from(y) / f64::from(h) - 0.5).abs() < 0.15
        {
            center += 1.;
            if r.min(g).min(b) > 180 {
                white += 1.;
            }
        }
    }
    dark / f64::from(w * h) > 0.65 && white / center > 0.25
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_pokemon_layouts_keep_orientation_under_all_quarter_turns() {
        let Some(root) = std::env::var_os("SCANNER_BENCH_DIR") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        for id in ["003", "005", "011", "015", "019", "021"] {
            let bytes = std::fs::read(root.join(format!("images/negative-{id}.png"))).unwrap();
            let image = scanner_images::decode::decode(&bytes).unwrap().to_rgb8();
            for turn in 0..4 {
                assert_eq!(
                    pokemon(&rotate(&image, turn)),
                    Some((4 - turn) % 4),
                    "back {id}, input rotation {turn}"
                );
            }
        }
    }
    #[test]
    fn plain_fields_are_not_known_layouts() {
        for color in [[0, 0, 0], [20, 30, 190], [255, 255, 255], [140, 80, 40]] {
            let image = RgbImage::from_pixel(70, 100, image::Rgb(color));
            assert_eq!(pokemon(&image), None);
            assert!(!yugioh(&image));
            assert!(!starwars(&image));
        }
    }
}
