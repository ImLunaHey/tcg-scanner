//! Small binary glyph operations; a closed counter distinguishes 0 from 5.
pub(super) fn similarity(mask: &[bool; 576], template: &[&[u8; 24]; 24]) -> f64 {
    let (mut intersection, mut union) = (0, 0);
    for (i, &ink) in mask.iter().enumerate() {
        let reference = template[i / 24][i % 24] == b'#';
        intersection += usize::from(ink && reference);
        union += usize::from(ink || reference);
    }
    intersection as f64 / union.max(1) as f64
}
fn neighbors(cell: usize) -> impl Iterator<Item = usize> {
    (-1isize..=1).flat_map(move |dy| {
        (-1isize..=1).filter_map(move |dx| {
            let (x, y) = ((cell % 24) as isize + dx, (cell / 24) as isize + dy);
            ((0..24).contains(&x) && (0..24).contains(&y)).then_some((y * 24 + x) as usize)
        })
    })
}
pub(super) fn has_counter(mask: &[bool; 576]) -> bool {
    let expanded: [bool; 576] = std::array::from_fn(|i| neighbors(i).any(|n| mask[n]));
    let closed: [bool; 576] = std::array::from_fn(|i| neighbors(i).all(|n| expanded[n]));
    let mut outside = [false; 576];
    let mut flood = vec![];
    for i in 0..576 {
        if (i / 24 == 0 || i / 24 == 23 || i % 24 == 0 || i % 24 == 23) && !closed[i] {
            outside[i] = true;
            flood.push(i);
        }
    }
    let mut head = 0;
    while head < flood.len() {
        let p = flood[head];
        head += 1;
        for next in neighbors(p).filter(|&n| n.abs_diff(p) == 24 || n / 24 == p / 24) {
            if !closed[next] && !outside[next] {
                outside[next] = true;
                flood.push(next);
            }
        }
    }
    let fraction = (0..576).filter(|&i| !closed[i] && !outside[i]).count() as f64 / 576.;
    (0.008..=0.35).contains(&fraction)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::finish::stamp::templates;
    #[test]
    fn zero_requires_a_counter_even_when_outline_is_similar() {
        for template in [templates::ZERO, templates::BLUE_ZERO] {
            let mask = std::array::from_fn(|i| template[i / 24][i % 24] == b'#');
            assert!(has_counter(&mask));
            assert_eq!(similarity(&mask, &template), 1.);
        }
        let solid = [true; 576];
        assert!(!has_counter(&solid));
        let open =
            std::array::from_fn(|i| (i / 24 < 4 || i / 24 > 19 || i % 24 < 4) && i % 24 < 21);
        assert!(!has_counter(&open));
    }
}
