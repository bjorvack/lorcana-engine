# Lorcana Engine Implementation Plan

Based on [../architecture/ARCHITECTURE.md](../architecture/ARCHITECTURE.md) and the
official rules in [../rules/](../rules/).

## Approach: vertical, test-first slices

Instead of building every component horizontally and testing at the end, we build
**thin vertical slices**. Each slice:

- delivers a **playable increment** (you can actually do something end-to-end),
- ships with its **acceptance tests** (the slice is not "done" until they pass),
- only adds the engine machinery that slice needs.

This front-loads the hard integration risk (turn loop, the bag, choices, game-state
checks) and keeps a working game at every step. There are no day-by-day estimates;
progress is measured by passing acceptance scenarios.

### Guardrails applied to every slice
- **Determinism**: seed + ordered inputs ⇒ identical state and event log. The PRNG
  lives in `GameState`; no global RNG; ordered collections only where order matters.
- **Inputs vs events**: players submit `Action`s and `Choice`s; the engine emits
  `GameEvent`s. Decisions during resolution are inputs, never hidden internals.
- **Rules accuracy**: the bag (not a stack), no priority/response windows, correct
  zones (deck/hand/inkwell/play/discard/bag), conditions incl. dry/drying.
- **Structured DSL**: effects are serializable enums; `Effect::Custom(name)` maps to a
  compiled-in Rust handler. No embedded scripting.

---

## Slice 0 — Deterministic core skeleton ✅

**Goal**: a `GameState` you can construct, clone, and serialize deterministically.

- [x] `GameState`, `PlayerState`, `CardInstance`, `Conditions`.
- [x] Zone model: deck/hand/inkwell/play/discard (`ZoneKind`); ordered `Zone`.
- [x] Seeded PRNG (`SeededRng` over `ChaCha8Rng`) stored in `GameState`;
      deterministic shuffle.
- [x] Deterministic identifiers (`PlayerId` by seat, sequential `CardId`,
      `CardDefId`) replacing the earlier random-UUID ids.
- [x] serde round-trip; clone.

The reducer (`apply(state, Input) -> (state, Vec<GameEvent>)`), the `Input`
type, and `GameEvent` move to Slice 1, where actions and the turn loop give them
something to act on.

**Acceptance**
- [x] Construct a game from two decks + seed; serialize → deserialize → identical
      (`tests/serialization.rs`).
- [x] Same seed ⇒ identical state; different seeds ⇒ different shuffles
      (`tests/determinism.rs`).
- [x] `SeededRng` shuffle determinism (inline unit test in `rng.rs`).

---

## Slice 1 — Game setup & turn loop ✅

**Goal**: start a game and pass turns.

- [x] `GameStatus` (NotStarted → AwaitingMulligan → Playing → Finished{winners}).
- [x] `engine::start` — seed-derived starting player, deal opening hand of 7,
      enter mulligan (§3.1). `GameState::new` stays a raw builder.
- [x] Mulligan/alter-hand as turn-ordered `Input`s (put-back to bottom, redraw to
      7, reshuffle, §3.1.6).
- [x] `Input`/`apply` reducer: rejects illegal inputs without mutating; `GameEvent`
      output log.
- [x] Turn loop: auto-run Beginning(Ready→Set→Draw) → Main → End of Turn → next
      player (§4); the game's first turn skips Draw (§4.2.3.2).
- [x] Action: put a card into the inkwell — once per turn **and** inkwell-symbol
      enforced via a minimal `CardDefinition { inkwell }` + `CardRegistry`
      (§4.3.3, §6.2.8).
- [x] Action: end turn → pass to next non-eliminated player.
- [x] Loss on drawing from an empty deck wired through the game-state check
      (§1.9, §3.2.1.2).

**Acceptance**
- [x] A game runs turns alternating players with correct phase/step transitions
      (`tests/turn_flow.rs`).
