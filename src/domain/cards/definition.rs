//! Card definition types.
//!
//! A `CardDefinition` is the static, printed data for a card (reference data),
//! as opposed to a [`CardInstance`] which is a specific copy in a game. Only the
//! fields needed so far are modeled; this grows as later slices need name,
//! classifications, more ability kinds, etc.
//!
//! [`CardInstance`]: crate::domain::game::CardInstance

use super::ability::{ActivatedAbility, GameRuleStatic, StaticAbility, TriggeredAbility};
use super::card_kind::CardKind;
use super::keyword::{Keyword, ShiftAbility};
use crate::domain::types::card::{CardType, Classification};
use crate::domain::types::ids::CardDefId;
use serde::{Deserialize, Serialize};

/// The printed data for a card.
///
/// Not `Copy`: it owns a `Vec` of abilities. Look it up by reference via
/// [`CardRegistry::get`](super::registry::CardRegistry::get).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDefinition {
    id: CardDefId,
    /// Ink cost to play the card (§6.2.7).
    cost: u32,
    /// Whether the card has the inkwell symbol and so may be put into the
    /// inkwell (§4.3.3.1, §6.2.8).
    inkwell: bool,
    /// Type-specific characteristics.
    kind: CardKind,
    /// The card's triggered abilities (§7.4).
    abilities: Vec<TriggeredAbility>,
    /// The card's activated abilities (§7.5).
    activated: Vec<ActivatedAbility>,
    /// The card's classifications (§6.2.6), e.g. Hero, Villain, Princess.
    classifications: Vec<Classification>,
    /// The card's static abilities (§7.6).
    static_abilities: Vec<StaticAbility>,
    /// The card's game-rule static abilities (e.g. win-condition overrides).
    rule_statics: Vec<GameRuleStatic>,
    /// The card's keyword abilities (§10).
    keywords: Vec<Keyword>,
    /// The names this card counts as (§6.2.1). Usually one; some count as several
    /// (e.g. Flotsam & Jetsam counts as `Flotsam` and `Jetsam`; Chip 'n' Dale as
    /// `Chip` and `Dale`). Used by Shift's same-name rule and "named X" effects.
    names: Vec<String>,
}

impl CardDefinition {
    /// Create a card definition with no abilities.
    #[must_use]
    pub const fn new(id: CardDefId, cost: u32, inkwell: bool, kind: CardKind) -> Self {
        Self {
            id,
            cost,
            inkwell,
            kind,
            abilities: Vec::new(),
            activated: Vec::new(),
            classifications: Vec::new(),
            static_abilities: Vec::new(),
            rule_statics: Vec::new(),
            keywords: Vec::new(),
            names: Vec::new(),
        }
    }

    /// Convenience constructor for a character card with no abilities.
    #[must_use]
    pub const fn character(
        id: CardDefId,
        cost: u32,
        inkwell: bool,
        strength: u32,
        willpower: u32,
        lore: u32,
    ) -> Self {
        Self::new(
            id,
            cost,
            inkwell,
            CardKind::Character {
                strength,
                willpower,
                lore,
            },
        )
    }

    /// Replace this definition's triggered abilities (builder style).
    #[must_use]
    pub fn with_abilities(mut self, abilities: Vec<TriggeredAbility>) -> Self {
        self.abilities = abilities;
        self
    }

    /// Replace this definition's activated abilities (builder style).
    #[must_use]
    pub fn with_activated(mut self, activated: Vec<ActivatedAbility>) -> Self {
        self.activated = activated;
        self
    }

    /// Replace this definition's classifications (builder style).
    #[must_use]
    pub fn with_classifications(mut self, classifications: Vec<Classification>) -> Self {
        self.classifications = classifications;
        self
    }

    /// Replace this definition's static abilities (builder style).
    #[must_use]
    pub fn with_static(mut self, static_abilities: Vec<StaticAbility>) -> Self {
        self.static_abilities = static_abilities;
        self
    }

    /// Replace this definition's game-rule static abilities (builder style).
    #[must_use]
    pub fn with_rule_statics(mut self, rule_statics: Vec<GameRuleStatic>) -> Self {
        self.rule_statics = rule_statics;
        self
    }

    /// Replace this definition's keyword abilities (builder style).
    #[must_use]
    pub fn with_keywords(mut self, keywords: Vec<Keyword>) -> Self {
        self.keywords = keywords;
        self
    }

    /// Set the names this card counts as (builder style).
    #[must_use]
    pub fn with_names(mut self, names: Vec<String>) -> Self {
        self.names = names;
        self
    }

    /// The printed-card id.
    #[must_use]
    pub const fn id(&self) -> CardDefId {
        self.id
    }

    /// The ink cost to play this card.
    #[must_use]
    pub const fn cost(&self) -> u32 {
        self.cost
    }

