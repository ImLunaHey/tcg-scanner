use crate::{
    decode::decode,
    deskew::{self, Point},
};
use image::RgbaImage;
use scanner_types::{
    Extensions,
    error::{Result, ScannerError},
    images::CardPreparation,
    scan::ScanBackground,
};

pub struct Boundary {
    pub points: Vec<Point>,
    pub touches_edge: bool,
}

/// Geometry only; orientation must be established by recognition before verified is set.
pub fn prepare_geometry(bytes: &[u8], background: ScanBackground) -> Result<CardPreparation> {
    let image = decode(bytes)?.thumbnail(1200, 1200).to_rgba8();
    let boundary = foreground(&image, background, false)?;
    let geometry = deskew::analyze(&boundary.points, image.width(), image.height());
    let mut warnings = geometry.warnings;
    warnings.push(
        "Orientation has not yet been established by text or a known card-back pattern.".into(),
    );
    if boundary.touches_edge {
        warnings.push(
            "Card is close to a scan boundary. Check that the original captured the entire edge."
                .into(),
        );
    }
    Ok(CardPreparation {
        crop: geometry.crop,
        quarter_turns: 0,
        orientation_verified: false,
        warnings,
        source_crop: Some(deskew::bounds(
            &boundary.points,
            image.width(),
            image.height(),
            0.,
        )),
        deskew_degrees: Some(geometry.degrees),
        orientation_basis: Some("unknown".into()),
        method: "card-boundary-rust-v1".into(),
        extensions: Extensions::new(),
    })
}

#[derive(Clone)]
struct Component {
    pixels: Vec<usize>,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

/// The dark union is only a candidate: callers must independently verify SWU branding.
pub fn foreground(
    image: &RgbaImage,
    background: ScanBackground,
    dark_union: bool,
) -> Result<Boundary> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    if width == 0 || height == 0 || width * height > 1_440_000 {
        return Err(not_found("Boundary analysis exceeds 1.44 megapixels."));
    }
    let mut mask = foreground_mask(image, background, dark_union);
    let mut components = vec![];
    for start in 0..mask.len() {
        if !mask[start] {
            continue;
        }
        let component = flood(&mut mask, start, width, height);
        if component.pixels.len() > if dark_union { 8 } else { width * height / 500 } {
            components.push(component);
        }
    }
    let Some(mut card) = components.iter().max_by_key(|c| c.pixels.len()).cloned() else {
        return Err(not_found(
            "No clear card boundary found against the selected background.",
        ));
    };
    if dark_union {
        card.pixels = components
            .iter()
            .flat_map(|c| c.pixels.iter().copied())
            .collect();
        for c in &components {
            card.left = card.left.min(c.left);
            card.right = card.right.max(c.right);
            card.top = card.top.min(c.top);
            card.bottom = card.bottom.max(c.bottom);
        }
    }
    validate_shape(&card, &components, width, height)?;
    let points = endpoints(&card.pixels, width, height);
    Ok(Boundary {
        points,
        touches_edge: card.left == 0
            || card.top == 0
            || card.right == width - 1
            || card.bottom == height - 1,
    })
}

fn foreground_mask(image: &RgbaImage, background: ScanBackground, dark_union: bool) -> Vec<bool> {
    let width = image.width() as usize;
    let backing: Vec<f64> = if background == ScanBackground::White {
        image
            .rows()
            .map(|row| {
                let margin = (width / 10).max(1);
                let mut samples: Vec<u16> = row
                    .enumerate()
                    .filter(|(x, _)| *x < margin || *x >= width - margin)
                    .map(|(_, p)| u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2]))
                    .collect();
                samples.sort_unstable();
                f64::from(samples[samples.len() / 2]) / 3.
            })
            .collect()
    } else {
        vec![]
    };
    image
        .pixels()
        .enumerate()
        .map(|(i, p)| {
            let (r, g, b) = (u16::from(p[0]), u16::from(p[1]), u16::from(p[2]));
            let (max, min) = (r.max(g).max(b), r.min(g).min(b));
            match background {
                ScanBackground::Black => {
                    max > if dark_union { 5 } else { 50 }
                        && r + g + b > if dark_union { 7 } else { 70 }
                }
                ScanBackground::White => {
                    backing[i / width] - f64::from(r + g + b) / 3. > 45. || max - min > 45
                }
            }
        })
        .collect()
}

