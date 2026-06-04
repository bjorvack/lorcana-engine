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

Done since: self-scoped **sing-a-song** trigger (`WhenThisSings`, DSL `"sings"`) —
fires for each singer in `apply_sing`, resolved from the bag after the song's
effect. `tests/actions.rs::a_sing_a_song_trigger_fires_for_the_singer`. (The
"one of *your* characters sings" yours-scope variant is a follow-up.)

Also done: first **yours-scoped** trigger — `WhenYoursQuests` ("whenever one of
your characters quests", DSL `"yours_quests"`). `apply_quest` scans the
controller's in-play characters (including the quester) and routes each through
`enqueue_triggers_for_def`, so `during_your_turn` gating + granted triggers are
honored. `tests/conformance.rs::yours_quests_trigger_*` (fires for another
character, fires for the quester itself, ignores the opponent's quest).

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
- [x] **Start/End-of-turn triggers** — done in Slice 8b-9. `AtStartOfTurn`
      resolves in the Set step (§4.2.2.3), `AtEndOfTurn` in the End phase
      (§4.4.1). The turn transition is now resumable: if a trigger suspends on a
      `PendingDecision`, `begin_turn`/`apply_end_turn` return and
      `resume_turn_progression` finishes the remaining steps from the current
      `(phase, step)` once the decision is answered. Registry is threaded through
      `apply_mulligan`/`begin_turn`/`apply_end_turn`. Tested in
      `tests/turn_triggers.rs`.

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

### Slice 6b — shared challenge-legality authority + Reckless ✅
- [x] Single legality authority `can_challenge` (with `target_legal_basic` and
      `character_has_keyword`) in `src/domain/engine/reducer.rs` — used by
      `apply_challenge`, the Bodyguard "if able" check, and Reckless. It carries
      back-linked TODOs for the **effect-driven** challenge legality (see Slice 8:
      can't-challenge / can't-be-challenged / can-challenge-ready / granted
      keywords) and **locations** as targets (Slice 7).
- [x] **Reckless** (§10.7): (a) can't quest; (b) can't end the turn while a ready
      Reckless character can legally challenge (`reckless_must_challenge`, reusing
      `can_challenge`). Locations-as-targets still TODO (Slice 7). Tested in
      `tests/keywords.rs`.

### Slice 6c — Shift ✅ (standard + variants)
- [x] **Shift** (§10.10): an alternate **play** cost (`PlayCard { shift_onto }`)
      that puts the card on top of a valid in-play character, forming a **stack**
      (`CardInstance.under`). Same-name (via `CardDefinition.names`, multi-name
      ready), **Universal**, and **[Classification]** variants
      (`Keyword::Shift(ShiftAbility { cost, kind })`). The top inherits the
      underlying character's exerted/dry/**drying** state (§10.10.3–5) and damage
      (§10.10.7); shift *is* playing, so enters-play / play-a-category triggers
      fire. Leaving play **dissolves** the stack into separate cards in the
      destination zone (`CardInstance::dissolve`, §5.1.7). Tested in `tests/shift.rs`.
- Deferred (Slice 8, back-linked in `keyword.rs` / `ShiftCost` / reducer TODOs):
  alternate Shift costs (discard / free-from-discard) + cost reducers (Yokai),
  effect-granted names + Morph wildcard targeting, the §10.10.6 modifier-transfer,
  and shift-conditional triggers ("if you used Shift", 23 cards).

### Slice 6d — Boost ✅
- [x] **Boost** (§10.4): `Input::Boost { card }` pays the character's ink cost,
      once per turn (`GameState::has/mark/clear_boosted_this_turn`), to move the
      top deck card **facedown** under it (`CardInstance::push_under`) — the same
      stack model as Shift, so it dissolves out on leave-play (§5.1.7). Tested in
      `tests/keywords.rs`.
- [x] Boost's "**card put under this**" watcher trigger
      (`TriggerCondition::WhenCardPutUnder`, enqueued in `apply_boost`).

### Slice 6e+ — remaining keywords (deferred, back-linked from `keyword.rs`)
- [x] **Bodyguard "may enter play exerted"** (§10.3.2): a play-time choice —
      `PendingDecision::EnterPlayExerted` / `Decision::EnterExerted`, answered with
      `Decide` after the Bodyguard enters play; tested in `tests/keywords.rs`.
- [x] **Support** (§10.13) — done in Slice 8a-2 (`enqueue_support_trigger`): an
  optional quest trigger adds the source's **current** `{S}` (modifiers included,
  via `current_character_stats`, snapshot at quest time) to another chosen
  character as a flat `+N` `UntilEndOfTurn` modifier. Tested in `tests/support.rs`.
- **Vanish** (§10.14) / **Ward** (§10.15): effect-targeting interactions (need
  targeted effects / choices — overlaps Slice 8).
- **Singer / Sing Together** (§10.11–12): songs — **Slice 7**.

Challenge/banish triggers into the bag (see
[Trigger taxonomy rollout](#trigger-taxonomy-rollout-when-the-triggercondition-todo-gets-done)):
- [x] "whenever this character challenges / is challenged"
      (`WhenThisChallenges` / `WhenChallenged`, enqueued in `apply_challenge`).
- [x] "when this is banished" (`WhenBanished`) / "...in a challenge"
      (`WhenBanishedInChallenge`, Marshmallow/HeiHei) / "banishes another in a
      challenge" (`WhenBanishesInChallenge`) — enqueued in `apply_challenge` from
      the `game_state_check` banishment events (`enqueue_banish_triggers`). Still
      Slice 8: the matching **effects** (return-to-hand, to-inkwell) — which must
      move the card **from the discard** — the §1.9.1.3 "banished by that
      character" attribution, and centralizing `WhenBanished` for effect-driven
      (non-challenge) banishment.

**Acceptance (whole slice)**
- [ ] Each keyword has a passing scenario matching its §10 definition/example.
- [ ] Shift forms/moves stacks correctly; the stack moves with its top card on leave.

---

## Slice 7 — Songs, locations, movement

**Goal**: remaining card types.

### Slice 7a — Actions & Songs ✅
- [x] **Actions** (§6.3): `CardKind::Action` is playable — pay ink, resolve its
      `CardDefinition.action_effects` **directly** (not via the bag, §6.3.1.2),
      then discard (never in play). Effects triggered by the play go to the bag
      after (§6.3.4); the play-a-category matcher (`category_matches`) now keys off
      the played card's **definition**, so Action/Song watchers work.
- [x] **Songs** (§6.3.3): `Input::Sing { song, singers }` plays a song by exerting
      eligible dry/ready characters instead of paying ink — single singer (cost ≥
      song cost, Singer-adjusted, §10.11) or **Sing Together** combined cost
      (§10.12). Shares `resolve_action_play`. Clears the Slice 6 Singer/Sing
      Together deferral. Tested in `tests/actions.rs`.
- Uses the minimal `Effect` enum for now; the full effect DSL is Slice 8.

### Slice 7b — Locations & movement ✅ (core)
- [x] **Locations** (§6.5): `CardKind::Location { move_cost, willpower, lore }` is
      playable; enters play faceup/undamaged (no ready/exerted/drying, §5.1.13.3),
      with `LocationStats` denormalized onto the `CardInstance`. **Willpower
      banishment** (§6.5.5) shares the `banishable_cards` path; **Set-step lore**
      (§6.5.6) is granted in `begin_turn`.
- [x] **Movement** (§4.3.7): `Input::MoveCharacter { character, location }` pays
      the location's move cost and records `CardInstance.at_location`. Tested in
      `tests/locations.rs`.
- [x] Locations as **challenge targets** (§4.3.6.19–22): `target_legal_basic`
      accepts a location any time (never exerted, Evasive N/A); Bodyguard only
      restricts choosing a *character* (gated in `can_challenge`); damage math
      already gives 0-back for non-characters. Tested in `tests/locations.rs`.
  Reckless's "must challenge … or location" now works too, since
  `can_legally_challenge_anything` scans all opposing in-play cards.
- Deferred (back-linked): **modifiable** location stats (the `Stat` TODO in
  `src/domain/game/modifier.rs`); location **abilities** and move / "while here"
  **triggers** (the `apply_move` TODO + trigger rollout).

### Slice 7c — Items ✅
- [x] **Items** (§6.4): `CardKind::Item` is playable — enters play faceup/in play
      (no strength/willpower/drying) via `place_item`. Its activated abilities work
      the turn it's played (§6.4.3) since `apply_use_ability` accepts any in-play
      card. Tested in `tests/items.rs`. (Item static/triggered abilities ride the
      shared enter-play tail.)

**Acceptance**
- [x] A song can be sung by exerting an eligible character (Slice 7a).
- [x] Characters move to a location for its move cost; locations grant lore at Set.

---

## Slice 8 — Effects, targeting & choices

**Goal**: the effect/choice DSL and the trickiest resolution rules.

**Design decisions (scoping):**
- **Target model:** a `Target` enum carried by targeted `Effect` variants
  (`SelfCard`, `ChosenCharacter { side: Any/Opposing/Yours, another }`, …;
  classification/cost filters and "up to N" added incrementally).
- **Choosing:** targets are picked **at resolution** — a targeted effect sets
  `PendingDecision::ChooseTarget { player, options, effect }` and suspends;
  `Decision::ChooseTarget(card)` applies the stashed effect to the pick. Reuses
  the bag suspend/resume (as triggers / Bodyguard-enter-exerted already do) and
  fits triggered abilities (targets chosen as they resolve).
- **Sub-slices (smallest-first):**
  - [x] **8a-1 — self move-zone effects** (no choice): `Effect::ReturnToHand` /
    `IntoInkwell` with `Target::SelfCard`, threading the effect **source** into
    `execute_effect`. Unblocks the banish-trigger effects (Marshmallow / HeiHei
    "return this card to hand", Gramma Tala "into your inkwell"), relocating from
    the discard. Tested in `tests/challenge.rs`.
  - [x] **8a-2 — targeting + Support:** `Target::ChosenCharacter { filter,
    another }` + `AllCharacters`, a reusable `CharacterFilter { side,
    classifications }`, and `PendingDecision::ChooseTarget` (choose at resolution,
    via the bag). **Support** (§10.13) wired as an optional quest trigger carrying
    `GiveStrengthThisTurn { ChosenCharacter, amount = source's current {S} }`
    (so modifiers count). `Effect` is now non-`Copy` (filters hold classification
    strings). Tested in `tests/support.rs`. Target **filter dimensions** still to
    grow (cost/{S}/state, item/location/player, group-"other") — back-linked on
    `CharacterFilter`.
  - [x] **8b-1 — targeted damage effects + centralized banish triggers:**
    `Effect::DealDamage` / `RemoveDamage` (chosen / all / self), and a
    `game_state_check_with_triggers` wrapper at the effect-resolution sites
    (`execute_trigger`, `apply_decision`, `apply_use_ability`,
    `resolve_action_play`) so **effect-driven** banishment fires `WhenBanished`
    (clearing the centralization deferral). `ReturnToHand`/`IntoInkwell` to a
    *chosen* target also work now (move to the target's owner's zone). Tested in
    `tests/targeted_effects.rs`.
  - [x] **8b-2 — direct banish:** `Effect::Banish(Target)` ("banish chosen
    character") via `banish_by_effect` (registry/events threaded through
    `execute_effect`/`apply_effect_to`): dissolve to discard, end modifiers, emit
    `Banished`, enqueue `WhenBanished` (so move-zone banish effects compose).
    Tested in `tests/targeted_effects.rs`.
  - [x] **8b-3 — filter dimensions:** `CharacterFilter` gained **cost**/`{S}`
    numeric comparisons (`NumericFilter` / `Comparison` — "N or less/more/exactly")
    and **damaged**/**exerted** booleans; matched in `character_matches_filter`
    (cost from the printed def, `{S}` from current stats). Tested in
    `tests/targeted_effects.rs`.
  - [x] **8b-4 — targeted actions verified:** a single-effect targeted **action**
    suspends for the choice and resolves correctly. Tested in `tests/actions.rs`.
  - [x] **8b-5 — multi-effect sequence with suspension (§7.1.2):** `resolve_effects`
    resolves a `Vec<Effect>` in order; a mid-sequence target choice stashes the
    remaining effects as `ChooseTarget { rest }` and `Decide` resumes them (may
    suspend again); empty-target effects fizzle and the sequence continues. All
    effect-resolution sites (triggers, abilities, actions) route through it.
    Unblocks "[A] then [B]" cards — Improvise, Energy Blast, Distract, Glean, …
    (30+). Tested in `tests/actions.rs`.
  - [x] **8b-6 — item & location targets:** `Target::ChosenItem { side }` /
    `ChosenLocation { side }` ("banish chosen item", §6.4/§6.5) — eligible sets
    via `chosen_permanent_options` (an item is an in-play card that is neither a
    character nor a location). Compose with `Banish`/`ReturnToHand`/`DealDamage`.
    Tested in `tests/targeted_effects.rs`.
  - [x] **8b-7 — "up to N" (§7.1.8):** `Target::UpToCharacters { filter, max }` +
    `PendingDecision::ChooseUpToN` + `Decision::ChooseTargets(Vec<CardId>)`. The
    controller submits 0..max **distinct** eligible targets; the effect applies to
    each, then `rest` resolves. Unblocks Painting the Roses Red, Double Trouble,
    Gumbo Pot, … `Decision` is now non-`Copy`. Tested in `tests/targeted_effects.rs`.
  - [x] **8b-8 — name filter + group-"other":** `CharacterFilter.names` ("chosen
    character named X", matched via the def's `has_name`) and `AllCharacters {
    filter, another }` so "your *other* characters" excludes the source. Tested in
    `tests/targeted_effects.rs`.
  - [x] **8b-9 — start/end-of-turn triggers + turn-progression-with-suspension**
    (clears the Slice 5h deferral): `TriggerCondition::AtStartOfTurn` resolves in
    the Set step (§4.2.2.3), `AtEndOfTurn` in the End phase (§4.4.1), both via
    `enqueue_turn_triggers`. The turn transition is now **resumable** — if a
    trigger suspends on a decision, `begin_turn` / `apply_end_turn` return, and
    `resume_turn_progression` (called after `apply_decision` drains the bag)
    finishes the remaining steps from the current `(phase, step)`. `registry` is
    threaded through `apply_mulligan`/`begin_turn`/`apply_end_turn`. Tested in
    `tests/turn_triggers.rs` (start, end, and a "may" trigger that pauses then
    resumes the turn into Main).
  - [x] **8b-10 — conditional effects (board guard):** `Effect::IfControl {
    filter, then }` resolves `then` only if the controller has an in-play
    character matching `filter` ("if you have a character named X in play, …").
    `then` may itself be targeted (delegates through `execute_effect`). Tested in
    `tests/targeted_effects.rs`.
  - [x] **8b-11 — exert / ready effects:** `Effect::Exert(Target)` /
    `Ready(Target)` ("exert chosen opposing character" — 49; "ready this/chosen" —
    67) toggle the target's `ready` condition, composing with all target shapes.
    Tested in `tests/targeted_effects.rs`.
  - [x] **8b-12 — continuous property modifiers + granted keywords:** a
    `PropertyModifier` layer (granted `Keyword` / `Restriction` / `Permission`,
    parallel to `StatModifier`). `character_has_keyword` and effective Challenger/
    Resist OR in granted keywords; `Effect::GrantKeywordThisTurn`. Tested in
    `tests/keywords.rs`.
  - [x] **8b-13 — effect-driven challenge/quest legality:** `Restriction`
    (CantQuest/CantChallenge/CantBeChallenged) and `Permission` (ChallengeReady/
    ChallengeEvasive/ChallengeWhileDrying/QuestWhileDrying) are split types routed
    through unified `has_restriction` / `has_permission` authorities — granted by
    effect **or** implied by a keyword (Alert⇒ChallengeEvasive, Rush⇒
    ChallengeWhileDrying, Reckless⇒CantQuest). Preventions beat permissions
    (§1.2.2, verified). `Effect::RestrictThisTurn` / `PermitThisTurn`. Tested in
    `tests/restrictions.rs`. This completes the effect-driven-challenge-legality
    deferral (Tiana's Palace/The Wall etc. now need only a conditional-static source).
  - [x] **8b-14 — conditional on the chosen target:** `Effect::IfTargetMatches {
    target, filter, then, otherwise }` chooses `target`, then applies `then`/
    `otherwise` to the chosen card by whether it matches `filter` ("Chosen
    character gets +2; if a Villain, +3 instead"). Tested in
    `tests/targeted_effects.rs`.
  - [x] **8b-15 — effect-driven return-to-deck:** `Effect::ReturnToDeck { target,
    position: Top/Bottom/Shuffle }` via `move_self_card` (dissolves any stack,
    facedown `in_deck` conditions, `insert_bottom` for bottom, `shuffle_deck` for
    shuffle-in §8.2.4.1) and removes the source's modifiers on leave-play. Tested
    in `tests/targeted_effects.rs`.
  - [x] **8b-16 — damage prevention:** `Restriction::TakesNoChallengeDamage` (a
    §7.7 damage replacement) zeroes challenge damage to a recipient
    (`combat_damage`), granted via `RestrictThisTurn` ("takes no damage from
    challenges this turn"). Tested in `tests/restrictions.rs`. NB: the "from **the**
    (current) challenge" variant (Raya/Peter Pan) still needs replacement timing
    (resolve the challenge trigger before damage) — deferred.
  - [x] **8b-17 — conditional static abilities (foundation):** a `Condition`
    (first: `SourceExerted`) gates `StatModifier` / `PropertyModifier`
    (`with_condition`), evaluated on demand by `GameState::condition_holds`; the
    stat/keyword/restriction/permission queries skip modifiers whose condition
    fails. `StaticAbility` carries it ("while this character is exerted, …").
    Tested in `tests/modifiers.rs`. Grows with more conditions (stat thresholds,
    "while here", "while you have a … in play") + richer static targets (names /
    at-location) to fully cover Tiana's Palace / The Wall / Kenai.
  - [x] **8b-18 — delayed triggers (§7.4.7):** `Effect::ScheduleDelayed { when:
    DelayedWhen::EndOfTurn, effect }` stores a one-shot `DelayedTrigger` in state;
    `apply_end_turn` enqueues those due (alongside the AtEndOfTurn triggers) so
    they resolve via the bag + resumable turn transition. Tested in
    `tests/turn_triggers.rs`. (More `DelayedWhen` variants — start-of-next-turn —
    grow from here.) Clears the "delayed trigger fires" acceptance.
  - **8b+ —** remaining: more `Condition` / `DelayedWhen` variants + static
    targets, **player** targets, the §7.7 "from the current challenge" timing +
    full replacement ordering, §1.9.1.3 attribution, and modifiable location stats.

### Slice 8b+ — harder resolution rules
- Replacement effects (§7.7): "instead"/"skip"/"enter"; self-replacement applied
  first; "same replacement can't apply twice"; replacement of steps/phases.
- Choice machinery completeness: "may" (§7.1.3), "up to N" (§7.1.8, no duplicates),
  ordering simultaneous discards/destinations, "that [game term]" resolution (§7.1.9).
- Floating & delayed triggered abilities (§7.4.7).
- [x] **Turn-progression-with-suspension** (was carried over from Slice 5h) — done
  in Slice 8b-9: `resume_turn_progression` finishes a turn transition that
  suspended on a start/end-of-turn trigger, and `AtStartOfTurn`/`AtEndOfTurn` are
  wired (registry threaded through `apply_mulligan`/`begin_turn`/`apply_end_turn`).
- **Effect-driven leave-play removals** (return-to-hand, banish-by-effect,
  return to **top or bottom of deck**, etc.): each MUST
  (a) **dissolve any stack** via `CardInstance::dissolve(<destination zone
  default conditions>)` so a Shift/Boost stack becomes separate cards in the
  destination — faceup for hand/discard, **facedown** (`Conditions::in_deck()`)
  for the deck, using `Zone::insert_bottom` for deck-bottom; §8.2.4.1 lets a
  shuffled-in stack's cards be freely ordered (RNG via the seeded rng);
  (b) call `GameState::remove_modifiers_from_source` and then run a game-state
  check, exactly like the banishment path — otherwise the static / win-loss
  modification layers go stale (see the caveat on `remove_modifiers_from_source`
  and the `banish` comment in `src/domain/rules/game_state_check.rs`). Also:
  timed selector effects must **snapshot** their targets (§7.6.3 — TODO on
  `expire_end_of_turn_modifiers`).
- **Effect-driven challenge legality** plugs into the single legality authority
  in `src/domain/engine/reducer.rs` (carries the back-linked TODOs):
  - challenger "can't challenge" effects (Frying Pan, Cobra Bubbles, Gantu) →
    `can_challenge` challenger side;
  - target "can't be challenged" (Tiana's Palace, The Wall, Panic) and the
    challenger's "can challenge ready characters" permission (Pick a Fight) →
    `target_legal_basic`;
  - **effect-granted keywords** ("gains Alert/Challenger…", Cri-Kee, Inkrunner,
    But I'm Much Faster) → `character_has_keyword` must OR in granted keywords.

**Acceptance**
- [ ] A worked replacement example from §7.7 reproduces exactly (ordering included).
- [x] "Up to N" forbids duplicate picks and allows 0; "may" can decline cleanly
      (Slice 8b-7 `tests/targeted_effects.rs`; "may" via `MayResolve`).
- [x] A delayed trigger ("at the end of your turn, …") fires at the right moment
      (Slice 8b-18, `tests/turn_triggers.rs`).
- [x] An effect that returns/banishes a card removes its modifiers and a pending
      win/loss/banishment resolves on the next check (parallels the Donald case)
      (Slice 8b-2/8b-15; `tests/win_loss_modifiers.rs` effect-banishes-Donald).
- [x] A turn transition resumes correctly after a bag suspension, and a
      start/end-of-turn trigger fires (Slice 8b-9, `tests/turn_triggers.rs`).

---

## Slice 8c — Card-coverage gap inventory (express every known card)

**Goal**: every printed card's text is expressible in the DSL. Grounded in a
2,610-card corpus (Lorcast, sets 1–12 + promos; 2,314 with rules text). Effect
mechanics ranked by card count, with the remaining gaps to close in order:

- [~] **discard** (288) — `Effect::Discard(DiscardAmount::Count(n)/WholeHand)`:
      the controller chooses N from hand (`PendingDecision::ChooseCardsToDiscard` /
      `Decision::DiscardCards`) or discards the whole hand outright. Done for
      **you**; `tests/targeted_effects.rs`. **Remaining:** "each opponent
      chooses/discards N" (opponent is the chooser) and at-random.
- [~] **play a card from a zone** (147) — `Effect::PlayFreeFromHand { filter }`
      plays an eligible hand card for free (`PendingDecision::ChoosePlayFree`;
      characters/items/locations enter play, actions resolve + discard). Optionality
      is composed via the new `Effect::May(Box<Effect>)` wrapper (one yes/no
      `MayResolveEffect`, reusable by any effect) rather than a per-effect flag.
      `tests/targeted_effects.rs`. **Remaining:** play from **discard**,
      cost-reduction (pay N less) alternate costs, free-played Bodyguard enter-exerted.
- [x] **Ward / can't be chosen** (§10.15) — modeled as `Restriction::CantBeChosen`
      (Ward keyword maps to it via `has_restriction`, so effect-granted Ward works
      too). Targeting splits into `matching_characters` (raw) and
      `choosable_characters` (matching minus what an opponent can't choose); only
      the *choosing* targets (`ChosenCharacter`/`UpToCharacters`) use the latter, so
      Ward blocks *choosing* but not *all-characters* effects, and never your own
      controller (§1.2.3). Challenges go through `can_challenge`, unaffected.
      Conformance: `tests/keywords.rs` (choose/all/own/up-to) + `tests/actions.rs`
      (§1.2.3 "deal damage to chosen char, draw" with a Warded target still draws).
      (Ward on items/locations: minor follow-up — needs registry in
      `chosen_permanent_options`.)
- [~] **search / look at top N** (59) — `Effect::LookAtTopAndTake { count, filter,
      rest }`: look at the top N, take **up to one** matching card to hand
      (`PendingDecision::ChooseFromRevealed` / `Decision::TakeRevealed`), the rest go
      to `rest` (top/bottom/shuffle). Covers Be Our Guest / Ariel / Develop Your
      Brain. `tests/reveal.rs`. **Remaining:** take >1, reorder-in-any-order of the
      rest, split top+bottom (Dr. Facilier), search the whole deck (tutor, 9).
- [x] **reveal (opponent's hand) / discard from it** (Lenny, Timon, Goldie) —
      `Effect::OpponentDiscardsChosen { whose, filter }`: resolve the (chosen)
      opponent via `PlayerScope` (prompting in multiplayer), then the controller
      picks a card matching `filter` from their hand via the `Choose` primitive
      (`ChoiceThen::DiscardFrom { owner }`). Filters reuse the algebra:
      `Category(Action)` (Lenny), `Not(Category(Character))` (Timon),
      `Category(Location)` (Goldie). Reveal is implicit (hand is known to the
      engine). `tests/opponent_discard.rs`.
- [x] **random discard / reveal hand** — `Effect::Discard` carries `by: DiscardBy`
      (`Owner` chooses, default; `Random` picks uniformly via the seeded RNG, no
      choice) for "discard a card at random" (Dangerous Plan / Lady Tremaine /
      Bruno). `Effect::RevealHand { whose }` emits a `HandRevealed { player, cards }`
      information event (Dolores / Copper / Nothing to Hide); reveal-and-pick emits
      it too. Reveal is **event-only** — the engine is full-information, so there's
      no persistent "revealed" state. `tests/opponent_discard.rs`.
- [~] **freeze / "can't ready"** (38) — modeled uniformly as
      `Restriction::CantReady` (every card action goes through restrictions): the
      ready step skips cards that have it. `Effect::Freeze(Target)` adds it with a
      new general duration `ModifierDuration::UntilStep { step, player }` (the
      `NextStep(step, player)` idea) = consumed when that controller next readies;
      survives end of turn. Continuous "can't ready" (Vincenzo) is the same
      restriction with a `WhileSourceInPlay` modifier. `tests/turn_triggers.rs`.
      **Remaining:** exert+freeze on the same chosen target (needs multi-effect-
      per-target); continuous can't-ready statics; Anna-style is covered.
- [~] **dynamic amounts — "+N for each / equal to"** (94 + 40) — a uniform
      `Amount` enum (`Fixed` | `PerMatchingCharacter(filter)` | `StatOf { stat,
      target }`) now backs every numeric effect field (`DrawCards`, `GainLore`,
      `EachOpponentLosesLore`, `GiveStrengthThisTurn`, `DealDamage`, `RemoveDamage`),
      evaluated at resolution via `eval_amount`. `StatOf` reads the source
      (`SelfCard`) or the resolved target's `{S}/{W}/{L}`, so it composes "your own
      / their / another's stat" — and Support now uses `StatOf{Strength,SelfCard}`,
      so chained Support buffs add the **combined** value (§7.8). Tested:
      `tests/targeted_effects.rs` (damage = number of your characters),
      `tests/support.rs` (chained Support). **Remaining (dynamic STATICs):**
      continuous "+1 {L} for each Villain" (Hades) needs a count-based
      `StatModifier` delta (today fixed); "for each card in zone" counts; cost
      reductions.
- [~] **player-scoped effects** — `PlayerScope { You, EachOpponent, EachPlayer }`
      backs `Effect::Discard { who, amount }`: each player in scope discards,
      sequenced (each is its own `ChooseCardsToDiscard`, the *discarding* player
      chooses; whole-hand needs no choice). Closes the deferred "each opponent
      chooses and discards" (35). `tests/targeted_effects.rs`.
  - [x] **choose-a-player axis (multiplayer-ready)** — `PlayerScope` gains
    `ChosenOpponent` / `ChosenPlayer` (+ resolved `Player(id)`). `resolve_scope`
    auto-resolves a single candidate (2-player "chosen opponent") and otherwise
    emits a `PendingDecision::ChoosePlayer` (3–4 player games); `Decision::
    ChoosePlayer` re-targets the effect onto the chosen player. Wired into
    `Discard`. `tests/multiplayer.rs` (4-player prompts; 2-player auto-resolves).
    **Remaining:** player-scoped **draw/lore** onto `PlayerScope` ("each/chosen
    player draws", "chosen opponent loses lore").
  - [x] **unified zone move + mill** — `Effect::Move { what: MoveSource, to:
    Destination }` is the single card-move primitive: `MoveSource::Card(Target)`
    (bounce / into-inkwell / return-to-deck — replaces the old `ReturnToHand`,
    `IntoInkwell`, `ReturnToDeck`) and `MoveSource::DeckTop { who, count }`
    (milling / digging). `Destination = Hand | Inkwell | Discard | Deck(pos)`.
    Mill = `Move { DeckTop, Discard }`, threads `PlayerScope` (so "top N of chosen
    player's deck into their discard" works in multiplayer). `tests/multiplayer.rs`.
    **Remaining:** other-player look-at-top (scoped `LookAtTopAndTake`).
- [x] **dynamic continuous statics** — `StatModifier`/`StaticAbility` gain
      `per: Count` (registry-free `ControlledCharacters`/`CardsInHand`/`DamageOnSelf`);
      effective delta = `delta × count`, evaluated live in `stat_delta`. Hades /
      Jafar / Minnie. `tests/modifiers.rs`.
- [x] **move damage** (113) — `Effect::MoveDamage { from, to, amount }`: up to N
      counters from one character to another (one side `SelfCard`, other chosen),
      capped by `from`'s damage; lethal banishes. `tests/targeted_effects.rs`.
      Two-chosen (Belle/Alma) deferred.
- [x] **name a card** (6) — `Effect::NameThenReveal { lore_on_match, match_to,
      otherwise_to }` + `Decision::NameCard(String)`: name, reveal top, branch on
      match (Merlin / Bruno / Sorcerer's Hat). `tests/reveal.rs`.
- [x] **grant an ability** (10) — `Effect::GrantAbilityThisTurn { target,
      condition, effect, optional }` adds a `GrantedTrigger` (until end of turn)
      that fires from the target alongside its printed triggers (Hero Work /
      Megara). `tests/granted.rs`. Granted **activated** abilities + "Blast"-style
      name-from-discard deferred.

Costs (activated-ability "discard a card", "{E}" etc.) ride the AbilityCost atom
work, tracked separately.

---

## Card-functionalization (post-Slice-9, ongoing)

Turning the loaded card pool into functional cards, driven by a recurring
expressibility triage. Latest triage: ~40% of cards fully functional (vanilla +
keyword-only + authored/expressible); ~300 cards now have authored abilities
across sets 1–12 + promos (multiple parallel worktree passes). Top remaining
blockers: look-at-top/reveal (~180), modal "choose one" (~80).

- [x] **trigger-context amounts ("that much" / "that many")** — `Amount::TriggerAmount`
  (DSL `"that much"` / `"damage dealt"`); the firing site substitutes the concrete
  value into the enqueued effect via `Effect::with_trigger_amount`, so the bagged
  effect carries a constant (resolution pipeline untouched). Wired for damage
  triggers (`WhenThisIsDealtDamage`); authors Hydra - Deadly Serpent's WATCH THE
  TEETH (set 3). `tests/conformance.rs::watch_the_teeth_deals_back_the_damage_just_taken`.

- [x] **DSL selector predicates** — `parse_filter` parses by-name / by-cost /
  by-stat-threshold (`"named X"`, `"with cost N or less"`, `"with N {S} or more"`),
  adding `CharacterFilter::Willpower`/`Lore`.
- [x] **Authored abilities for sets 1-12 + promos** (~300 cards) via parallel
  worktrees; action/song play-abilities route to `action_effects`.
- [x] **Permanent keyword/property grants** — `Effect::Grant { target, property }`
  + `ModifierDuration::Permanent` (cleared when the target leaves play); DSL
  `grant_keyword … duration = "permanent"`.
- [x] **Count-threshold conditionals** — `IfControl` gains `at_least: u32`
  ("if you have N or more …"); DSL `if_you_have = "<filter>", at_least = N`.
- [x] **DSL exposures of existing engine effects** — `move_damage = N, from, to`
  (`Effect::MoveDamage`); `restrict = "cant_quest"|"cant_challenge"|… ` (granted
  `Property::Restriction`); and a proper `duration = "next_turn"`
  (`Effect::GrantNextTurn`, `UntilStep{Ready, owner}` — the "at the start of their
  next turn" timing, mirrors freeze).
- [ ] **look-at-top/reveal variants** (expose/extend `LookAtTopAndTake` +
  shuffle/reveal/search), **modal "choose one"**, trigger-context amounts; ongoing
  authoring passes as each feature lands.

## Slice 9 — Real card data & conformance suite

**Goal**: scale beyond hand-written cards and lock in correctness.

- [x] **TOML card-data loader** (`load_toml`) — the engine's own committed format
  (`cards/*.toml`) → `CardDefinition`, validated on load (type/stats/keywords).
  Authored by us; external datasets (Lorcast) are research-only and never loaded.
  Covers printed characteristics + keywords (values inline); text-based abilities
  via the effect DSL are a separate concern. authored inline in tests,
  `tests/card_loader.rs`.
- [~] **Effect-DSL authoring (first cut)** — `[[card.abilities]]` author **triggered**
  abilities in TOML, mapped to the `Effect` AST. Hybrid surface: structured verb
  tables (`{ draw = 1 }`) + `do = [..]` sequences (-> `Effect::All`), with leaf
  **selectors** as compact strings (`"chosen opposing character"`, `"each
  opponent"`) *or* the structured AST form as a fallback. Verbs covered: draw,
  gain/lose lore, deal/remove damage, give-strength, banish/exert/ready/freeze,
  discard, grant-keyword. Added `Effect::All` (sequencing) to the engine.
  `src/domain/cards/dsl.rs`, authored inline in tests, `tests/card_loader.rs`. Played
  end-to-end (TOML -> registry -> engine).
- [~] **Effect-DSL: activated + static abilities** — `[[card.activated]]`
  (`cost = { exert, ink }` + `do`) -> `ActivatedAbility`; `[[card.statics]]`
  (`strength/willpower/lore = N`, `to = "your other Hero characters"`) ->
  `StaticAbility`. Beast's Mirror ({E},1 -> draw) and Hercules (+1 {S} to other
  Heroes) in `tests/card_loader.rs`.
- [~] **Effect-DSL: dynamic amounts + conditionals + static per/while** — amounts
  accept `"per <filter>"` (-> `PerMatchingCharacter`), `"cards in hand"`,
  `"damage on self"`, `"<stat> of self"`, or the structured form, anywhere an
  integer was allowed; `{ if_you_have = "<filter>", then = {..} }` -> `IfControl`;
  statics take `per = "<filter>"` and `while = "exerted"`. Maleficent (conditional
  + for-each lore) and Cruella (static `per` + `while`) in `tests/card_loader.rs`.
  **Next:** more triggers/verbs as cards force them; conformance suite.
- A conformance test suite: encode the rules examples (§7–§10) and a library of
  hand-authored interaction scenarios as golden tests.
- **Trigger taxonomy completeness** (see
  [Trigger taxonomy rollout](#trigger-taxonomy-rollout-when-the-triggercondition-todo-gets-done)):
  loading real cards forces any still-missing `TriggerCondition` variant and the
  scope-filter / event→trigger matcher to be finished and tested. The
  `TriggerCondition` TODO should be empty after this slice.

**Acceptance**
- [x] A meaningful subset of cards loads and validates (`tests/card_loader.rs`).
- [~] The conformance suite passes and runs in CI — `tests/conformance.rs` holds
  rule-cited (§7–§10) **end-to-end** golden tests; every card is authored in the
  TOML DSL and loaded, so they exercise loader → engine. Runs under the existing
  CI `cargo test`. Covers §7.1.2 ordering, §8 bounce, §9 lethal damage, and
  §7.4 "whenever you play a [category]", into-inkwell, and keywords
  §10.2/10.3/10.5/10.6/10.7/10.8/10.9/10.15. DSL trigger surface gained
  `play_action`/`play_song`/`play_character`/… (`WhenYouPlay`). Growing.
- [ ] No remaining items in the `TriggerCondition` TODO.

---

## Slice 10 — Playable host & robustness

Turn the rules library into something usable + hardened.

- [x] **Public API facade** (`application::Game`) — `new`, `from_decks` (validate
  decks §2.1.1 → expand → start, returning `SetupError`), `submit`, `state`/
  `status`/`pending`, and `legal_actions()` (the engine's first action
  *enumeration*; it validates by trying each candidate on a clone, so it can't
  drift from `apply`). Pending-decision answers read from the decision; mulligan +
  turn moves enumerated. `tests/api.rs`, `tests/play_from_decks.rs` (incl. the
  invariant: every reported action is accepted). **Next:** multi-pick &
  Shift/Sing enumeration; perspective-aware state view.
- [x] **CLI host** (`application::host` + `src/main.rs`) — `render` (state + numbered
  legal actions), an interactive stdin loop, a deterministic `demo` auto-play
  (`cargo run -- demo [seed]`), and **`play <d1.txt> <d2.txt> [seed]`** which builds
  the combined card pool from `cards/sets/`, loads two decklists, and auto-plays a
  full real game (`registry_from_dir`/`play_from_files`). `tests/cli_demo.rs` +
  `tests/play_from_decks.rs` run full games to completion.
- [x] **Self-play / fuzz** — `tests/self_play.rs` (25 seeds, tiny decks) **and**
  `tests/self_play_official.rs` (30 full games across the real official decklists +
  matchups): every reported-legal action accepted, no panics, 60-card conservation
  per player, no win-threshold-without-game-over, most games finish.

## Slice 11 — Deck construction, decklists & behaviour auditing

- [x] **Card metadata for deck-building/display** — `CardDefinition` gains
  `ink_types` (1–2; dual-ink commits both colours), `max_deck_copies` (override of
  the default 4 — Dalmatian Puppy 99, The Glass Slipper 2), `image` (URL), and
  `text` (printed rules text). All optional TOML fields; backfilled pool-wide from
  the research dump (`cards/scripts/backfill_meta.py` / `backfill_text.py`) without
  disturbing authored abilities. Punctuation normalized to ASCII (apostrophes/NBSP;
  accents kept) + `Heihei→HeiHei`; baked into `from_lorcast.py`.
- [x] **`Deck` + validation** (`domain::deck`) — a deck is `[(CardDefId, count)]`;
  `validate(&registry)` enforces §2.1.1 (≥60 cards, ≤2 ink types, per-**full-name**
  copy limit with overrides), `expand()` for `GameState`, and `from_text`/`to_text`
  for the community `count name` share format (printing-lossy; printings collapse
  by name). `tests/deck.rs`. `CardRegistry` gained `iter()`/`find_by_name()` and
  `load_toml_from` (unique ids across files → one combined cross-set registry).
- [x] **Official decklists** — all 21 starter decks for sets 1–10 (Mushu Report
  wiki), stored under `decks/` as `count name` text. `tests/official_decks.rs`
  validates each resolves against the combined pool, is exactly 60, and is legal.
  Surfaced + fixed a card-pool gap (5 lore-less locations; generator no longer
  skips them) — pool is complete vs the dump (unique-name diff = 0).
- [x] **Behaviour audit log** (feature `audit-log`, off in release) —
  `application::audit::play_and_log`/`audit_from_files` emit a transcript pairing
  each acting card's printed `text` with the events it produced (CardId tokens
  resolved to names), for AI/human review of "did the card do what it says".
  `cargo run --features audit-log -- audit <d1> <d2> [seed]`; `tests/audit_log.rs`
  (cfg-gated). Generated logs are git-ignored.

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

## Composability refactors (in progress)

Survey found bespoke code that the same "small algebra + general continuation"
pattern can collapse. Tracked and done one at a time:

- [x] **#1a CharacterFilter → boolean algebra** — `enum { Any, Side, Classification,
      Named, Cost, Strength, Damaged, Exerted, IsSource, IsCard, And, Or, Not }`
      with a recursive `eval_filter`. `another` is now sugar (`with_another` →
      `filter.exclude_source()` = `And([.., Not(IsSource)])`); the same exclusion
      predicates also express "not the already-chosen card". `tests/targeted_effects.rs`
      (`filter_algebra_or_composes`).
- [x] **`another` removed** — folded entirely into the filter; all exclusions are
      `Not(IsSource)` / `Not(IsCard)` via the algebra (no flag/helper/`options.retain`).
      Recorded as a required rule in `AGENTS.md` ("Composable algebras").
- [x] **#2 collapse `Grant*ThisTurn` → `GrantThisTurn { target, property }`**.
- [x] **#3 fold `Count` into `Amount`** — denormalized cost + names onto
      `CardInstance` so the filter is registry-free; one `GameState::matches_filter`
      / `eval_amount` now serves both the reducer and dynamic statics, and `Count`
      is gone (`ControlledCharacters` → `PerMatchingCharacter(filter)`, plus
      `CardsInHand`/`DamageOnSource`).
- [x] **#4 fold `PlayFilter` into the filter algebra** — added
      `CharacterFilter::Category(CardCategory)`; removed `PlayFilter`. One vocabulary
      for in-play/hand/deck: in-play derives category from the instance
      (registry-free `matches_filter`), hand/deck evaluate the printed predicates
      against the definition (`def_matches_filter`, reducer already has the
      registry). No `GameState::new` change needed.
- [x] **#1b general `Choose { options, min, max, then }`** — all 7 former bespoke
      choices (ChoosePlayer, ChooseMoveTarget, ChooseTarget, ChooseUpToN,
      ChoosePlayFree, ChooseFromRevealed, ChooseCardsToDiscard) are now one
      `Choose` over `ChoiceRef = Card | Player` with five `ChoiceThen`
      continuations (SubstituteAndResolve, ApplyToEach, PlayFree, TakeRevealed,
      Discard). One `apply_choose_decision` + `choice_to_pending`.
- [~] **#5 unify `Target`/`PlayerScope`; remove `substitute_*`** — **declined after
      assessment.** `Target` (card-shaped: filters/all/up-to/items/locations) and
      `PlayerScope` (player-shaped: You/Each*/Chosen*/Player) are structurally
      different; a unified type would add "invalid-here" variants — *more* specific
      code. Their real shared point already exists: `ChoiceRef = Card | Player`
      inside `Choose`. And `substitute_*` implement the clean "substitute the pick
      into the effect AST and re-resolve" pattern (reusing `execute_effect`);
      removing them would duplicate per-effect resolution. Net: would worsen the
      code, so not pursued. **However**, revisiting it after #4 surfaced a real,
      bounded win in the same spirit: `Target::ChosenItem`/`ChosenLocation` are now
      one `ChosenPermanent { filter }` (using the `Category` predicate), and the
      parallel `chosen_permanent_options`/`PermanentKind` path is deleted —
      item/location targeting now goes through the unified filter algebra like
      everything else.

All previously-deferred card features are now done: granted **activated** abilities
(`Effect::GrantActivatedThisTurn`, `tests/granted.rs`) and **Blast from Your Past**
(`Effect::NameThenRecur`, `tests/reveal.rs`).
