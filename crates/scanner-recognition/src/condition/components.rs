use image::RgbImage;
pub(super) fn light(pixel: &image::Rgb<u8>) -> bool {
    let [r, g, b] = pixel.0;
    r.min(g).min(b) > 145 && r.max(g).max(b) - r.min(g).min(b) < 55
}
pub(super) fn exterior(image: &RgbImage) -> Vec<bool> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut visited = vec![false; w * h];
    let mut queue = Vec::new();
    let add = |p: usize, visited: &mut Vec<bool>, queue: &mut Vec<usize>| {
        if !visited[p] && light(image.get_pixel((p % w) as u32, (p / w) as u32)) {
            visited[p] = true;
            queue.push(p);
        }
    };
    for x in 0..w {
        add(x, &mut visited, &mut queue);
        add((h - 1) * w + x, &mut visited, &mut queue);
    }
    for y in 0..h {
        add(y * w, &mut visited, &mut queue);
        add(y * w + w - 1, &mut visited, &mut queue);
    }
    let mut head = 0;
    while head < queue.len() {
        let p = queue[head];
        head += 1;
        let (x, y) = (p % w, p / w);
        if x > 0 {
            add(p - 1, &mut visited, &mut queue);
        }
        if x + 1 < w {
            add(p + 1, &mut visited, &mut queue);
        }
        if y > 0 {
            add(p - w, &mut visited, &mut queue);
        }
        if y + 1 < h {
            add(p + w, &mut visited, &mut queue);
        }
    }
    visited
}

pub(super) struct Component {
    pub area: usize,
    pub left: usize,
    pub right: usize,
    pub top: usize,
    pub bottom: usize,
}
pub(super) fn collect(mask: &mut [bool], start: usize, w: usize, h: usize) -> Component {
    let mut queue = vec![start];
    mask[start] = false;
    let mut result = Component {
        area: 0,
        left: w,
        right: 0,
        top: h,
        bottom: 0,
    };
    while result.area < queue.len() {
        let p = queue[result.area];
        result.area += 1;
        let (x, y) = (p % w, p / w);
        result.left = result.left.min(x);
        result.right = result.right.max(x);
        result.top = result.top.min(y);
        result.bottom = result.bottom.max(y);
        for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                let next = yy * w + xx;
                if mask[next] {
                    mask[next] = false;
                    queue.push(next);
                }
            }
        }
    }
    result
}
