//! Fixed-capacity time series for sparklines.

use std::collections::VecDeque;

/// A ring buffer of the most recent samples.
#[derive(Debug, Clone)]
pub struct Series {
    values: VecDeque<u64>,
    capacity: usize,
}

impl Series {
    /// Creates an empty series holding at most `capacity` samples.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Appends a sample, evicting the oldest when full.
    pub fn push(&mut self, value: u64) {
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// The most recent sample, or 0.
    #[must_use]
    pub fn latest(&self) -> u64 {
        self.values.back().copied().unwrap_or_default()
    }

    /// Exactly `width` samples, right-aligned: newest last, zero-padded on the left.
    #[must_use]
    pub fn window(&self, width: usize) -> Vec<u64> {
        let skip = self.values.len().saturating_sub(width);
        let mut out = vec![0; width.saturating_sub(self.values.len())];
        out.extend(self.values.iter().skip(skip));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_and_windows() {
        let mut s = Series::new(3);
        (1..=5).for_each(|v| s.push(v));
        assert_eq!(s.latest(), 5);
        assert_eq!(s.window(3), [3, 4, 5]);
        assert_eq!(s.window(5), [0, 0, 3, 4, 5]);
        assert_eq!(s.window(2), [4, 5]);
    }
}
