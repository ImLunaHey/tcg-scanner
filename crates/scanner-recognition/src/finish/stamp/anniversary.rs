use super::{
    Match,
    components::{self, Component},
    glyph, templates,
};
use image::RgbImage;
use scanner_types::images::ImageCrop;

struct Digit {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    three: f64,
    zero: f64,
}
pub(super) fn find(image: &RgbImage) -> Option<Match> {
    let mut best: Option<Match> = None;
    for blue in [false, true] {
        let mut digits = vec![];
        components::visit(
            image,
            [0.70, 0.32, 0.98, 0.94],
            false,
            |p| {
                let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
                if blue {
                    b > r + 20 && b > g + 10
                } else {
                    r > g + 35 && r > b + 35
                }
            },
            |c| {
                if !(0.012..=0.045).contains(&(c.height as f64 / image.height() as f64))
                    || !(0.45..=1.2).contains(&(c.width as f64 / c.height as f64))
                {
                    return;
                }
                let mask = normalize(&c, image.width() as usize);
                let (three, zero) = if blue {
                    (&templates::BLUE_THREE, &templates::BLUE_ZERO)
                } else {
                    (&templates::THREE, &templates::ZERO)
                };
                digits.push(Digit {
                    x: c.x,
                    y: c.y,
                    width: c.width,
                    height: c.height,
                    three: glyph::similarity(&mask, three),
                    zero: if glyph::has_counter(&mask) {
                        glyph::similarity(&mask, zero)
                    } else {
                        0.
                    },
                });
            },
        );
        for left in digits.iter().filter(|d| d.three >= 0.75) {
            for right in digits.iter().filter(|d| d.zero >= 0.75 && d.x > left.x) {
                let height = (left.height + right.height) as f64 / 2.;
                if left.y.abs_diff(right.y) as f64 >= height * 0.35
                    || !(0.75..=1.3).contains(&(left.height as f64 / right.height as f64))
                    || !(2.5..=5.5).contains(&((right.x - left.x) as f64 / left.width as f64))
                {
                    continue;
                }
                let score = left.three.min(right.zero);
                if best.as_ref().is_none_or(|b| score > b.score) {
                    best = Some(Match {
                        score,
                        dark_face: false,
                        region: ImageCrop {
                            x: left.x as f64 / image.width() as f64,
                            y: left.y as f64 / image.height() as f64,
                            width: (right.x + right.width - left.x) as f64 / image.width() as f64,
                            height: left.height.max(right.height) as f64 / image.height() as f64,
                        },
                    });
                }
            }
        }
    }
    best
}
fn normalize(c: &Component, image_width: usize) -> [bool; 576] {
    let mut pixels = vec![false; c.width * c.height];
    for &p in &c.pixels {
        pixels[(p / image_width - c.y) * c.width + p % image_width - c.x] = true;
    }
    std::array::from_fn(|i| {
        let x = (((i % 24) as f64 + 0.5) * c.width as f64 / 24.) as usize;
        let y = (((i / 24) as f64 + 0.5) * c.height as f64 / 24.) as usize;
        pixels[y.min(c.height - 1) * c.width + x.min(c.width - 1)]
    })
}
