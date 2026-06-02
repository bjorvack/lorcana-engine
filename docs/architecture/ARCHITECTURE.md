# Lorcana Engine Architecture

> **Note**: This is a living document describing the intended architecture of the
> lorcana-engine. It is written to match the official Disney Lorcana Comprehensive
> Rules (see [../rules/](../rules/)). As implementation progresses via vertical
> slices (see [../planning/IMPLEMENTATION_PLAN.md](../planning/IMPLEMENTATION_PLAN.md)),
> concrete type definitions will be filled in here.

## Overview

The lorcana-engine is a headless, deterministic, rules-accurate game engine for
Disney's Lorcana trading card game. It is data-driven (card definitions describe
behaviour declaratively) and is built so that the same seed plus the same ordered
sequence of inputs always produces the same game state and event log.

The architecture deliberately models Lorcana's *actual* rules rather than borrowing
concepts from other TCGs. In particular, Lorcana has **no MTG-style stack and no
priority/response windows** — it uses **the bag** for simultaneous triggered
abilities, and effects resolve at "sorcery speed" only.

## Core Design Principles

### 1. Deterministic, reproducible game state
- **Deterministic**: same seed + same ordered inputs ⇒ identical state and event log.
- **Replayable**: the full input stream can be re-applied to reconstruct any point.
- **Serializable**: game state can be saved/loaded at any point, including while
  waiting on a player decision mid-resolution.
- **Authoritative**: a single `GameState` is the source of truth.

To preserve determinism:
- The seeded PRNG is **domain state**: a `SeededRng` (wrapping
  `rand_chacha::ChaCha8Rng`) lives **inside** `GameState` and is serialized with
  it, so all randomness (shuffles, random discards) is part of the reproducible
  state. No global RNG. `ChaCha8Rng` is chosen because its output is stable
  across crate versions, unlike `StdRng`.
- Identifiers are deterministic: players are identified by seat index
  (`PlayerId`) and card instances by a sequentially allocated `CardId` — never
  by random UUIDs, which would break replays.
- Game logic avoids iteration-order-dependent containers. Use ordered collections
  (`BTreeMap`, `Vec`, index-keyed maps) wherever iteration order can affect outcomes.

### 2. Inputs vs. events
The engine is a **state machine driven by an input stream**:
- **Inputs** are the things players submit: `Action`s (play a card, quest, challenge,
  put a card in the inkwell, end turn, …) and `Choice`s (answers to decisions the
  engine requests during resolution, e.g. "choose a character", "you may …",
  "order these triggers").
- **Events** are **outputs**: an append-only log describing what happened, for UIs,
  replays, and debugging.

This distinction matters because many Lorcana effects require player input *while an
ability is resolving*. Those choices are first-class inputs, not hidden internal
decisions — that is what keeps replays exact.

### 3. Data-driven card definitions
- Card behaviour is described declaratively (definitions + a structured effect DSL).
- Adding most new cards requires **no engine code changes**, only new definitions.
- Definitions are version-control friendly and validated on load.

### 4. Structured effect DSL (no general-purpose scripting)
- Effects, targets, and conditions are represented as **serializable Rust enums**.
- The vast majority of card text is templated and maps directly onto this DSL.
- For the rare card that doesn't fit, the escape hatch is `Effect::Custom(name)`,
  which dispatches to a **compiled-in, deterministic Rust handler** — not an embedded
  script interpreter. This protects determinism, serialization, and replay.
- Embedded scripting (e.g. Rhai) is intentionally **not** part of the core design. It
  can be reconsidered only if a concrete card provably cannot be expressed by the DSL
  plus custom handlers.

### 5. Headless
- No coupling to rendering or input. The engine emits events; a host consumes them.
- Embeddable in web (WASM), desktop, mobile, or terminal.

## The bag (Lorcana's resolution model)

This is the most important rules-specific concept and replaces any notion of a
"stack" or "priority":

- When one or more triggered abilities' conditions are met simultaneously, each is
  added to **the bag** by the controller of the card that triggered it (rules §8.7.3).
- The **active player** then resolves **all** of their abilities from the bag, one at
  a time and in an order they choose (§8.7.4–8.7.5). Newly created triggers are added
  to the bag as the current ability finishes.
- Then the **next player in turn order** resolves all of theirs, and so on around the
  table, until the bag is empty (§8.7.6–8.7.8).
- There is **no opponent response window**: players cannot play cards or activate
  abilities in reaction to another player's action. Activated abilities and plays are
  taken by the active player during their Main Phase only.

