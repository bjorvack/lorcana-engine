//! Deterministic, seeded random number generation.
//!
//! The RNG is part of the serialized game state (not an infrastructure detail):
//! storing the generator alongside the rest of the state is what makes replays
//! exact. We use `ChaCha8Rng` specifically because it is a fixed, named
//! algorithm whose output is stable across crate versions — `StdRng` makes no
//! such guarantee and could silently change between releases.

use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// A deterministic random number generator carried inside the game state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeededRng {
    inner: ChaCha8Rng,
}

impl SeededRng {
    /// Create a generator from a 64-bit seed.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Shuffle a slice in place deterministically, advancing the generator.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        slice.shuffle(&mut self.inner);
    }
}

#[cfg(test)]
mod tests {
    use super::SeededRng;

    #[test]
    fn same_seed_shuffles_identically() {
        let mut a = SeededRng::from_seed(42);
        let mut b = SeededRng::from_seed(42);

        let mut xs: Vec<u32> = (0..50).collect();
        let mut ys = xs.clone();
        a.shuffle(&mut xs);
        b.shuffle(&mut ys);

        assert_eq!(xs, ys);
        // Both generators advanced by the same amount.
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = SeededRng::from_seed(1);
        let mut b = SeededRng::from_seed(2);

        let mut xs: Vec<u32> = (0..50).collect();
        let mut ys = xs.clone();
        a.shuffle(&mut xs);
        b.shuffle(&mut ys);

        assert_ne!(xs, ys);
    }
}
