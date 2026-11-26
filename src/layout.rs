pub const GAP: f32 = 12.0;

/// Packs indices into rows using each card's measured width.
/// Unknown widths (0.0) get a row of their own for the first frame.
pub fn rows(widths: &[f32], available: f32, gap: f32) -> Vec<Vec<usize>> {
    if widths.is_empty() {
        return Vec::new();
    }
    if widths.iter().any(|width| *width <= 0.0) {
        return (0..widths.len()).map(|index| vec![index]).collect();
    }

    let mut rows = Vec::new();
    let mut current = Vec::new();
    let mut used = 0.0f32;
    for (index, width) in widths.iter().enumerate() {
        let width = width.min(available);
        let needed = if current.is_empty() { width } else { gap + width };
        if !current.is_empty() && used + needed > available {
            rows.push(std::mem::take(&mut current));
            used = width;
        } else {
            used += needed;
        }
        current.push(index);
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_widths_get_their_own_row() {
        assert_eq!(rows(&[0.0, 0.0], 800.0, GAP), vec![vec![0], vec![1]]);
        assert!(rows(&[], 800.0, GAP).is_empty());
    }

    #[test]
    fn cards_share_a_row_when_they_fit() {
        assert_eq!(rows(&[400.0, 300.0], 800.0, GAP), vec![vec![0, 1]]);
    }

    #[test]
    fn cards_wrap_when_the_row_is_full() {
        assert_eq!(rows(&[500.0, 300.0], 800.0, GAP), vec![vec![0], vec![1]]);
        assert_eq!(
            rows(&[300.0, 300.0, 300.0], 800.0, GAP),
            vec![vec![0, 1], vec![2]]
        );
    }

    #[test]
    fn oversized_cards_still_get_one_row_each() {
        assert_eq!(rows(&[900.0, 900.0], 800.0, GAP), vec![vec![0], vec![1]]);
    }
}
