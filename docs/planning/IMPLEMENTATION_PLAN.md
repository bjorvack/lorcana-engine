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

## Slice 2 — Vanilla characters & questing ✅

**Goal**: win a game with French-vanilla characters.

- [x] `CardKind` enum (Character{strength, willpower, lore}, Action, Item,
      Location); `CardDefinition` expanded with `cost` + `kind`; `CardType` is a
      derived tag. (Classifications/abilities deferred until referenced.)
- [x] `Input::PlayCard` — play a character, paying its ink cost by auto-exerting
      ready ink (fungible, §8.5.1); it enters `drying` (§5.1.11). Non-character
      types are rejected for now (`CardTypeNotPlayableYet`).
- [x] Set step transitions `drying → dry` (wired in Slice 1, now meaningful).
- [x] `Input::Quest` — exert a dry, ready character and gain its `{L}` (§4.3.5);
      rejects drying/exerted/not-a-character/not-in-play.
- [x] Win at 20 lore via questing, through the game-state check (§3.2).

**Acceptance**
- [x] Cannot quest with a drying character; can after it dries
      (`tests/play_and_quest.rs`).
- [x] Questing exerts the character and adds the correct lore.
- [x] Reaching 20 lore ends the game with the correct winner.
- [x] Insufficient ink prevents playing a card (rejected, no mutation).

**Notes**
- TOML loading of definitions is deferred to Slice 9 (real card data); Slice 2
  builds `CardDefinition`s directly / via a test `CardRegistry`.
- Card classifications aren't modeled yet — nothing references them until static
  abilities (Slice 5); added then.

---

## Slice 3 — Challenges ✅

**Goal**: combat with damage and banishment.

- [x] In-play character stats live on the `CardInstance` (`CharacterStats`, set
      from the definition at play time); the game-state check stays state-only.
- [x] `Input::Challenge` — exert a dry, ready character; target an **exerted**
      opposing character (§4.3.6). Both deal `{S}` damage simultaneously
      (§4.3.6.13); damage counters persist (§9).
- [x] `RequiredAction::Banish` in the game-state check: `damage ≥ willpower` →
      banish to **discard**, clearing counters (§1.9.1.3, §9.4, §8.6.2); win/loss
      still resolved first (§1.9.2).
- [x] Legality: drying/exerted challenger rejected; target must be opposing,
      in-play, and exerted; rejections leave state unchanged.

**Acceptance**
- [x] Challenge applies mutual damage and banishes lethal characters to discard
      (`tests/challenge.rs`), including a trade that banishes both.
- [x] Cannot challenge a ready character, nor with a drying character.
- [x] 0-strength characters deal no damage (§4.3.6.14).
- [x] Damage persists (only cleared on banishment for now).

**Notes — challenge/banish are heavy hook points (deferred, cross-linked):**
- Location challenge → Slice 7 (no locations yet).
- Legality overrides: Rush, Evasive, Alert, Bodyguard → Slice 6; "can challenge
  ready" / "can't challenge" effects → Slice 5/8. (See the `apply_challenge` doc
  comment in `src/domain/engine/reducer.rs`.)