    /// Whether this card has the inkwell symbol.
    #[must_use]
    pub const fn has_inkwell_symbol(&self) -> bool {
        self.inkwell
    }

    /// The type-specific characteristics.
    #[must_use]
    pub const fn kind(&self) -> CardKind {
        self.kind
    }

    /// The card-type tag.
    #[must_use]
    pub const fn card_type(&self) -> CardType {
        self.kind.card_type()
    }

    /// This card's triggered abilities.
    #[must_use]
    pub fn abilities(&self) -> &[TriggeredAbility] {
        &self.abilities
    }

    /// This card's activated abilities.
    #[must_use]
    pub fn activated_abilities(&self) -> &[ActivatedAbility] {
        &self.activated
    }

    /// This card's classifications (§6.2.6).
    #[must_use]
    pub fn classifications(&self) -> &[Classification] {
        &self.classifications
    }

    /// Whether this card has the given classification.
    #[must_use]
    pub fn has_classification(&self, classification: &Classification) -> bool {
        self.classifications.contains(classification)
    }

    /// This card's static abilities (§7.6).
    #[must_use]
    pub fn static_abilities(&self) -> &[StaticAbility] {
        &self.static_abilities
    }

    /// This card's game-rule static abilities.
    #[must_use]
    pub fn rule_statics(&self) -> &[GameRuleStatic] {
        &self.rule_statics
    }

    /// This card's keyword abilities (§10).
    #[must_use]
    pub fn keywords(&self) -> &[Keyword] {
        &self.keywords
    }

    /// Whether this card has the given keyword, e.g. `&Keyword::Evasive`.
    #[must_use]
    pub fn has_keyword(&self, keyword: &Keyword) -> bool {
        self.keywords.contains(keyword)
    }

    /// Total Resist `+N` (damage reduction) on this card (§10.8, stacks).
    #[must_use]
    pub fn resist(&self) -> u32 {
        self.keywords
            .iter()
            .filter_map(|k| match k {
                Keyword::Resist(n) => Some(*n),
                _ => None,
            })
            .sum()
    }

    /// Total Challenger `+N` (`{S}` while challenging) on this card (§10.5, stacks).
    #[must_use]
    pub fn challenger_bonus(&self) -> u32 {
        self.keywords
            .iter()
            .filter_map(|k| match k {
                Keyword::Challenger(n) => Some(*n),
                _ => None,
            })
            .sum()
    }

    /// The names this card counts as (§6.2.1).
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether this card counts as the given name.
    #[must_use]
    pub fn has_name(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }

    /// This card's Shift ability (cost + target restriction), if any (§10.10).
    #[must_use]
    pub fn shift(&self) -> Option<&ShiftAbility> {
        self.keywords.iter().find_map(|k| match k {
            Keyword::Shift(ability) => Some(ability),
            _ => None,
        })
    }

    /// This card's Boost cost in ink, if it has Boost (§10.4).
    #[must_use]
    pub fn boost(&self) -> Option<u32> {
        self.keywords.iter().find_map(|k| match k {
            Keyword::Boost(cost) => Some(*cost),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CardDefinition;
    use crate::domain::effects::{Effect, TriggerCondition};
    use crate::domain::types::ids::CardDefId;

    #[test]
    fn new_card_has_no_abilities() {
        let def = CardDefinition::character(CardDefId::from_raw(1), 3, true, 2, 2, 1);
        assert!(def.abilities().is_empty());
        assert!(def.activated_abilities().is_empty());
        assert!(def.classifications().is_empty());
        assert_eq!(def.cost(), 3);
        assert!(def.has_inkwell_symbol());
    }

    #[test]
    fn classifications_round_trip_and_query() {
        use crate::domain::types::card::Classification;
        let def = CardDefinition::character(CardDefId::from_raw(3), 4, true, 3, 4, 2)
            .with_classifications(vec!["Villain".into(), "Sorcerer".into()]);
        assert_eq!(def.classifications().len(), 2);
        assert!(def.has_classification(&Classification::new("Villain")));
        assert!(!def.has_classification(&Classification::new("Hero")));
    }

    #[test]
    fn with_abilities_attaches_triggers() {
        use crate::domain::cards::TriggeredAbility;
        let def = CardDefinition::character(CardDefId::from_raw(2), 1, true, 1, 1, 1)
            .with_abilities(vec![
                TriggeredAbility::new(TriggerCondition::WhenYouPlayThis, Effect::DrawCards(1)),
                TriggeredAbility::optional(
                    TriggerCondition::WhenThisQuests,
                    Effect::EachOpponentLosesLore(1),
                ),
            ]);

        assert_eq!(def.abilities().len(), 2);
        assert_eq!(
            def.abilities()[0].condition,
            TriggerCondition::WhenYouPlayThis
        );
        assert!(!def.abilities()[0].optional);
        assert!(def.abilities()[1].optional);
    }
}
