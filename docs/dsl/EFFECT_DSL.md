# Effect DSL reference

GENERATED — do not edit by hand. Run `cargo run --bin dsl_reference` to regenerate from `src/domain/cards/dsl_reference.rs`. This is the authoritative syntax for authoring card abilities in the per-card files (`cards/<set>/<collector>.toml`); the grammar lives in `src/domain/cards/dsl.rs`.

Abilities are authored at the top level of a per-card file; `combine_sets.py` re-nests them under `card.` for the loader.

## Triggered abilities — `[[abilities]]`

`on` selects the trigger; `do` is one effect table or an array (resolved in order); `may = true` makes it "you may"; `during_your_turn` / `during_opponents_turn` gate when it fires.

```toml
[[abilities]]
on = "play"
do = [{ draw = 1 }, { gain_lore = 1 }]
```

### Triggers (`on`)

| Token | Meaning |
| --- | --- |
| `play` | When you play this card. |
| `play_this` | Alias of `play`. |
| `play_with_shift` | When you play this card using Shift. |
| `play_character` | Whenever you play a character. |
| `play_action` | Whenever you play an action. |
| `play_song` | Whenever you play a song. |
| `play_item` | Whenever you play an item. |
| `play_location` | Whenever you play a location. |
| `quest` | When this character quests. |
| `challenge` | When this character challenges. |
| `challenged` | When this character is challenged. |
| `banish` | When this character is banished. |
| `banished_in_challenge` | When banished in a challenge. |
| `banishes_in_challenge` | When this banishes another in a challenge. |
| `start_of_turn` | At the start of your turn. |
| `end_of_turn` | At the end of your turn. |
| `draw` | When you draw a card (`you_draw`). |
| `card_put_in_inkwell` | When a card is put into your inkwell. |

## Activated abilities — `[[activated]]`

A `cost` (`{ exert = true, ink = 1, banish = false }`) plus a `do` effect.

```toml
[[activated]]
cost = { exert = true, ink = 1 }
do = { draw = 1 }
```

## Static abilities — `[[statics]]`

A continuous stat modifier: one of `strength` / `willpower` / `lore` on a `to` selector, optionally scaled (`per = "<filter>"`) or gated (`while = "exerted"`).

```toml
[[statics]]
strength = 1
to = "your other Hero characters"
```

## §7.7 replacements

`[[redirect_damage]]` (`from = "<selector>"`) redirects matching damage onto this card; `[[prevent_damage]]` (`to = "<selector>"`, `"this"` for itself) prevents it.

## Effect verbs (`do`)

