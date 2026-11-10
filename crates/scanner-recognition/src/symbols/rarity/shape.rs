use super::super::components;
use image::RgbImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Circle,
    Diamond,
    Star,
}
impl Shape {
    fn name(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Diamond => "diamond",
            Self::Star => "star",
        }
    }
}
pub(super) fn classify(image: &RgbImage, threshold: u8) -> Option<(String, f64)> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    if w == 0 || h == 0 || w * h > 100_000 {
        return None;
    }
    let mask: Vec<_> = image
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0;
            r.max(g).max(b) < threshold && r.max(g).max(b) - r.min(g).min(b) < 60
        })
        .collect();
    let mut accepted = vec![];
    components::visit(&mask, &mask, w, h, |c| {
        if c.points.len() < 8
            || c.left == 0
            || c.top == 0
            || c.left + c.width >= w
            || c.top + c.height >= h
            || c.height < 6
            || (c.height as f64 / h as f64) < 0.18
            || !(0.75..=1.3).contains(&(c.width as f64 / c.height as f64))
        {
            return;
        }
        let ink: std::collections::HashSet<_> = c.points.into_iter().collect();
        let mut scores: Vec<_> = [Shape::Circle, Shape::Diamond, Shape::Star]
            .into_iter()
            .map(|shape| {
                let (mut intersection, mut union) = (0, 0);
                for y in 0..c.height {
                    for x in 0..c.width {
                        let expected = template(
                            shape,
                            (x as f64 + 0.5) / c.width as f64,
                            (y as f64 + 0.5) / c.height as f64,
                        );
                        let actual = ink.contains(&((c.top + y) * w + c.left + x));
                        intersection += usize::from(expected && actual);
                        union += usize::from(expected || actual);
                    }
                }
                (shape, intersection as f64 / union.max(1) as f64)
            })
            .collect();
        scores.sort_by(|a, b| b.1.total_cmp(&a.1));
        if scores[0].1 >= 0.82 && scores[0].1 - scores[1].1 >= 0.08 {
            accepted.push((c.left, scores[0].0, scores[0].1));
        }
    });
    accepted.sort_by_key(|c| c.0);
    let first = accepted.first()?;
    if first.0 >= h {
        return None;
    }
    if accepted.len() == 1 {
        return Some((first.1.name().into(), first.2));
    }
    if accepted.len() == 2
        && accepted.iter().all(|c| c.1 == Shape::Star)
        && ((accepted[1].0 - first.0) as f64) < (h as f64 * 1.5)
    {
        return Some(("doubleStar".into(), first.2.min(accepted[1].2)));
    }
    None
}
fn template(shape: Shape, x: f64, y: f64) -> bool {
    match shape {
        Shape::Circle => (x - 0.5).powi(2) + (y - 0.5).powi(2) <= 0.25,
        Shape::Diamond => (x - 0.5).abs() + (y - 0.5).abs() <= 0.5,
        Shape::Star => {
            static VERTICES: std::sync::LazyLock<Vec<(f64, f64)>> =
                std::sync::LazyLock::new(|| {
                    let points: Vec<_> = (0..10)
                        .map(|i| {
                            let a =
                                i as f64 * std::f64::consts::PI / 5. - std::f64::consts::PI / 2.;
                            let r = if i % 2 == 0 { 0.5 } else { 0.28 };
                            (0.5 + a.cos() * r, 0.5 + a.sin() * r)
                        })
                        .collect();
                    let (minx, maxx, miny, maxy) = points
                        .iter()
                        .fold((1f64, 0f64, 1f64, 0f64), |(a, b, c, d), p| {
                            (a.min(p.0), b.max(p.0), c.min(p.1), d.max(p.1))
                        });
                    points
                        .into_iter()
                        .map(|p| ((p.0 - minx) / (maxx - minx), (p.1 - miny) / (maxy - miny)))
                        .collect()
                });
            let mut inside = false;
            for i in 0..10 {
                let (a, b) = (VERTICES[i], VERTICES[(i + 1) % 10]);
                if (a.1 > y) != (b.1 > y) && x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 {
                    inside = !inside;
                }
            }
            inside
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_printed_shape_but_rejects_clipping_blocks_and_distant_marks() {
        let mut image = RgbImage::from_pixel(80, 40, image::Rgb([255; 3]));
        for y in 10..30 {
            for x in 4..24 {
                if (x as i32 - 14).abs() + (y as i32 - 20).abs() <= 9 {
                    image.put_pixel(x, y, image::Rgb([0; 3]));
                }
            }
        }
        assert_eq!(classify(&image, 90).unwrap().0, "diamond");
        let clipped = image::imageops::crop_imm(&image, 5, 0, 75, 40).to_image();
        assert!(classify(&clipped, 90).is_none());
        assert!(classify(&RgbImage::from_pixel(80, 40, image::Rgb([0; 3])), 90).is_none());
        let mut distant = RgbImage::from_pixel(100, 40, image::Rgb([255; 3]));
        image::imageops::replace(&mut distant, &image, 50, 0);
        assert!(classify(&distant, 90).is_none());
    }
}
