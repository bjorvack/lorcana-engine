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
use crate::domain::effects::{Effect, TriggerCondition};
use crate::domain::types::card::{Classification, InkType};
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
    /// Ink type(s): one, or two for dual-ink (e.g. `ink = ["Ruby", "Sapphire"]`).
    #[serde(default)]
    pub ink: Vec<String>,
    /// Deck-building copy-limit override (§2.1.1.3); omit for the default 4.
    pub max_copies: Option<u32>,
    /// URL (or path) to the card's image, for display.
    pub image: Option<String>,
    /// Triggered abilities authored in the effect DSL (see [`super::dsl`]).
    #[serde(default)]
    pub abilities: Vec<super::dsl::TomlAbility>,
    /// Activated abilities (`{E}`/ink cost + effect).
    #[serde(default)]
    pub activated: Vec<super::dsl::TomlActivated>,
    /// Continuous static stat modifiers.
    #[serde(default)]
    pub statics: Vec<super::dsl::TomlStatic>,
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
        let bad = |detail: String| LoadError::BadAbility {
            name: self.name.clone(),
            detail,
        };
        let mut abilities: Vec<_> = self
            .abilities
            .iter()
            .map(|a| a.to_ability().map_err(&bad))
            .collect::<Result<_, _>>()?;
        // An action/song never enters play, so its "when you play this" abilities
        // are really its on-play **action effects** (§6.3.2), resolved directly.
        let mut action_effects = Vec::new();
        if matches!(kind, CardKind::Action) {
            abilities.retain(|a| {
                if a.condition == TriggerCondition::WhenYouPlayThis {
                    action_effects.push(if a.optional {
                        Effect::May(Box::new(a.effect.clone()))
                    } else {
                        a.effect.clone()
                    });
                    false
                } else {
                    true
                }
            });
        }
        let activated = self
            .activated
            .iter()
            .map(|a| a.to_ability().map_err(&bad))
            .collect::<Result<_, _>>()?;
        let statics = self
            .statics
            .iter()
            .map(|s| s.to_static().map_err(&bad))
            .collect::<Result<_, _>>()?;
        let ink_types = self
            .ink
            .iter()
            .map(|s| {
                ink_from(s).ok_or_else(|| LoadError::BadAbility {
                    name: self.name.clone(),
                    detail: format!("unknown ink type {s:?}"),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(CardDefinition::new(id, self.cost, self.inkwell, kind)
            .with_classifications(classifications)
            .with_names(vec![self.name.clone()])
            .with_keywords(keywords)
            .with_abilities(abilities)
            .with_activated(activated)
            .with_static(statics)
            .with_action_effects(action_effects)
            .with_ink_types(ink_types)
            .with_max_deck_copies(self.max_copies)
            .with_image(self.image.clone()))
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

/// Map an ink-type name to an [`InkType`].
fn ink_from(s: &str) -> Option<InkType> {
    Some(match s {
        "Amber" => InkType::Amber,
        "Amethyst" => InkType::Amethyst,
        "Emerald" => InkType::Emerald,
        "Ruby" => InkType::Ruby,
        "Sapphire" => InkType::Sapphire,
        "Steel" => InkType::Steel,
        _ => return None,
    })
}