- [x] Inkwell action enforces once-per-turn and the inkwell-symbol requirement.
- [x] Emptying the deck and being forced to draw loses the game.
- [x] Events emitted for each phase/step/turn transition.
- [x] Same seed + same inputs ⇒ identical state and event log.

**Notes**
- The win/loss check is the seam from the previous commit; the game-state-check
  driver (`game_state_check`) applies its required actions in turn order, with the
  win-beats-lose tie-break and last-player-standing.
- Full mid-resolution decisions (`PendingDecision`) are still deferred to Slice 8;
  mulligan only needs sequential, turn-ordered inputs.

---

## Slice 2 — Vanilla characters & questing

**Goal**: win a game with French-vanilla characters.

- Card definitions for characters (cost, S/W/L, classifications), loaded from TOML.
- Play a character paying ink cost; it enters `drying` (§5.1.11).
- Set step transitions `drying → dry`.
- Action: quest (exert a dry character, gain its `{L}`, §4.3.5).
- Win at 20 lore (§3.2).

**Acceptance**
- [ ] Cannot quest with a drying character; can after it dries.
- [ ] Questing exerts the character and adds the correct lore.
- [ ] Reaching 20 lore ends the game with the correct winner.
- [ ] Insufficient ink prevents playing a card.

---

## Slice 3 — Challenges

**Goal**: combat with damage and banishment.

- Action: challenge (exert a dry character; target an **exerted** opposing character or
  a location, §4.3.6).
- Both deal damage equal to `{S}`; damage counters persist (§9, §6.2.9–6.2.10).
- Game-state check banishes anything with `damage >= willpower` → **discard**.
- Drying characters can't be declared as challengers; challenge-targeting legality.

**Acceptance**
- [ ] Challenge applies mutual damage and banishes lethal characters to discard.
- [ ] Cannot challenge a ready character (must be exerted), nor with a drying character.
- [ ] Damage persists across turns until banishment/heal.

---

## Slice 4 — The bag & triggered abilities

**Goal**: simultaneous triggers resolve in correct order.

- `Ability::Triggered`; trigger conditions (e.g. "when you play this character" /
  "whenever this character quests").
- Bag: enqueue on trigger; **active player resolves all theirs (chosen order), then
  each player around the table** (§8.7); newly created triggers re-enqueue.
- Bag resolution can suspend on a `PendingDecision` (ordering / choices).
- Game-state check after each bag entry resolves.

**Acceptance**
- [ ] Multiple simultaneous triggers resolve active-player-first, in a player-chosen
      order, around the table until empty.
- [ ] A trigger created during resolution is added and resolved correctly.
- [ ] A worked example from §8.7 reproduces exactly.

---

## Slice 5 — Activated & static abilities, modifiers

**Goal**: costs and continuous effects.

- `Ability::Activated` (`[Cost] — [Effect]`): costs incl. exert, pay ink, banish-self.
  Activated abilities resolve immediately (not via the bag, §7.5).
- `Ability::Static`: continuous stat/permission modifiers while in play (§7.6).
- Modifier combination (§7.8); negative `{S}`/`{L}` clamp to 0 while retaining true
  value.
- **Win/loss modification layer**: wire static abilities into the win/loss seam
  from Slice 1 so effects can **add / remove-suppress / override** conditions
  (Golden Rules §1.2.1/§1.2.2). This realizes the edge cases enumerated in the
  `TODO(modification layer / Slice 5+)` block in
  [`src/domain/rules/win_loss.rs`](../../src/domain/rules/win_loss.rs) — e.g.
  Donald Duck – Flustered Sorcerer ("Opponents need 25 lore to win") overriding
  `lore_to_win`. Convert those TODO bullets into real tests here.

**Acceptance**
- [ ] An activated ability pays its cost and applies its effect; illegal if cost
      unpayable.
- [ ] A static `+N {S}` applies to existing and newly-played matching characters and
      ends when its source leaves play.
- [ ] Stacking positive/negative modifiers clamps for use but not for further math.
- [ ] A static ability can override the win threshold (Donald Duck: opponents need
      25 lore), and the `win_loss.rs` modification-layer TODO cases are now tested.

