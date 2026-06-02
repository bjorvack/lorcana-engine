//! Card classifications (§6.2.6).

use serde::{Deserialize, Serialize};

/// A card classification such as `Hero`, `Villain`, `Princess`, or `Floodborn`
/// (§6.2.6).
///
/// Classifications are an **open vocabulary** defined by the cards themselves
/// (42 distinct across the current pool and growing per set), not by the rules —
/// so this is a newtype over a string rather than a closed enum. Comparison is
/// exact/case-sensitive; data is expected to use the canonical spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Classification(String);

impl Classification {
    /// Create a classification from any string-like value.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The classification name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Classification {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}
