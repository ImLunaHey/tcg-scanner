#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Searching,
    Moving,
    Waiting,
    Unchanged,
    Ready,
}
#[derive(Default)]
pub struct SteadyCard {
    previous: Option<Vec<u8>>,
    submitted: Option<Vec<u8>>,
    stable: u32,
    last_attempt: Option<f64>,
    pub matched: bool,
}
impl SteadyCard {
    pub fn observe(&mut self, frame: &[u8], now: f64) -> Observation {
        if frame.is_empty() {
            return Observation::Searching;
        }
        let mean = frame.iter().map(|&v| f64::from(v)).sum::<f64>() / frame.len() as f64;
        let variance = frame
            .iter()
            .map(|&v| (f64::from(v) - mean).powi(2))
            .sum::<f64>()
            / frame.len() as f64;
        self.stable = if self
            .previous
            .as_ref()
            .is_some_and(|p| difference(frame, p) < 5.)
        {
            self.stable.saturating_add(1)
        } else {
            0
        };
        self.previous = Some(frame.to_vec());
        if variance < 180. {
            Observation::Searching
        } else if self.matched
            && self
                .submitted
                .as_ref()
                .is_some_and(|p| difference(frame, p) < 12.)
        {
            Observation::Unchanged
        } else if self.stable < 2 {
            Observation::Moving
        } else if self
            .last_attempt
            .is_some_and(|last| now - last < if self.matched { 1200. } else { 2500. })
        {
            Observation::Waiting
        } else {
            Observation::Ready
        }
    }
    pub fn accept(&mut self, frame: Vec<u8>, now: f64) {
        self.submitted = Some(frame);
        self.last_attempt = Some(now);
        self.matched = false;
    }
    pub fn retry_after(&mut self, now: f64, delay: f64) {
        self.last_attempt = Some(now + delay - 2500.);
        self.matched = false;
    }
}
fn difference(a: &[u8], b: &[u8]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }
    a.iter()
        .zip(b)
        .map(|(&a, &b)| f64::from(a.abs_diff(b)))
        .sum::<f64>()
        / a.len() as f64
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steady_frames_are_throttled_and_matched_cards_need_a_new_view() {
        let mut gate = SteadyCard::default();
        let card: Vec<_> = (0..128)
            .map(|n| if n % 2 == 0 { 10 } else { 240 })
            .collect();
        assert_eq!(gate.observe(&[100; 128], 0.), Observation::Searching);
        assert_eq!(gate.observe(&card, 200.), Observation::Moving);
        assert_eq!(gate.observe(&card, 400.), Observation::Moving);
        assert_eq!(gate.observe(&card, 600.), Observation::Ready);
        gate.accept(card.clone(), 600.);
        assert_eq!(gate.observe(&card, 800.), Observation::Waiting);
        assert_eq!(gate.observe(&card, 3100.), Observation::Ready);
        gate.matched = true;
        assert_eq!(gate.observe(&card, 5000.), Observation::Unchanged);
        let next: Vec<_> = card.iter().map(|v| 255 - v).collect();
        assert_eq!(gate.observe(&next, 5200.), Observation::Moving);
        assert_eq!(gate.observe(&next, 5400.), Observation::Moving);
        assert_eq!(gate.observe(&next, 5600.), Observation::Ready);
        gate.retry_after(5600., 60000.);
        assert_eq!(gate.observe(&next, 65000.), Observation::Waiting);
        assert_eq!(gate.observe(&next, 65600.), Observation::Ready);
    }
}
