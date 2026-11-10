use image::GrayImage;
use imageproc::{
    contours::find_contours,
    distance_transform::Norm,
    edges::canny,
    geometry::{approximate_polygon_dp, arc_length, contour_area, convex_hull},
    morphology::dilate,
    point::Point,
};
type Quad = [(f32, f32); 4];

pub(super) fn find(image: &GrayImage) -> Option<Quad> {
    let edges = dilate(&canny(image, 25., 75.), Norm::LInf, 1);
    // An edge ring has inner and outer contours. Background texture can join
    // the outer contour while the inner contour still bounds the whole card.
    find_contours::<i32>(&edges)
        .into_iter()
        .filter_map(|c| candidate(&c.points, image.width(), image.height()))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|c| c.0)
}
fn candidate(contour: &[Point<i32>], width: u32, height: u32) -> Option<(Quad, f64)> {
    let area = contour_area(contour);
    if area < f64::from(width) * f64::from(height) * 0.35 {
        return None;
    }
    let mut hull = convex_hull(contour);
    if hull.len() < 4 {
        return None;
    }
    // Split the closed curve at distinct endpoints. DP's line distance is
    // undefined for an identical first/last point.
    let split = (1..hull.len()).max_by_key(|&i| {
        let dx = hull[i].x - hull[0].x;
        let dy = hull[i].y - hull[0].y;
        dx * dx + dy * dy
    })?;
    let epsilon = arc_length(&hull, true) * 0.018;
    hull.push(hull[0]);
    let mut polygon = approximate_polygon_dp(&hull[..=split], epsilon, false);
    polygon.pop();
    polygon.extend(approximate_polygon_dp(&hull[split..], epsilon, false));
    polygon.pop();
    simplify_seam(&mut polygon, epsilon);
    if polygon.len() != 4 || (contour_area(&polygon) - area).abs() > area * 0.10 {
        return None;
    }
    let mut points: Vec<_> = polygon
        .into_iter()
        .map(|p| (p.x as f32, p.y as f32))
        .collect();
    let centre = points
        .iter()
        .fold((0., 0.), |a, p| (a.0 + p.0 / 4., a.1 + p.1 / 4.));
    points.sort_by(|a, b| {
        (a.1 - centre.1)
            .atan2(a.0 - centre.0)
            .total_cmp(&(b.1 - centre.1).atan2(b.0 - centre.0))
    });
    let first = points
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1.0 + a.1.1).total_cmp(&(b.1.0 + b.1.1)))?
        .0;
    points.rotate_left(first);
    let quad: Quad = points.try_into().ok()?;
    valid(&quad, width, height).then_some((quad, area))
}
// Open-curve DP retains its two split endpoints, even when one lies on a
// straight edge of a rounded card. Remove only geometrically redundant points.
fn simplify_seam(points: &mut Vec<Point<i32>>, epsilon: f64) {
    while points.len() > 4 {
        let redundant = (0..points.len()).find(|&i| {
            let a = points[(i + points.len() - 1) % points.len()];
            let b = points[i];
            let c = points[(i + 1) % points.len()];
            let (dx, dy) = (f64::from(c.x - a.x), f64::from(c.y - a.y));
            let (px, py) = (f64::from(b.x - a.x), f64::from(b.y - a.y));
            let length_squared = dx * dx + dy * dy;
            let projection = px * dx + py * dy;
            length_squared > 0.
                && (0. ..=length_squared).contains(&projection)
                && (px * dy - py * dx).abs() <= epsilon * length_squared.sqrt()
        });
        let Some(index) = redundant else { break };
        points.remove(index);
    }
}
fn valid(q: &Quad, width: u32, height: u32) -> bool {
    let (w, h) = (width as f32, height as f32);
    if q.iter()
        .any(|p| p.0 <= w * 0.01 || p.1 <= h * 0.01 || p.0 >= w * 0.99 || p.1 >= h * 0.99)
    {
        return false;
    }
    let lengths: Vec<_> = (0..4)
        .map(|i| (q[i].0 - q[(i + 1) % 4].0).hypot(q[i].1 - q[(i + 1) % 4].1))
        .collect();
    let a = (lengths[0] + lengths[2]) / 2.;
    let b = (lengths[1] + lengths[3]) / 2.;
    if !(0.52..=0.88).contains(&(a.min(b) / a.max(b))) {
        return false;
    }
    (0..4).all(|i| {
        let prev = q[(i + 3) % 4];
        let p = q[i];
        let next = q[(i + 1) % 4];
        let cosine = ((prev.0 - p.0) * (next.0 - p.0) + (prev.1 - p.1) * (next.1 - p.1))
            / (lengths[(i + 3) % 4] * lengths[i]);
        cosine.abs() <= 25_f32.to_radians().sin()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};
    use imageproc::drawing::draw_polygon_mut;
    #[test]
    fn detects_rounded_card_on_textured_background() {
        let mut frame = GrayImage::from_fn(600, 800, |x, y| {
            Luma([40 + ((x * 73 + y * 137 + x * y * 19) % 100) as u8])
        });
        let points = [
            (80, 72),
            (480, 60),
            (510, 90),
            (520, 720),
            (500, 750),
            (70, 760),
            (50, 740),
            (55, 100),
        ];
        draw_polygon_mut(
            &mut frame,
            &points.map(|(x, y)| Point::new(x, y)),
            Luma([180]),
        );
        let quad = find(&frame).expect("rounded card must survive background texture");
        for (actual, expected) in
            quad.iter()
                .zip([(55., 72.), (510., 60.), (520., 750.), (50., 760.)])
        {
            assert!(
                (actual.0 - expected.0).hypot(actual.1 - expected.1) < 40.,
                "{quad:?}"
            );
        }
    }
    #[test]
    fn seam_cleanup_keeps_real_extra_corners() {
        let mut polygon = vec![
            Point::new(0, 0),
            Point::new(200, 0),
            Point::new(200, 400),
            Point::new(100, 500),
            Point::new(0, 400),
        ];
        simplify_seam(&mut polygon, 10.);
        assert_eq!(polygon.len(), 5);
    }
    #[test]
    fn rejects_partial_small_square_and_non_rectangular_objects() {
        for points in [
            [(0, 30), (400, 30), (400, 750), (0, 750)],
            [(250, 250), (390, 250), (390, 450), (250, 450)],
            [(70, 100), (530, 100), (530, 560), (70, 560)],
            [(70, 70), (550, 70), (100, 750), (70, 750)],
        ] {
            let mut frame = GrayImage::new(600, 800);
            draw_polygon_mut(
                &mut frame,
                &points.map(|(x, y)| Point::new(x, y)),
                Luma([240]),
            );
            assert!(find(&frame).is_none(), "{points:?}");
        }
    }
}