| Verb | Meaning | Example |
| --- | --- | --- |
| `draw` | Draw N cards (optional `who` scope, default you). | `{ draw = 2 }` |
| `gain_lore` | A player gains lore (default you). | `{ gain_lore = 1 }` |
| `lose_lore` | A player loses lore (default each opponent). | `{ lose_lore = 1, who = "each opponent" }` |
| `deal_damage` | Deal N damage to a target. | `{ deal_damage = 2, target = "chosen character" }` |
| `remove_damage` | Remove N damage from a target. | `{ remove_damage = 2, target = "chosen character" }` |
| `give_strength` | Change a target's {S} this turn (may be negative). | `{ give_strength = 2, target = "chosen character" }` |
| `give_willpower` | Change a target's {W} this turn. | `{ give_willpower = 1, target = "chosen character" }` |
| `give_lore` | Change a target's {L} this turn. | `{ give_lore = 1, target = "chosen character" }` |
| `banish` | Banish the target. | `{ banish = "chosen character" }` |
| `exert` | Exert the target. | `{ exert = "chosen opposing character" }` |
| `ready` | Ready the target. | `{ ready = "self" }` |
| `freeze` | Freeze the target (can't ready next turn). | `{ freeze = "chosen opposing character" }` |
| `boost` | Put the top N cards of your deck face-down under this character (§10.4). | `{ boost = 1 }` |
| `prevent_next_damage` | The next time the target would be dealt damage, it takes none (§7.7). | `{ prevent_next_damage = "chosen character" }` |
| `return_to_hand` | Return a target permanent to its owner's hand (§8). | `{ return_to_hand = "chosen character" }` |
| `return_from_discard` | Return a card matching a selector from your discard to hand. | `{ return_from_discard = "a character" }` |
| `into_inkwell` | Put a target into its owner's inkwell. | `{ into_inkwell = "chosen item" }` |
| `inkwell_from_hand` | Put a card matching a selector from your hand into your inkwell. | `{ inkwell_from_hand = "a card" }` |
| `discard` | A player discards N cards from hand (default you choose). | `{ discard = 1, who = "each opponent" }` |
| `play_free` | Play a card matching a selector from your hand for free (§6). | `{ play_free = "a character" }` |
| `search` | Search your deck for up to `take` cards matching the filter; shuffle. | `{ search = "a character", take = 1 }` |
| `look_at_top` | Look at the top N; take up to `take` matching to hand; `rest` = bottom|top|shuffle. | `{ look_at_top = 4, take = "a song", rest = "bottom" }` |
| `move_damage` | Move N damage counters `from` one target `to` another. | `{ move_damage = 2, from = "chosen character", to = "self" }` |
| `grant_keyword` | Grant a keyword to a target (`duration` = this_turn|next_turn|permanent). | `{ grant_keyword = "Evasive", target = "self", duration = "this_turn" }` |
| `restrict` | Grant a restriction (see Restrictions) to a target, for a `duration`. | `{ restrict = "cant_quest", target = "chosen opposing character" }` |
| `if_you_have` | Resolve `then` only if you control `at_least` (default 1) matching characters. | `{ if_you_have = "a Villain character", at_least = 2, then = { draw = 1 } }` |
| `if_count` | Resolve `then` only if a count condition holds (e.g. cards in hand). | `{ if_count = "more than 3 cards in your hand", then = { draw = 1 } }` |
| `choose_one` | Choose one of 2+ sub-effects to resolve (`optional` to allow none). | `{ choose_one = [ { draw = 1 }, { gain_lore = 1 } ] }` |
| `may_choose_one` | Optional `choose_one` (you may decline). | `{ may_choose_one = [ { draw = 1 }, { gain_lore = 1 } ] }` |
| `then_to` | Resolve `apply_to` once, then apply each listed effect to that target (§ `OnTarget`). | `{ apply_to = "chosen character", then_to = [ { exert = "self" }, { freeze = "self" } ] }` |

## Target selectors (`target` / `to` / `from`)

| Token | Meaning |
| --- | --- |
| `self / this` | The source card itself. |
| `chosen [opposing|your] [Classification] character` | A chosen character, optionally filtered by side/classification. |
| `another chosen character` | Excludes the source (`Not(IsSource)`). |
| `all your/opposing characters` | Every matching character (no choice). |
| `chosen item / chosen location` | A chosen permanent of that kind. |
| `the challenging / challenged character` | The other combatant bound by a challenge trigger. |
| `named "X"` | Predicate: card name contains X. |
| `with cost N or less/more` | Numeric threshold on cost / {S} / {W} / {L}. |
| `a/an … card` | A printed-card filter (e.g. for play_free / search / return_from_discard). |

A leaf may also be the structured AST form (a TOML table) when the compact grammar can't express it.

## Amounts (any numeric field)

| Token | Meaning |
| --- | --- |
| `<integer>` | A fixed amount. |
| `"per <filter>"` | One for each matching character (e.g. "per your Villain character"). |
| `"cards in hand"` | Your current hand size. |
| `"damage on self"` | Damage counters on the source. |
| `"<strength|willpower|lore>"` | A stat of the source. |
| `"that much" / "damage dealt"` | The amount the trigger carries. |

## Player scopes (`who` / `whose`)

| Token | Meaning |
| --- | --- |
| `you / yourself` | The controller. |
| `each opponent / opponents` | Every opponent. |
| `each player / all players` | Everyone. |
| `chosen opponent` | One opponent (auto-resolved in 2-player). |
| `chosen player` | One player. |

## Durations (`duration`)

| Token | Meaning |
| --- | --- |
| `this_turn` | Until end of turn (default). |
| `next_turn` | Until the end of your next turn. |
| `permanent` | While the source is in play. |

## Restrictions (`restrict`)

| Token | Meaning |
| --- | --- |
| `cant_quest` | Can't quest. |
| `cant_challenge` | Can't challenge. |
| `cant_be_challenged` | Can't be challenged. |
| `cant_be_chosen` | Can't be chosen by opponents (Ward). |
| `cant_ready` | Doesn't ready during the ready step. |
| `takes_no_challenge_damage` | Takes no damage when challenging/challenged. |
