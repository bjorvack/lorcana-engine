# Card Definitions

Cards in the engine's **own TOML format**, authored by us. External datasets
(e.g. Lorcast) are only research aids — they are never loaded by the engine.

## Layout

- `<set>/<collector_number>.toml` — **one file per card** and the **source of
  truth** (e.g. `1/103.toml`). The set directory is the lowercased set code
  (`1`…`12`, `p1`/`p2`/`p3`, `c2`, `cp`, `d23`, `dis`); the filename is the card's
  Lorcast collector number. Each file holds one card's structured fields + printed
  `text` at the top level (abilities are authored separately — see below).
- `sets/<code>.toml` — **generated, git-ignored** combined files (one per set) that
  the engine loads and the wasm crate embeds (`include_dir!`). Regenerate them
  before building or testing:
  ```bash
  python3 cards/scripts/combine_sets.py
  ```
  CI runs this step in every job; never edit or commit `sets/`.
- `scripts/combine_sets.py` — per-card files → `sets/<code>.toml` (textual; runs on
  any Python 3).
- `scripts/split_sets.py` — one-time migration that produced the per-card files
  from the old combined files (collector numbers from the Lorcast API by image
  hash).
- `scripts/from_lorcast.py` — (re)generates the per-card files' structured fields +
  `text` from a research dump. It does **not** emit abilities.
- `scripts/card_io.py` — shared field emitter used by the generators.

## Card format

Each per-card file holds one card's fields at the top level (no `[[card]]`
wrapper — `combine_sets.py` adds that when building a set file):

```toml
name = "Genie - The Ever Impressive"   # full name (Character - Version)
type = "Character"                     # Character | Action | Song | Item | Location
cost = 5
ink = ["Sapphire"]                     # 1 ink, or 2 for dual-ink: ["Ruby", "Sapphire"]
image = "https://…"                    # display image URL
max_copies = 4                         # deck copy-limit (default 4; e.g. 99 for Dalmatian Puppy)
inkwell = true                         # has the inkwell symbol (omitted when false)
strength = 4                           # characters
willpower = 5                          # characters / locations
lore = 2                               # characters / locations
move_cost = 1                          # locations only
classifications = ["Floodborn", "Ally"]
keywords = ["Evasive", "Challenger 2"] # value (if any) is inline
text = "<printed rules text>"          # used to author abilities (AI pass)
```

Keyword values are inline: `"Challenger 2"`, `"Resist 1"`, `"Shift 5"`,
`"Singer 5"`, `"Sing Together 4"`, `"Boost 1"`; valueless keywords are just their
name (`"Evasive"`, `"Bodyguard"`, `"Ward"`, `"Rush"`, `"Alert"`, `"Reckless"`,
`"Vanish"`, `"Support"`).

## Abilities (the effect DSL)

A card's abilities are authored as sub-tables **below** its scalar fields, at the
top level of the per-card file (`combine_sets.py` re-nests them under `card.` when
building a set file). See `src/domain/cards/dsl.rs` for the authoritative grammar.

```toml
# Triggered ability: on = play | play_action/song/character/item/location |
#   quest | challenge | challenged | banish | banished_in_challenge |
#   banishes_in_challenge | start_of_turn | end_of_turn.
# `do` is one effect, or an array (resolved in order). `may = true` for "you may".
[[abilities]]
on = "play"
do = [{ draw = 1 }, { gain_lore = 1 }]

# Activated ability: a cost + an effect.
[[activated]]
cost = { exert = true, ink = 1 }
do = { draw = 1 }

# Static stat modifier: one of strength/willpower/lore, on a selector,
# optionally scaled (`per`) or gated (`while = "exerted"`).
[[statics]]
strength = 1
to = "your other Hero characters"
```

**Effect verbs** (`do`): `draw`, `gain_lore`, `lose_lore`, `deal_damage`,
`remove_damage`, `give_strength`, `banish`, `exert`, `ready`, `freeze`, `discard`,
`return_to_hand`, `into_inkwell`, `grant_keyword` (`duration = "this_turn"`
default | `"permanent"`), `if_you_have` (`+ at_least = N`, `then = {…}`),
`move_damage` (`= N, from = "…", to = "…"`), `look_at_top` (`= N, take = "<filter>", rest = "bottom"|"top"|"shuffle"`), and `restrict` (`= "cant_quest" |
"cant_challenge" | "cant_be_challenged" | "cant_be_chosen" | "cant_ready" |
"takes_no_challenge_damage"`, with the same `duration`).

**Amounts** (numbers, or a dynamic string): an integer, `"per <filter>"`,
`"cards in hand"`, `"damage on self"`.

**Selectors** (`target` / `to` / filters): `"self"`,
`"chosen [opposing|your] [Classification] character"`, `"all/another … characters"`,
`"chosen item/location"`, plus predicates `"named X"`, `"with cost N or less/more"`,
`"with N {S}/{W}/{L} or more/less"`. A leaf may also be the full structured AST
form (a TOML table) when the compact grammar can't express it.

Actions/Songs: an `on = "play"` ability becomes the card's on-play action effect.

## Loading

`lorcana_engine::load_toml(&str)` parses a document into `CardDefinition`s
(validating types/stats/keywords/abilities); `load_toml_from(&str, first_id)`
assigns unique ids so multiple sets load into one registry. Both insert into a
`CardRegistry`. See `tests/card_loader.rs`, `tests/all_sets_load.rs`, and the
deck tooling in `decks/`.
