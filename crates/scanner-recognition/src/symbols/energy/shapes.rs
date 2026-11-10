use super::super::components;
use image::RgbImage;
use scanner_types::images::ImageCrop;
pub(super) struct Shape {
    pub mask: Vec<bool>,
    pub ratio: f64,
    pub region: ImageCrop,
}

pub(super) fn collect(image: &RgbImage, threshold: u8, grouping: f64) -> Vec<Shape> {
    let (iw, ih) = (image.width() as usize, image.height() as usize);
    let (left, right, bottom) = (
        (iw as f64 * 0.82).floor() as usize,
        (iw as f64 * 0.995).ceil() as usize,
        (ih as f64 * 0.18).ceil() as usize,
    );
    let (w, h) = (right.min(iw) - left, bottom.min(ih));
    if w == 0 || h == 0 || w * h > 300_000 {
        return vec![];
    }
    let mask: Vec<_> = (0..w * h)
        .map(|i| {
            let [r, g, b] = image.get_pixel((left + i % w) as u32, (i / w) as u32).0;
            r.max(g).max(b) < threshold && r.max(g).max(b) - r.min(g).min(b) < 65
        })
        .collect();
    let joined = components::join(&mask, w, h, (iw as f64 * grouping).ceil().max(1.) as usize);
    let mut result = vec![];
    components::visit(&mask, &joined, w, h, |c| {
        if c.points.len() < 10
            || c.left == 0
            || c.top == 0
            || c.left + c.width >= w
            || c.top + c.height >= h
            || !(0.02..=0.115).contains(&(c.width as f64 / iw as f64))
            || !(0.012..=0.09).contains(&(c.height as f64 / ih as f64))
        {
            return;
        }
        let ink: std::collections::HashSet<_> = c.points.into_iter().collect();
        let normalized = (0..576)
            .map(|i| {
                let x = c.left + (((i % 24) as f64 + 0.5) * c.width as f64 / 24.) as usize;
                let y = c.top + (((i / 24) as f64 + 0.5) * c.height as f64 / 24.) as usize;
                ink.contains(&(y * w + x))
            })
            .collect();
        result.push(Shape {
            mask: normalized,
            ratio: c.width as f64 / c.height as f64,
            region: ImageCrop {
                x: (left + c.left) as f64 / iw as f64,
                y: c.top as f64 / ih as f64,
                width: c.width as f64 / iw as f64,
                height: c.height as f64 / ih as f64,
            },
        });
    });
    result
}