Implications for the engine:
- The bag is an ordered collection plus a small resolver state machine; it is **not**
  LIFO and is **not** a zone in the physical sense (though the rules group it with
  zones in §8.7).
- Resolving a bag entry may require a `Choice` from a specific player, so bag
  resolution can suspend on a `PendingDecision`.

## Zones

Per rules §8, the zones are exactly:

| Zone | Visibility | Notes |
|------|------------|-------|
| Deck | private | facedown, ordered |
| Hand | private (owner) | |
| Inkwell | private | facedown; each card = 1 ink regardless of its face |
| Play | public | characters, items, locations |
| Discard | public | banished cards and resolved actions go here |
| Bag | n/a | where triggered abilities wait to resolve (see above) |

Notes:
- There is **no separate "banished" zone** — banished cards go to **discard** (§8.6.2).
- "Field"/"battlefield" is called **Play**.
- **Card stacks** (created by Shift) are *not* a zone and are *not* the bag. A stack is
  an ordered pile of card instances **within Play**: a top card with one or more cards
  under it (§5.1.6–5.1.7, §10.10). The engine models this as a stack of card instances
  attached to the top card; when the top card leaves play the whole stack moves with it.

## Card instances and conditions

A **card definition** is static data (name, cost, ink, stats, abilities). A **card
instance** is a specific card in a specific zone with mutable state. Per rules §5, an
instance carries **conditions**:

- `ready` / `exerted`
- `damage: u32` (damage counters; persistent; banish when `damage >= willpower`, §6.2.10)
- `dry` / `drying` — the "summoning sickness" condition. A **drying** character cannot
  quest, be declared as a challenger, or exert to pay a cost (§5.1.11). It becomes
  **dry** at the start of its controller's next turn (Set step).
- `faceup` / `facedown`
- stack membership (`on_top` / `under` / `in_a_stack`)

Conditions are validated per zone (§5.1.13): e.g. inkwell cards may only be
ready/exerted/facedown; deck cards only facedown; discard cards only faceup.

## Turn structure

Matches rules §4 exactly:

- **Beginning Phase**: `Ready` → `Set` → `Draw`
  - *Ready*: "during your turn" effects begin; ready all your cards (§4.2.1).
  - *Set*: drying characters become dry; gain lore from your locations; start-of-turn
    triggers go to the bag and resolve (§4.2.2).
  - *Draw*: draw a card (the starting player skips this on turn 1) (§4.2.3).
- **Main Phase**: turn actions in any order — put a card in inkwell (once per turn),
  play a card, quest, challenge, move a character to a location, use activated
  abilities (§4.3).
- **End of Turn Phase**: end-of-turn triggers go to the bag; "this turn" effects end
  (§4.4).

(There is no "Cleanup" step; that earlier name has been removed.)

## Game-state checks

The engine performs a **game-state check** at the times defined in §1.9 (end of any
step, after any action or ability finishes resolving, and after each bag entry
resolves). A check applies required actions such as banishing characters/locations
with `damage >= willpower` and detecting win/loss (20 lore; empty-deck draw).

## Card types

Per rules §6:
- **Character** — has `{S}` strength, `{W}` willpower, `{L}` lore, and at least one
  classification; can quest and challenge.
- **Action** — played for a one-time effect, then to discard; never enters Play. A
  **Song** is an Action with the **"Song" classification** (not a distinct card type),
  payable by exerting a character (§6.3.3).
- **Item** — stays in Play.
- **Location** — stays in Play; has move cost and willpower; may give lore each turn.

## Abilities

Per rules §7, modeled as a tagged union:
- **Triggered** ("When/Whenever/At the start of/At the end of …") → goes to the bag.
- **Activated** (`[Cost] — [Effect]`) → used by the active player; resolves immediately
  (not via the bag).
- **Static** — continuous modifiers/permissions while in play (or for a duration).
- **Replacement** ("instead"/"skip"/"enter") → §7.7, including self-replacement
  ordering and "same replacement can't apply twice" rules.
- **Floating / delayed** triggered abilities (§7.4.7) exist outside the bag until their
  condition is met, then enqueue.

Stat modifiers (§7.8) apply continuously and combine; negative `{S}`/`{L}` clamp to 0
for use while retaining the true value for further modification.

## Layered structure

All **game logic lives in `domain`**. `infrastructure` is IO/adapters only.
`application` is a thin facade.

