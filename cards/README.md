# Card Definitions

Card definitions in the engine's **own TOML format** (`*.toml`). These are
authored by us; external datasets (e.g. Lorcast) are only research aids and are
never loaded directly.

## Format

Each file is a list of `[[card]]` tables. Only the **printed characteristics +
keywords** are defined here; a card's text-based triggered / activated / static
abilities are authored via the effect DSL (a separate concern — see
`docs/architecture/ARCHITECTURE.md`).

```toml
[[card]]
name = "Genie"
type = "Character"            # Character | Action | Song | Item | Location
cost = 5
inkwell = true
strength = 4                 # characters
willpower = 5                # characters / locations
lore = 2                     # characters / locations
move_cost = 1                # locations
classifications = ["Floodborn", "Ally"]
keywords = ["Evasive", "Challenger 2"]   # value (if any) is inline
```

Keyword values are inline: `"Challenger 2"`, `"Resist 1"`, `"Shift 5"`,
`"Singer 5"`, `"Sing Together 4"`, `"Boost 1"`; valueless keywords are just their
name (`"Evasive"`, `"Bodyguard"`, `"Ward"`, …).

## Loading

`lorcana_engine::load_toml(&str)` parses a document into `CardDefinition`s
(validating types/stats/keywords), which insert directly into a `CardRegistry`.
See `cards/examples.toml` and `tests/card_loader.rs`.
