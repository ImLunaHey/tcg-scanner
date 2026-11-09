use scanner_types::images::ImageCrop;

pub type Point = (f64, f64);

#[derive(Clone, Copy)]
pub(crate) struct Bounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Bounds {
    pub fn width(self) -> f64 {
        self.right - self.left
    }
    pub fn height(self) -> f64 {
        self.bottom - self.top
    }
    fn area(self) -> f64 {
        self.width() * self.height()
    }
}

pub struct Geometry {
    pub crop: ImageCrop,
    pub degrees: f64,
    pub warnings: Vec<String>,
}

/// Minimum-area enclosure of the actual foreground, with parallel-edge refinement.
pub fn analyze(points: &[Point], width: u32, height: u32) -> Geometry {
    let initial = enclosure(points, 0.);
    let mut best = initial;
    let mut degrees: f64 = 0.;
    for step in -120..=120 {
        let angle = f64::from(step) / 10.;
        let rect = enclosure(points, angle.to_radians());
        if rect.area() < best.area() {
            best = rect;
            degrees = angle;
        }
    }
    let mut warnings = vec![];
    if degrees.abs() >= 11.9 {
        degrees = 0.;
        warnings.push("Card tilt exceeds the supported deskew range. Review manually.".into());
    } else if let Some(correction) = edge_correction(points, degrees)
        && correction.abs() <= 0.6
        && (degrees + correction).abs() < 11.9
    {
        let refined = degrees + correction;
        degrees = if refined.abs() >= 0.15 { refined } else { 0. };
    } else if degrees.abs() < 0.15 || best.area() > initial.area() * 0.999 {
        degrees = 0.;
    }
    Geometry {
        crop: bounds(points, width, height, degrees),
        degrees,
        warnings,
    }
}

pub fn bounds(points: &[Point], width: u32, height: u32, degrees: f64) -> ImageCrop {
    let canvas = enclosure(
        &[
            (0., 0.),
            (width as f64, 0.),
            (0., height as f64),
            (width as f64, height as f64),
        ],
        degrees.to_radians(),
    );
    let rect = enclosure(points, degrees.to_radians());
    let left = (rect.left - 1.).max(canvas.left);
    let top = (rect.top - 1.).max(canvas.top);
    let right = (rect.right + 1.).min(canvas.right);
    let bottom = (rect.bottom + 1.).min(canvas.bottom);
    ImageCrop {
        x: (left - canvas.left) / canvas.width(),
        y: (top - canvas.top) / canvas.height(),
        width: (right - left) / canvas.width(),
        height: (bottom - top) / canvas.height(),
    }
}

pub(crate) fn enclosure(points: &[Point], radians: f64) -> Bounds {
    let (s, c) = radians.sin_cos();
    let mut bounds = Bounds {
        left: f64::INFINITY,
        top: f64::INFINITY,
        right: f64::NEG_INFINITY,
        bottom: f64::NEG_INFINITY,
    };
    for &(x, y) in points {
        let (x, y) = (x * c - y * s, x * s + y * c);
        bounds.left = bounds.left.min(x);
        bounds.right = bounds.right.max(x);
        bounds.top = bounds.top.min(y);
        bounds.bottom = bounds.bottom.max(y);
    }
    bounds
}

fn edge_correction(points: &[Point], degrees: f64) -> Option<f64> {
    let (s, c) = degrees.to_radians().sin_cos();
    let mut aligned: Vec<_> = points
        .iter()
        .map(|&(x, y)| (x * c - y * s, x * s + y * c))
        .collect();
    let mut box_ = enclosure(&aligned, 0.);
    let landscape = box_.width() > box_.height();
    if landscape {
        aligned.iter_mut().for_each(|p| *p = (p.1, p.0));
        box_ = enclosure(&aligned, 0.);
    }
    let middle: Vec<_> = aligned
        .into_iter()
        .filter(|p| p.1 > box_.top + box_.height() * 0.1 && p.1 < box_.bottom - box_.height() * 0.1)
        .collect();
    let midpoint = (box_.left + box_.right) / 2.;
    let left = edge_slope(
        &middle
            .iter()
            .copied()
            .filter(|p| p.0 < midpoint)
            .collect::<Vec<_>>(),
        box_.height(),
    )?;
    let right = edge_slope(
        &middle
            .iter()
            .copied()
            .filter(|p| p.0 > midpoint)
            .collect::<Vec<_>>(),
        box_.height(),
    )?;
    if (left.atan() - right.atan()).to_degrees().abs() > 0.15 {
        return None;
    }
    Some(((left + right) / 2.).atan().to_degrees() * if landscape { -1. } else { 1. })
}

fn edge_slope(points: &[Point], span: f64) -> Option<f64> {
    if points.len() < 24 || span < 80. {
        return None;
    }
    let (slope, intercept) = fit(points)?;
    let mut residuals: Vec<_> = points
        .iter()
        .map(|p| (p.0 - slope * p.1 - intercept).abs())
        .collect();
    residuals.sort_by(f64::total_cmp);
    let tolerance = (residuals[residuals.len() / 2] * 3.).max(0.75);
    let inliers: Vec<_> = points
        .iter()
        .copied()
        .filter(|p| (p.0 - slope * p.1 - intercept).abs() <= tolerance)
        .collect();
    let box_ = enclosure(&inliers, 0.);
    if inliers.len() < points.len() * 4 / 5 || box_.height() < span * 0.5 {
        return None;
    }
    let (slope, intercept) = fit(&inliers)?;
    let rms = (inliers
        .iter()
        .map(|p| (p.0 - slope * p.1 - intercept).powi(2))
        .sum::<f64>()
        / inliers.len() as f64)
        .sqrt();
    (rms <= 0.65).then_some(slope)
}

fn fit(points: &[Point]) -> Option<(f64, f64)> {
    let mean_x = points.iter().map(|p| p.0).sum::<f64>() / points.len() as f64;
    let mean_y = points.iter().map(|p| p.1).sum::<f64>() / points.len() as f64;
    let variance = points.iter().map(|p| (p.1 - mean_y).powi(2)).sum::<f64>();
    if variance <= 0. {
        return None;
    }
    let slope = points
        .iter()
        .map(|p| (p.0 - mean_x) * (p.1 - mean_y))
        .sum::<f64>()
        / variance;
    Some((slope, mean_x - slope * mean_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counteracts_both_signs_of_card_tilt() {
        for tilt in [-4.5_f64, 0., 3.2] {
            let (s, c) = tilt.to_radians().sin_cos();
            let points: Vec<_> = (0..400)
                .flat_map(|y| [(100., y as f64), (380., y as f64)])
                .map(|(x, y)| (x * c - y * s + 200., x * s + y * c + 100.))
                .collect();
            let geometry = analyze(&points, 1000, 1000);
            assert!(
                (geometry.degrees + tilt).abs() < 0.11,
                "{tilt}: {}",
                geometry.degrees
            );
            assert!((geometry.crop.width / geometry.crop.height - 0.7).abs() < 0.02);
        }
    }
}
