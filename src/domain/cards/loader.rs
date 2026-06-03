//! Load card definitions from our own **TOML** card format into
//! [`CardDefinition`]s.
//!
//! This is the engine's canonical, committed card data format (authored by us —
//! external datasets like Lorcast are only research aids, never loaded directly).
//! A file is a list of `[[card]]` tables:
//!
//! ```toml
//! [[card]]
//! name = "Genie"
//! type = "Character"           # Character | Action | Song | Item | Location
//! cost = 5
//! inkwell = true
//! strength = 4                 # characters
//! willpower = 5                # characters / locations
//! lore = 2                     # characters / locations
//! move_cost = 1                # locations
//! classifications = ["Floodborn", "Ally"]
//! keywords = ["Evasive", "Challenger 2"]   # value (if any) is inline
//! ```
//!
//! Only the *structured* characteristics + keywords are loaded here; a card's
//! text-based triggered / activated / static abilities are authored via the
//! effect DSL (a separate concern).

use super::{CardDefinition, CardKind, Keyword, ShiftAbility};
use crate::domain::types::card::Classification;
use crate::domain::types::ids::CardDefId;
use serde::Deserialize;

/// A file of card definitions: `[[card]]` tables.
#[derive(Debug, Clone, Deserialize)]
struct CardFile {
    #[serde(default)]
    card: Vec<TomlCard>,
}

/// One `[[card]]` table in the TOML format (the fields the loader maps).
#[derive(Debug, Clone, Deserialize)]
pub struct TomlCard {
    /// The card's name.
    pub name: String,
    /// `Character` | `Action` | `Song` | `Item` | `Location`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Ink cost.
    pub cost: u32,
    /// Whether it has the inkwell symbol.
    #[serde(default)]
    pub inkwell: bool,
    /// Printed `{S}` (characters).
    pub strength: Option<u32>,
    /// Printed `{W}` (characters / locations).
    pub willpower: Option<u32>,
    /// Printed `{L}` (characters / locations).
    pub lore: Option<u32>,
    /// Move cost (locations).
    pub move_cost: Option<u32>,
    /// Classifications (Hero / Villain / Princess / …).
    #[serde(default)]
    pub classifications: Vec<String>,
    /// Keyword abilities, value inline where applicable ("Challenger 2", "Shift 5").
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Triggered abilities authored in the effect DSL (see [`super::dsl`]).
    #[serde(default)]
    pub abilities: Vec<super::dsl::TomlAbility>,
}

/// Why a card couldn't be loaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    /// The TOML didn't parse.
    #[error("invalid card TOML: {0}")]
    Toml(String),
    /// The `type` wasn't recognised.
    #[error("card {name:?}: unrecognised type {kind:?}")]
    UnknownType {
        /// The card name.
        name: String,
        /// The offending type.
        kind: String,
    },
    /// A character / location was missing a required stat.
    #[error("card {name:?}: missing stat {stat}")]
    MissingStat {
        /// The card name.
        name: String,
        /// Which stat was absent.
        stat: &'static str,
    },
    /// A keyword we don't model, or a valued keyword missing its value.
    #[error("card {name:?}: keyword {keyword:?} could not be loaded")]
    BadKeyword {
        /// The card name.
        name: String,
        /// The offending keyword string.
        keyword: String,
    },
    /// An ability couldn't be parsed from the effect DSL.
    #[error("card {name:?}: {detail}")]
    BadAbility {
        /// The card name.
        name: String,
        /// What went wrong.
        detail: String,
    },
}

impl TomlCard {
    /// Map this card to a [`CardDefinition`] with the given id.
    ///
    /// # Errors
    /// Returns [`LoadError`] if the type / stats / keywords can't be mapped.
    pub fn to_definition(&self, id: CardDefId) -> Result<CardDefinition, LoadError> {
        let kind = self.card_kind()?;
        let classifications = self
            .classifications
            .iter()
            .map(Classification::new)
            .collect();
        let keywords = self
            .keywords
            .iter()
            .map(|kw| {
                keyword_from(kw).ok_or_else(|| LoadError::BadKeyword {
                    name: self.name.clone(),
                    keyword: kw.clone(),
                })
            })
            .collect::<Result<_, _>>()?;
        let abilities = self
            .abilities
            .iter()
            .map(|a| {
                a.to_ability().map_err(|detail| LoadError::BadAbility {
                    name: self.name.clone(),
                    detail,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(CardDefinition::new(id, self.cost, self.inkwell, kind)
            .with_classifications(classifications)
            .with_names(vec![self.name.clone()])
            .with_keywords(keywords)
            .with_abilities(abilities))
    }

    fn stat(&self, value: Option<u32>, stat: &'static str) -> Result<u32, LoadError> {
        value.ok_or_else(|| LoadError::MissingStat {
            name: self.name.clone(),
            stat,
        })
    }

    fn card_kind(&self) -> Result<CardKind, LoadError> {
        match self.kind.as_str() {
            "Character" => Ok(CardKind::Character {
                strength: self.stat(self.strength, "strength")?,
                willpower: self.stat(self.willpower, "willpower")?,
                lore: self.stat(self.lore, "lore")?,
            }),
            "Location" => Ok(CardKind::Location {
                move_cost: self.stat(self.move_cost, "move_cost")?,
                willpower: self.stat(self.willpower, "willpower")?,
                lore: self.stat(self.lore, "lore")?,
            }),
            "Item" => Ok(CardKind::Item),
            // A Song is an action (with the Song classification); both are actions.
            "Action" | "Song" => Ok(CardKind::Action),
            _ => Err(LoadError::UnknownType {
                name: self.name.clone(),
                kind: self.kind.clone(),
            }),
        }
    }
}

/// Map a keyword string (name + optional inline value) to a [`Keyword`].
pub(crate) fn keyword_from(s: &str) -> Option<Keyword> {
    let mut parts = s.split_whitespace();
    let name = parts.next()?;
    let rest = s[name.len()..].trim();
    let value = || rest.trim_start_matches('+').parse::<u32>().ok();
    Some(match name {
        "Evasive" => Keyword::Evasive,
        "Bodyguard" => Keyword::Bodyguard,
        "Rush" => Keyword::Rush,
        "Alert" => Keyword::Alert,
        "Ward" => Keyword::Ward,
        "Reckless" => Keyword::Reckless,
        "Vanish" => Keyword::Vanish,
        "Support" => Keyword::Support,
        "Challenger" => Keyword::Challenger(value()?),
        "Resist" => Keyword::Resist(value()?),
        "Singer" => Keyword::Singer(value()?),
        "Boost" => Keyword::Boost(value()?),
        "Shift" => Keyword::Shift(ShiftAbility::ink_same_name(value()?)),
        // "Sing Together N" — two words before the value.
        "Sing" if rest.starts_with("Together") => {
            Keyword::SingTogether(rest["Together".len()..].trim().parse().ok()?)
        }
        _ => return None,
    })
}

/// Load all card definitions from a TOML document, assigning each a sequential
/// [`CardDefId`] (its index among the file's cards).
///
/// # Errors
/// Returns [`LoadError`] on invalid TOML or any card that can't be mapped.
pub fn load_toml(toml_str: &str) -> Result<Vec<CardDefinition>, LoadError> {
    let file: CardFile = toml::from_str(toml_str).map_err(|e| LoadError::Toml(e.to_string()))?;
    file.card
        .iter()
        .enumerate()
        .map(|(i, c)| c.to_definition(CardDefId::from_raw(u32::try_from(i).unwrap_or(u32::MAX))))
        .collect()
}
