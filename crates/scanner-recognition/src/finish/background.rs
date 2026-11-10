//! Measure broad clear patches separately from printed glyph pixels.
pub(super) struct Background {
    pub dark: f64,
    pub light: f64,
}
impl Background {
    pub fn polarity(&self) -> Option<&'static str> {
        if self.dark >= 0.05 && self.dark > self.light * 3. {
            Some("dark")
        } else if self.light >= 0.05 && self.light > self.dark * 3. {
            Some("light")
        } else {
            None
        }
    }
    pub fn measure(values: &[f64], width: usize, height: usize) -> Self {
        if width < 12 || height < 6 || width.checked_mul(height) != Some(values.len()) {
            return Self {
                dark: 0.,
                light: 0.,
            };
        }
        Self {
            dark: largest(values, width, height, true),
            light: largest(values, width, height, false),
        }
    }
}
fn largest(values: &[f64], width: usize, height: usize, dark: bool) -> f64 {
    let inset = 1.max(width / 20);
    let inner_width = width - inset * 2;
    let minimum_width = 4.max((inner_width as f64 * 0.18).ceil() as usize);
    let minimum_height = 3.max((height as f64 * 0.12).ceil() as usize);
    let mut heights = vec![0; inner_width + 1];
    let mut largest = 0;
    for y in 0..height {
        for x in 0..inner_width {
            let value = values[y * width + x + inset];
            let matches = if dark { value < 0.30 } else { value > 0.70 };
            heights[x] = if matches { heights[x] + 1 } else { 0 };
        }
        let mut stack: Vec<(usize, usize)> = vec![];
        for (x, &height) in heights.iter().enumerate() {
            let mut start = x;
            while let Some(&(left, previous)) = stack.last() {
                if previous <= height {
                    break;
                }
                stack.pop();
                let span = x - left;
                if span >= minimum_width && previous >= minimum_height {
                    largest = largest.max(span * previous);
                }
                start = left;
            }
            if stack.last().map(|s| s.1) != Some(height) {
                stack.push((start, height));
            }
        }
    }
    largest as f64 / (inner_width * height) as f64
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broad_backgrounds_are_independent_of_thin_letter_stems() {
        let mut values = vec![0.05; 100 * 20];
        for x in (24..76).step_by(6) {
            for y in 4..16 {
                values[y * 100 + x] = 0.95;
            }
        }
        let result = Background::measure(&values, 100, 20);
        assert_eq!(result.polarity(), Some("dark"));
        for value in &mut values {
            *value = 1. - *value;
        }
        assert_eq!(
            Background::measure(&values, 100, 20).polarity(),
            Some("light")
        );
        assert_eq!(Background::measure(&[0.; 4], 2, 2).polarity(), None);
    }
}