```
┌───────────────────────────────────────────────┐
│ Host (UI / CLI / tests) — consumes events,     │
│ submits Actions and Choices                    │
└───────────────────────────────────────────────┘
                     │
                     ▼
┌───────────────────────────────────────────────┐
│ application/api  (thin facade)                 │
│  new_game · submit(Action|Choice) · query ·    │
│  subscribe(events)                             │
└───────────────────────────────────────────────┘
                     │
                     ▼
┌───────────────────────────────────────────────┐
│ domain  (the engine)                           │
│  state · zones · turn · cards · effects ·      │
│  bag · rules · resolution · events             │
└───────────────────────────────────────────────┘
                     │
                     ▼
┌───────────────────────────────────────────────┐
│ infrastructure  (IO/adapters)                  │
│  parsing(TOML) · serialization(serde) ·        │
│  card-data loader                              │
└───────────────────────────────────────────────┘
```

Note: the seeded PRNG is **not** an infrastructure adapter — because it is part
of the serialized, reproducible game state it lives in the domain
(`domain/game/rng.rs`), keeping the dependency direction (domain → nothing)
clean.

## Module layout

The domain groups its game-state types under `domain/game/`. Modules marked
*(planned)* will be added by later slices.

```
src/
├── main.rs                     # CLI entry point
├── lib.rs                      # library exports / public prelude
├── domain/
│   ├── game/                   # game state and the engine core
│   │   ├── state.rs            # GameState (owns the seed + SeededRng)
│   │   ├── player_state.rs     # PlayerState + its zones
│   │   ├── card_instance.rs    # CardInstance (CardId + CardDefId + Conditions)
│   │   ├── conditions.rs       # Conditions (ready/exerted, damage, drying, …)
│   │   ├── zone.rs             # Zone: ordered pile of CardInstance
│   │   ├── zone_kind.rs        # ZoneKind: deck/hand/inkwell/play/discard
│   │   ├── rng.rs              # SeededRng (ChaCha8Rng) — domain state
│   │   ├── turn.rs             # turn progression (planned)
│   │   └── events.rs           # output GameEvent log (planned)
│   ├── types/                  # leaf types: ids, card enums, phase/step
│   ├── cards/                  # CardDefinition, Registry, keywords (planned)
│   ├── effects/                # Effect/Target/Condition DSL + resolver (planned)
│   ├── bag/                    # trigger collection + ordered resolution (planned)
│   ├── rules/                  # legality + game-state checks (planned)
│   └── resolution/             # PendingDecision / Choice (planned)
├── infrastructure/
│   ├── parsing/                # TOML → CardDefinition (planned)
│   ├── serialization/          # serde helpers (planned)
│   └── carddata/               # bulk card-data loader (planned)
├── application/
│   └── api/                    # thin facade: actions, choices, queries, events (planned)
└── shared/                     # error & result types
```

Type-safe IDs (`CardId`, `CardDefId`, `PlayerId`, `GameId`) and core enums
(`CardType`, `InkType`, `Rarity`, `Phase`, `Step`) live under `domain/types/`.

## Data flow

### Submitting an action
```
Action ──▶ api.submit
        ──▶ rules: legality check (turn/phase, cost, timing, targeting)
        ──▶ domain: apply primitive state changes
        ──▶ effects/bag: enqueue & resolve (active player first, around the table)
        ──▶ rules: game-state check (banish, win/loss)
        ──▶ events appended
        ──▶ returns either "advanced" or a PendingDecision
```

### Resolving a decision
```
Choice ──▶ api.submit (must match the outstanding PendingDecision)
        ──▶ resume the suspended resolution at the exact point it paused
        ──▶ continue effects/bag → game-state check → events
```

## Technology stack

**Core**: `serde` (state + definitions), `toml` (definitions),
`rand` + `rand_chacha` (deterministic `ChaCha8Rng` seeded PRNG), `thiserror`
(errors), `uuid` (the external `GameId` handle).

**Intentionally excluded from the core**: a general-purpose embedded script engine.
Effects use the structured DSL + compiled-in custom handlers instead.

## Testing strategy

Testing is **continuous and slice-driven**, not a final phase:
- **Unit tests** for each component as it is built.
- **Scenario/integration tests** per vertical slice (a slice isn't done until its
  acceptance scenarios pass).
- **Property tests** for the core determinism invariant:
  `seed + inputs ⇒ identical state + event log` (replay equivalence).
- **Golden tests** for known states and expected event sequences.
- **Conformance tests** mapping rules-section examples (e.g. the worked examples in
  §7–§10) to executable scenarios.

## Future extensibility

- **New cards**: add definitions; extend the DSL only when needed.
- **New mechanics/keywords**: add DSL/condition variants and rules hooks.
- **Performance**: continuous-effect caching, state diffing, WASM build for web.
- **Multi-crate**: the domain/infrastructure/application boundaries are designed so
  they can later be split into separate crates if the project grows.
