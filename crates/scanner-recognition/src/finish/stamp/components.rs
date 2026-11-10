use image::RgbImage;
use scanner_types::images::ImageCrop;

pub(super) struct Component {
    pub pixels: Vec<usize>,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
impl Component {
    pub fn region(&self, image: &RgbImage) -> ImageCrop {
        ImageCrop {
            x: self.x as f64 / image.width() as f64,
            y: self.y as f64 / image.height() as f64,
            width: self.width as f64 / image.width() as f64,
            height: self.height as f64 / image.height() as f64,
        }
    }
}
/// Consume each bounded component immediately; noisy artwork cannot accumulate
/// an unbounded collection of tiny masks. Images are already limited to 1260².
pub(super) fn visit(
    image: &RgbImage,
    region: [f64; 4],
    diagonals: bool,
    ink: impl Fn(&image::Rgb<u8>) -> bool,
    mut consume: impl FnMut(Component),
) {
    let w = image.width() as usize;
    let h = image.height() as usize;
    let [left, top, right, bottom] = region;
    let (x0, y0, x1, y1) = (
        (left * w as f64) as usize,
        (top * h as f64) as usize,
        (right * w as f64) as usize,
        (bottom * h as f64) as usize,
    );
    let mut seen = vec![false; w * h];
    for y in y0..y1 {
        for x in x0..x1 {
            let first = y * w + x;
            if seen[first] || !ink(image.get_pixel(x as u32, y as u32)) {
                continue;
            }
            seen[first] = true;
            let mut component = Component {
                pixels: vec![first],
                x,
                y,
                width: 1,
                height: 1,
            };
            let (mut max_x, mut max_y, mut head) = (x, y, 0);
            while head < component.pixels.len() {
                let p = component.pixels[head];
                head += 1;
                let (px, py) = (p % w, p / w);
                component.x = component.x.min(px);
                component.y = component.y.min(py);
                max_x = max_x.max(px);
                max_y = max_y.max(py);
                for dy in -1isize..=1 {
                    for dx in -1isize..=1 {
                        if (dx == 0 && dy == 0) || (!diagonals && dx != 0 && dy != 0) {
                            continue;
                        }
                        let (nx, ny) = (px as isize + dx, py as isize + dy);
                        if nx < x0 as isize
                            || nx >= x1 as isize
                            || ny < y0 as isize
                            || ny >= y1 as isize
                        {
                            continue;
                        }
                        let next = ny as usize * w + nx as usize;
                        if !seen[next] && ink(image.get_pixel(nx as u32, ny as u32)) {
                            seen[next] = true;
                            component.pixels.push(next);
                        }
                    }
                }
            }
            component.width = max_x - component.x + 1;
            component.height = max_y - component.y + 1;
            consume(component);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connectivity_and_roi_are_explicit() {
        let mut pixels = RgbImage::new(8, 8);
        for (x, y) in [(2, 2), (3, 3), (7, 7)] {
            pixels.put_pixel(x, y, image::Rgb([255; 3]));
        }
        for (diagonals, expected) in [(false, 2), (true, 1)] {
            let mut sizes = vec![];
            visit(
                &pixels,
                [0., 0., 0.75, 0.75],
                diagonals,
                |p| p[0] > 0,
                |c| sizes.push(c.pixels.len()),
            );
            assert_eq!(sizes.len(), expected);
            assert_eq!(sizes.iter().sum::<usize>(), 2);
        }
    }
}
