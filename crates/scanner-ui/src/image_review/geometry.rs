use scanner_types::images::ImageCrop;
pub(super) fn valid(r: ImageCrop) -> bool {
    [r.x, r.y, r.width, r.height]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.)
        && r.width > 0.
        && r.height > 0.
        && r.x + r.width <= 1.
        && r.y + r.height <= 1.
}
pub(super) fn expanded(r: ImageCrop) -> Option<ImageCrop> {
    if !valid(r) {
        return None;
    }
    let (x, y) = ((r.x - 0.04).max(0.), (r.y - 0.04).max(0.));
    Some(ImageCrop {
        x,
        y,
        width: (r.x + r.width + 0.04).min(1.) - x,
        height: (r.y + r.height + 0.04).min(1.) - y,
    })
}
/// Whole image placement in a clipped viewport, with an optional source crop.
pub(super) fn placement(frame: [f32; 2], image: [u32; 2], crop: ImageCrop) -> Option<[f32; 4]> {
    if !valid(crop) || frame.iter().any(|v| !v.is_finite() || *v <= 0.) || image.contains(&0) {
        return None;
    }
    let (cw, ch) = (image[0] as f64 * crop.width, image[1] as f64 * crop.height);
    let scale = (frame[0] as f64 / cw).min(frame[1] as f64 / ch);
    let (w, h) = (image[0] as f64 * scale, image[1] as f64 * scale);
    Some([
        ((frame[0] as f64 - cw * scale) / 2. - crop.x * w) as f32,
        ((frame[1] as f64 - ch * scale) / 2. - crop.y * h) as f32,
        w as f32,
        h as f32,
    ])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_and_overlay_keep_the_same_coordinates_through_letterboxing_and_zoom() {
        assert_eq!(
            placement([600., 400.], [200, 400], Default::default()),
            Some([200., 0., 200., 400.])
        );
        let r = ImageCrop {
            x: 0.25,
            y: 0.4,
            width: 0.5,
            height: 0.2,
        };
        let [x, y, w, h] = placement([600., 400.], [200, 400], r).unwrap();
        // The selected region fills the height; horizontal letterboxing is 50px.
        assert!((x + r.x as f32 * w - 50.).abs() < 0.01);
        assert!((y + r.y as f32 * h).abs() < 0.01);
        assert!((r.height as f32 * h - 400.).abs() < 0.01);
        let edge = expanded(ImageCrop {
            x: 0.,
            y: 0.99,
            width: 0.01,
            height: 0.01,
        })
        .unwrap();
        assert_eq!(edge.x, 0.);
        assert_eq!(edge.y + edge.height, 1.);
        assert!(placement([0., 400.], [200, 400], r).is_none());
        assert!(expanded(ImageCrop { x: f64::NAN, ..r }).is_none());
    }
}
