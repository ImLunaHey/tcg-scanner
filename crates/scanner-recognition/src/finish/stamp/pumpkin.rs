use super::{
    Match,
    components::{self, Component},
    templates::PUMPKIN,
};
use image::RgbImage;

pub(super) fn find(image: &RgbImage) -> Option<Match> {
    let mut best: Option<Match> = None;
    components::visit(image, [0.65, 0.28, 0.98, 0.60], true, orange, |c| {
        let (w, h) = (image.width() as f64, image.height() as f64);
        if !(0.14..=0.27).contains(&(c.width as f64 / w))
            || !(0.09..=0.18).contains(&(c.height as f64 / h))
            || !(0.70..=0.95).contains(&((c.x + c.width / 2) as f64 / w))
            || !(0.34..=0.57).contains(&((c.y + c.height / 2) as f64 / h))
        {
            return;
        }
        let value = Match {
            score: similarity(&c, image.width() as usize),
            region: c.region(image),
            dark_face: dark_face(&c, image),
        };
        if best.as_ref().is_none_or(|b| value.score > b.score) {
            best = Some(value);
        }
    });
    best
}
fn orange(p: &image::Rgb<u8>) -> bool {
    let (r, g, b) = (p[0] as f64 / 255., p[1] as f64 / 255., p[2] as f64 / 255.);
    r > 0.18 && r - g > 0.07 && g - b > 0.05 && g / r > 0.25 && g / r < 0.83 && b / r < 0.65
}
fn similarity(c: &Component, image_width: usize) -> f64 {
    let mut shape = [0f64; 1024];
    let mut totals = [0f64; 1024];
    let bin = |x: usize, y: usize| (y * 32 / c.height).min(31) * 32 + (x * 32 / c.width).min(31);
    for &p in &c.pixels {
        shape[bin(p % image_width - c.x, p / image_width - c.y)] += 1.;
    }
    for y in 0..c.height {
        for x in 0..c.width {
            totals[bin(x, y)] += 1.;
        }
    }
    1. - shape
        .iter()
        .enumerate()
        .map(|(i, s)| (s / totals[i].max(1.) - f64::from(PUMPKIN[i / 32][i % 32] == b'#')).abs())
        .sum::<f64>()
        / 1024.
}
fn dark_face(c: &Component, image: &RgbImage) -> bool {
    [
        (0.27, 0.35, 0.62, 0.67),
        (0.67, 0.75, 0.62, 0.67),
        (0.45, 0.55, 0.77, 0.84),
    ]
    .into_iter()
    .all(|(a, b, d, e)| {
        let (mut dark, mut total) = (0, 0);
        for y in c.y + (d * c.height as f64) as usize..c.y + (e * c.height as f64) as usize {
            for x in c.x + (a * c.width as f64) as usize..c.x + (b * c.width as f64) as usize {
                let p = image.get_pixel(x as u32, y as u32);
                let luminance =
                    (0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64) / 255.;
                total += 1;
                dark += usize::from(luminance < 0.25);
            }
        }
        total > 0 && dark as f64 / total as f64 >= 0.65
    })
}