fn flood(mask: &mut [bool], start: usize, width: usize, height: usize) -> Component {
    let mut c = Component {
        pixels: vec![start],
        left: width,
        right: 0,
        top: height,
        bottom: 0,
    };
    mask[start] = false;
    let mut head = 0;
    while head < c.pixels.len() {
        let pixel = c.pixels[head];
        head += 1;
        let (x, y) = (pixel % width, pixel / width);
        c.left = c.left.min(x);
        c.right = c.right.max(x);
        c.top = c.top.min(y);
        c.bottom = c.bottom.max(y);
        for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
            for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                let next = ny * width + nx;
                if mask[next] {
                    mask[next] = false;
                    c.pixels.push(next);
                }
            }
        }
    }
    c
}

fn validate_shape(
    card: &Component,
    others: &[Component],
    width: usize,
    height: usize,
) -> Result<()> {
    let (w, h) = (
        (card.right - card.left + 1) as f64,
        (card.bottom - card.top + 1) as f64,
    );
    let area = (width * height) as f64;
    if !(0.60..=0.83).contains(&(w.min(h) / w.max(h)))
        || w * h < area * 0.015
        || w * h >= area * 0.85
    {
        return Err(not_found(
            "Detected shape does not have a supported card aspect ratio. Use manual cropping.",
        ));
    }
    if others.iter().any(|other| {
        other.pixels.len() > card.pixels.len() / 5
            && (other.right < card.left
                || other.left > card.right
                || other.bottom < card.top
                || other.top > card.bottom)
    }) {
        return Err(not_found(
            "More than one substantial object found. Crop manually to avoid losing a card.",
        ));
    }
    Ok(())
}

fn endpoints(pixels: &[usize], width: usize, height: usize) -> Vec<Point> {
    let mut rows = vec![None::<(usize, usize)>; height];
    let mut cols = vec![None::<(usize, usize)>; width];
    for &p in pixels {
        let (x, y) = (p % width, p / width);
        let r = rows[y].get_or_insert((x, x));
        r.0 = r.0.min(x);
        r.1 = r.1.max(x);
        let c = cols[x].get_or_insert((y, y));
        c.0 = c.0.min(y);
        c.1 = c.1.max(y);
    }
    let mut points = vec![];
    for (y, range) in rows.into_iter().enumerate() {
        if let Some((l, r)) = range {
            for y in [y, y + 1] {
                points.push((l as f64, y as f64));
                points.push(((r + 1) as f64, y as f64));
            }
        }
    }
    for (x, range) in cols.into_iter().enumerate() {
        if let Some((t, b)) = range {
            for x in [x, x + 1] {
                points.push((x as f64, t as f64));
                points.push((x as f64, (b + 1) as f64));
            }
        }
    }
    points
}

fn not_found(message: &str) -> ScannerError {
    ScannerError::new("cardNotFound", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    #[test]
    fn detects_outer_card_and_refuses_two_separate_cards() {
        let mut image = RgbaImage::from_pixel(600, 600, Rgba([0, 0, 0, 255]));
        for y in 100..400 {
            for x in 40..250 {
                image.put_pixel(x, y, Rgba([240, 240, 200, 255]));
            }
        }
        let boundary = foreground(&image, ScanBackground::Black, false).unwrap();
        assert!(!boundary.touches_edge);
        let crop = deskew::bounds(&boundary.points, 600, 600, 0.);
        assert!((crop.width - 212. / 600.).abs() < 1e-9);
        for y in 100..400 {
            for x in 330..540 {
                image.put_pixel(x, y, Rgba([240, 240, 200, 255]));
            }
        }
        assert!(foreground(&image, ScanBackground::Black, false).is_err());
    }
    #[test]
    fn follows_grey_to_white_backing_transition() {
        let mut image = RgbaImage::from_fn(600, 600, |_, y| {
            if y < 300 {
                Rgba([190, 190, 190, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        for y in 150..450 {
            for x in 200..410 {
                image.put_pixel(x, y, Rgba([90, 100, 120, 255]));
            }
        }
        let boundary = foreground(&image, ScanBackground::White, false).unwrap();
        assert!(!boundary.touches_edge);
        assert!(deskew::bounds(&boundary.points, 600, 600, 0.).height < 0.51);
    }
}