---

## Slice 6 — Keywords (incremental)

**Goal**: implement the keyword set (§10), simplest first.

1. Static/“simple”: **Evasive, Bodyguard, Rush, Resist, Challenger, Ward, Support,
   Singer**.
2. Complex: **Shift** (card stacks!), **Sing Together**, **Boost**, **Vanish**,
   **Reckless**, **Alert**.

Each keyword gets reminder-text-accurate behaviour and a focused scenario test. Shift
introduces the in-Play card-stack model (top/under/in-a-stack, §5.1.6–5.1.7, §10.10).

**Acceptance**
- [ ] Each keyword has a passing scenario matching its §10 definition/example.
- [ ] Shift forms/moves stacks correctly; the stack moves with its top card on leave.

---

## Slice 7 — Songs, locations, movement

**Goal**: remaining card types.

- **Songs**: Action + "Song" classification; pay by exerting a character of
  sufficient cost (§6.3.3); interaction with Singer / Sing Together.
- **Locations**: play, move cost to move a character there (§4.3.7), willpower &
  banishment, start-of-turn lore (§6.5).

**Acceptance**
- [ ] A song can be sung by exerting an eligible character or paid for with ink.
- [ ] Characters move to a location for its move cost; locations grant lore at Set.

---

## Slice 8 — Replacement effects & choices

**Goal**: the trickiest resolution rules.

- Replacement effects (§7.7): "instead"/"skip"/"enter"; self-replacement applied
  first; "same replacement can't apply twice"; replacement of steps/phases.
- Choice machinery completeness: "may" (§7.1.3), "up to N" (§7.1.8, no duplicates),
  ordering simultaneous discards/destinations, "that [game term]" resolution (§7.1.9).
- Floating & delayed triggered abilities (§7.4.7).

**Acceptance**
- [ ] A worked replacement example from §7.7 reproduces exactly (ordering included).
- [ ] "Up to N" forbids duplicate picks and allows 0; "may" can decline cleanly.
- [ ] A delayed trigger ("at the end of your turn, …") fires at the right moment.

---

## Slice 9 — Real card data & conformance suite

**Goal**: scale beyond hand-written cards and lock in correctness.

- Bulk card-data loader mapping a community dataset (e.g. LorcanaJSON-style data) into
  our `CardDefinition`/DSL, or generate TOML from it.
- Definition validation on load (schema + DSL well-formedness).
- A conformance test suite: encode the rules examples (§7–§10) and a library of
  hand-authored interaction scenarios as golden tests.

**Acceptance**
- [ ] A meaningful subset of a real set loads and validates.
- [ ] The conformance suite passes and runs in CI.

---

## Cross-cutting tracks (run alongside slices)

- **Public API** (`application/api`): grow `new_game`, `submit(Action|Choice)`,
  `query(state view)`, `subscribe(events)` as slices need them — not a separate late
  phase.
- **CLI** (`src/main.rs`): a thin text host to drive/inspect games for manual testing.
- **Determinism property tests**: maintained continuously, not deferred.
- **Docs**: keep ARCHITECTURE.md in sync as concrete types land.

## Definition of done (per slice)

1. Behaviour matches the cited rules sections.
2. Unit + scenario acceptance tests pass.
3. Determinism property test still holds.
4. `cargo fmt`, `cargo clippy` (pedantic/nursery clean), `cargo test` green.
5. ARCHITECTURE.md updated if the slice introduced/changed core types.

## Known code corrections to make as we implement

- Remove the fabricated `Step::Cleanup`; End of Turn is its own phase, not a cleanup
  step.
- Fix the `Set` step doc comment ("set ink" is wrong — Set is dry + location lore).
- Treat **Song** as an Action with the "Song" classification rather than a separate
  `CardType::Song` variant.

## Possible future multi-crate split

If the project grows, the existing boundaries extract cleanly into:
`lorcana-domain`, `lorcana-infrastructure`, `lorcana-application`, `lorcana-cli`.
