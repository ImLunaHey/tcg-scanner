pub(super) struct Component {
    pub points: Vec<usize>,
    pub left: usize,
    pub top: usize,
    pub width: usize,
    pub height: usize,
}
/// Consume bounded components immediately. Bridge pixels may connect strokes,
/// but only original ink contributes to the returned component or shape score.
pub(super) fn visit(
    mask: &[bool],
    joined: &[bool],
    w: usize,
    h: usize,
    mut consume: impl FnMut(Component),
) {
    if w == 0
        || h == 0
        || w.checked_mul(h)
            .is_none_or(|n| n > 300_000 || n != mask.len() || n != joined.len())
    {
        return;
    }
    let mut seen = vec![false; mask.len()];
    for start in 0..joined.len() {
        if seen[start] || !joined[start] {
            continue;
        }
        seen[start] = true;
        let mut queue = vec![start];
        let mut points = vec![];
        let (mut left, mut top, mut right, mut bottom, mut head) = (w, h, 0, 0, 0);
        while head < queue.len() {
            let p = queue[head];
            head += 1;
            let (x, y) = (p % w, p / w);
            if mask[p] {
                points.push(p);
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
            for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let next = yy * w + xx;
                    if joined[next] && !seen[next] {
                        seen[next] = true;
                        queue.push(next);
                    }
                }
            }
        }
        if !points.is_empty() {
            consume(Component {
                points,
                left,
                top,
                width: right - left + 1,
                height: bottom - top + 1,
            });
        }
    }
}

/// Separable square dilation in linear time, used solely to group ink strokes.
pub(super) fn join(mask: &[bool], w: usize, h: usize, radius: usize) -> Vec<bool> {
    let mut horizontal = vec![false; mask.len()];
    let mut result = horizontal.clone();
    for y in 0..h {
        let mut prefix = vec![0usize; w + 1];
        for x in 0..w {
            prefix[x + 1] = prefix[x] + usize::from(mask[y * w + x]);
        }
        for x in 0..w {
            horizontal[y * w + x] =
                prefix[w.min(x + radius + 1)] > prefix[x.saturating_sub(radius)];
        }
    }
    for x in 0..w {
        let mut prefix = vec![0usize; h + 1];
        for y in 0..h {
            prefix[y + 1] = prefix[y] + usize::from(horizontal[y * w + x]);
        }
        for y in 0..h {
            result[y * w + x] = prefix[h.min(y + radius + 1)] > prefix[y.saturating_sub(radius)];
        }
    }
    result
}