- Challenge/banish **triggers** ("whenever this character challenges / is
  banished / banishes another in a challenge") → **Slice 4** (the bag), with the
  §1.9.1.3 "banished by that character" attribution. Damage modification (Resist)
  → Slice 6; banish replacement/prevention → Slice 8. (See the `game_state_check`
  TODO in `src/domain/rules/game_state_check.rs`.)

---

## Slice 4 — The bag & triggered abilities ✅ (core)

**Goal**: simultaneous triggers resolve in correct order.

Grounded in a survey of all **2,314 cards with text**; the full trigger taxonomy
is recorded as a TODO on `TriggerCondition` (`src/domain/effects/trigger.rs`).

- [x] Data model: `TriggerCondition` (small: `WhenYouPlayThis`, `WhenThisQuests`),
      minimal `Effect` (`DrawCards`, `GainLore`, `EachOpponentLosesLore`), and
      `TriggeredAbility` (with an optional/"you may" flag) on `CardDefinition`.
- [x] Bag (`§8.7`): triggers enqueue; the **active player resolves all of theirs
      first (in a player-chosen order), then each player around the table**;
      a game-state check follows each bag entry.
- [x] Resolution **suspends on a `PendingDecision`** (`OrderTriggers` when a player
      has ≥2 triggers; `MayResolve` for "you may") and resumes via
      `Input::Decide(Decision)`; other inputs are rejected while a decision is
      pending. This is the first piece of the choice/`PendingDecision` machinery.
- [x] ETB and quest self-triggers detected and fired with the minimal effects.

**Acceptance**
- [x] Multiple simultaneous triggers: the controller chooses the order via
      `OrderTriggers`; both resolve (`tests/triggers.rs`).
- [x] Optional triggers wait for `May(yes/no)`; declining does nothing.
- [x] ETB draw and quest triggers fire; deterministic across a play+decide run.

**Deferred (cross-linked) — not in this slice:**
- Broader trigger conditions (play-a-[type], challenge, banish, start/end of turn,
  damage, sing, boost…) — add against the `TriggerCondition` TODO as needed.
- **Challenge/banish triggers** (Scar, Captain Hook, Cheshire Cat, Marshmallow)
  and the §1.9.1.3 "banished by that character" attribution: now that the bag
  exists, these enqueue from the hooks documented in `apply_challenge` /
  `game_state_check`. Slice 6 (keyword interactions) and the full effect DSL
  (Slice 8) build on this.
- The full effect/target DSL and richer decisions (targeting, "up to N") — Slice 8.

### Trigger taxonomy rollout (when the `TriggerCondition` TODO gets done)

The `TriggerCondition` TODO (`src/domain/effects/trigger.rs`) is a **living
checklist**, ticked off as the mechanics that emit each event arrive — there is
no single "done" moment. Each addition follows the same recipe: add the variant
+ detection + a scenario test. Mapping of the deferred kinds to their slice:

| Deferred trigger kind | Lands in |
|---|---|
| Start / End of turn; play-a-[type/classification] (needs classifications) | **Slice 5** |
| Challenge / banish triggers (challenges, is challenged, banishes-another, is banished) + §1.9.1.3 "banished by that character" attribution; Boost trigger | **Slice 6** |
| Sing-a-song; move-to-location / "while here" | **Slice 7** |
| Damage / ready / leaves-play / draw (and any stragglers) | as needed, Slices 6–8 |
| Full taxonomy + scope filters completeness | guaranteed by **Slice 9** (real card data + conformance) |

**Structural item (don't forget):** today only *self*-scoped triggers are detected
at the action site (`enqueue_self_triggers`). Watching *other* cards' events
(scope filters: one of your / a / an opposing character) requires a general
**event → trigger matcher**. Build it when the first cross-scope card appears
(Slice 5 or 6), not as per-card hacks; harden it in Slice 9. Each slice below
back-links here.

---

## Slice 5 — Activated & static abilities, modifiers

**Goal**: costs and continuous effects. Split into the smallest shippable
sub-slices; each is independently tested and committed.

**Decision — modifier model (recorded):** continuous-effects list, computed on
demand. Printed base stats stay on the `CardInstance`; `GameState` holds active
modifiers `{source, selector, stat, delta, duration}`; a current value is
`base + Σ matching deltas`, clamped to 0 only at point of use while the true
value is retained for further modifier math (§7.8.1.2/§7.8.2/§7.8.3). Effects are
added when their source enters play and removed when it leaves (§7.6.4); timed
("until end of turn") ones expire at cleanup. Keeps the game-state check
state-only (consistent with Slice 3). Grounded in the card pool: `-N{S}` ×71,
selectors over 42 classifications.

### Slice 5a — Activated abilities ✅
- `ActivatedAbility { cost, effect }` on `CardDefinition`; `Input::UseAbility`.
- Costs: exert-self + pay-ink now (the dominant `{E}` / `{E}+N{I}` shapes);
  banish-self / discard deferred (TODO with back-link). Drying characters can't
  pay an `{E}` cost (§4.2.2.1).
- Resolve **immediately**, not via the bag (§7.5.3.3); reuse the minimal effects.
- [x] Acceptance: an activated ability pays its cost and applies its effect;
      illegal if the cost is unpayable or the source is drying/exerted.

### Slice 5b — Classifications (data) ✅
- `Classification` (open-vocabulary newtype over `String`) + `classifications`
  on `CardDefinition`. Unblocks selectors (5e) and play-a-classification triggers.
- [x] Acceptance: classifications round-trip and are queryable.

### Slice 5c — Continuous-effects layer (refactor, no behaviour change) ✅
- `GameState` modifier list + `current_character_stats(card)` = base + Σ deltas
  (clamped at use, true total retained). Challenge/quest/banishment now read
  current stats; modifiers end when their source leaves play.
- [x] Acceptance: all existing tests still pass; current == base with no
      modifiers; combine/clamp follows §7.8 (`tests/modifiers.rs`).

### Slice 5d — Self static modifiers ✅
- `StaticAbility::self_modifier(stat, delta)` on `CardDefinition`; applied as a
  `WhileSourceInPlay` modifier when the card enters play (§7.6.2), removed when it
  leaves (§7.6.4, via `remove_modifiers_from_source` in the banish path).
- [x] Acceptance: a self `+N{S}` is reflected in `current_character_stats` on
      enter (and thus in challenge damage, which reads current stats);
      `tests/modifiers.rs`.

### Slice 5e — Selector static modifiers (needs 5b) ✅
- Classifications denormalized onto `CardInstance` (so matching is state-only);
  `ModifierTarget::OwnedCharacters { owner, classifications (any-of), except }`
  and `StaticAbility::owned_characters(...)`. `GameState::target_matches` resolves
  selectors against in-play owner + classifications, evaluated on demand so the
  set is dynamic (later-entering cards are affected, §7.6.2).
- [x] Acceptance: "your Villain characters get +N" buffs only matching owned
      characters incl. later-entering ones; `except` gives "your other
      characters"; ±combine retains true value (§7.8); `tests/modifiers.rs`.

  Note: `CardInstance` is now `Clone` (not `Copy`) since it owns classifications.

### Slice 5f — Timed modifiers ✅
- `ModifierDuration::UntilEndOfTurn` modifiers are removed at the End step
  (§7.6.1) via `expire_end_of_turn_modifiers`. (Effects that *create* timed
  selector modifiers must snapshot their targets per §7.6.3 — back-linked TODO on
  that method, lands with the effect DSL in Slice 8.)
- [x] Acceptance: a `this turn` modifier ends at end of turn (`tests/modifiers.rs`).

### Slice 5g — Win/loss & game-rule static modifiers ✅ (override layer)
- `GameRuleStatic` on `CardDefinition` + `RuleModifier` in `GameState`;
  `lore_to_win(state, player)` now reads the effective threshold. Donald Duck –
  Flustered Sorcerer ("Opponents need 25 lore to win") adds a `LoreToWin`
  override for each opponent on enter; it's removed when he leaves play (§7.6.4),
  and the game-state check applies the now-eligible win on the next pass (§1.9.2).
- [x] Acceptance: Donald raises opponents' threshold to 25 (own stays 20); when
      Donald leaves play a pending 20-lore win resolves (`tests/win_loss_modifiers.rs`).
- Remaining (deferred, back-linked in `win_loss.rs`): the **add** and
  **remove/suppress** condition kinds ("you can't lose", added alternate wins)
  need their ability kinds + the effect DSL (Slice 5g+/8).

### Slice 5h — Trigger additions (see [Trigger taxonomy rollout](#trigger-taxonomy-rollout-when-the-triggercondition-todo-gets-done))
- [x] **Play-a-[classification]** (`TriggerCondition::WhenYouPlay(CardCategory)`):
      the cross-scope **event → trigger matcher** (`enqueue_play_a_card_triggers`)
      scans the controller's other in-play cards on a play and enqueues matches.
      Only characters are playable, so character categories are exercised;
      action/song/item/location categories are wired but unreachable until those
      types are playable (Slice 7). Tested in `tests/triggers.rs`.
- [ ] **Start/End-of-turn triggers — DEFERRED (needs a design decision).** These
      fire during the turn's Beginning/End, where resolving the bag can **suspend**
      on a `PendingDecision` (ordering / "may"). The current engine resolves the
      bag and returns; it has no way to **resume the rest of a turn transition**
      after a suspension (e.g. finish the End step and pass the turn once an
      end-of-turn trigger's decision is answered). Implementing these correctly
      needs a **turn-progression state machine that survives suspension** (a
      "what to do after the bag empties" continuation), which also requires
      threading the registry through `start` / `apply_end_turn` / `begin_turn`.
      This overlaps the resolution work in Slice 8 and should be designed
      deliberately. Until then `WhenYouPlayThis`/`WhenThisQuests`/`WhenYouPlay`
      (which fire at input sites that already resolve the bag) are sufficient.
- [ ] Acceptance (remaining): a start-of-turn trigger fires (after the
      turn-progression-with-suspension machinery lands).

---

## Slice 6 — Keywords (incremental)

**Goal**: implement the keyword set (§10), simplest first. Modeled as a `Keyword`
enum (`src/domain/cards/keyword.rs`, full §10 set; behaviour wired per sub-slice —
see the TODO there). Split smallest-first like Slice 5.

### Slice 6a — Challenge-cluster keywords
- **Rush** (§10.9): challenger needn't be dry. **Evasive** (§10.6) / **Alert**
  (§10.2): only Evasive (or an Alert challenger) may challenge an Evasive target.
  **Bodyguard** (§10.3.3): an opponent must challenge a Bodyguard if able.
  **Resist +N** (§10.8): reduces challenge damage taken. **Challenger +N**
  (§10.5): +N `{S}` while challenging.
- All wired into the Slice 3 challenge legality/damage seam (see the
  `apply_challenge` doc comment in `src/domain/engine/reducer.rs`).
- [ ] Acceptance: each of the six alters challenge legality/damage per its §10
      definition (`tests/keywords.rs`).

### Slice 6b+ — remaining keywords (deferred, back-linked from `keyword.rs`)
- **Bodyguard "may enter play exerted"** (§10.3.2): a play-time choice — needs a
  small decision at play (deferred from 6a).
- **Reckless** (§10.7): (a) can't quest (the `apply_quest` TODO already notes
  this); (b) can't end the turn while a ready Reckless character *can legally
  challenge* an opposing exerted character or location. (b) needs a **shared
  "can this character legally challenge anything?" predicate** extracted from
  `apply_challenge` (so it respects Evasive/Bodyguard/Rush/exerted + locations);
  that same predicate should back a fully-correct Bodyguard "if able" check. See
  the TODO in `apply_end_turn` (`src/domain/engine/reducer.rs`). Locations as
  challenge targets arrive in Slice 7.
- **Support** (§10.13): quest trigger adding `{S}` to a chosen character (quest
  trigger + target choice + timed modifier).
- **Vanish** (§10.14) / **Ward** (§10.15): effect-targeting interactions (need
  targeted effects / choices — overlaps Slice 8).
- **Singer / Sing Together** (§10.11–12): songs — **Slice 7**.
- **Boost** (§10.4): put-card-under mechanic (and its trigger — see the bag
  rollout). **Shift** (§10.10): introduces the in-Play **card-stack** model
  (top/under/in-a-stack, §5.1.6–5.1.7).

Also still pending here: the **challenge/banish triggers** into the bag (see
[Trigger taxonomy rollout](#trigger-taxonomy-rollout-when-the-triggercondition-todo-gets-done)
and the `apply_challenge` / `game_state_check` hooks): "whenever this character
challenges / is challenged / banishes another in a challenge" / "when this
character is banished", plus the §1.9.1.3 "banished by that character" attribution.

**Acceptance (whole slice)**
- [ ] Each keyword has a passing scenario matching its §10 definition/example.
- [ ] Shift forms/moves stacks correctly; the stack moves with its top card on leave.

---

## Slice 7 — Songs, locations, movement

**Goal**: remaining card types.

- **Songs**: Action + "Song" classification; pay by exerting a character of
  sufficient cost (§6.3.3); interaction with Singer / Sing Together.
- **Locations**: play, move cost to move a character there (§4.3.7), willpower &
  banishment, start-of-turn lore (§6.5). Location characteristics (move cost,
  willpower, lore) become modifiable `Stat` variants in the continuous-effects
  layer — see the TODO on `Stat` in `src/domain/game/modifier.rs`.
- **Triggers** (see [Trigger taxonomy rollout](#trigger-taxonomy-rollout-when-the-triggercondition-todo-gets-done)):
  add sing-a-song and move-to-location / "while here" `TriggerCondition` variants
  with these mechanics.

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
- **Turn-progression-with-suspension** (carried over from Slice 5h): the engine
  needs a "what to do after the bag empties" continuation so a turn transition
  can resume after bag resolution suspends on a decision. This unblocks
  **start/end-of-turn triggers** (`TriggerCondition` Start/EndOfYourTurn) — see
  the back-linked TODOs in `begin_turn` / `apply_end_turn`
  (`src/domain/engine/reducer.rs`) and "Slice 5h" above. Likely also threads the
  registry through `start` / `apply_end_turn` / `begin_turn`.
- **Effect-driven leave-play removals** (return-to-hand, banish-by-effect, etc.):
  each MUST call `GameState::remove_modifiers_from_source` and then run a
  game-state check, exactly like the banishment path — otherwise the static /
  win-loss modification layers go stale (see the caveat on
  `remove_modifiers_from_source` and the `banish` comment in
  `src/domain/rules/game_state_check.rs`). Also: timed selector effects must
  **snapshot** their targets (§7.6.3 — TODO on `expire_end_of_turn_modifiers`).

**Acceptance**
- [ ] A worked replacement example from §7.7 reproduces exactly (ordering included).
- [ ] "Up to N" forbids duplicate picks and allows 0; "may" can decline cleanly.
- [ ] A delayed trigger ("at the end of your turn, …") fires at the right moment.
- [ ] An effect that returns/banishes a card removes its modifiers and a pending
      win/loss/banishment resolves on the next check (parallels the Donald case).
- [ ] A turn transition resumes correctly after a bag suspension, and a
      start/end-of-turn trigger fires (completes the deferred Slice 5h piece).

---

## Slice 9 — Real card data & conformance suite

**Goal**: scale beyond hand-written cards and lock in correctness.

- Bulk card-data loader mapping a community dataset (e.g. LorcanaJSON-style data) into
  our `CardDefinition`/DSL, or generate TOML from it.
- Definition validation on load (schema + DSL well-formedness).
- A conformance test suite: encode the rules examples (§7–§10) and a library of
  hand-authored interaction scenarios as golden tests.
- **Trigger taxonomy completeness** (see
  [Trigger taxonomy rollout](#trigger-taxonomy-rollout-when-the-triggercondition-todo-gets-done)):
  loading real cards forces any still-missing `TriggerCondition` variant and the
  scope-filter / event→trigger matcher to be finished and tested. The
  `TriggerCondition` TODO should be empty after this slice.

**Acceptance**
- [ ] A meaningful subset of a real set loads and validates.
- [ ] The conformance suite passes and runs in CI.
- [ ] No remaining items in the `TriggerCondition` TODO.

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
