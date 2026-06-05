# Self-Play AI Plan for the Lorcana Engine

> **Status:** research / design (no code yet). This document captures the
> architecture, the decisions and their rationale, the engine prerequisites, and
> a sliced delivery plan for adding a strong, fast, self-play-trained AI to
> `lorcana-engine`. It is written to match the existing engine design
> (`docs/architecture/ARCHITECTURE.md`) and slice workflow
> (`docs/planning/IMPLEMENTATION_PLAN.md`).

## 1. Goal & honest framing

**Goal.** A *smart and fast* AI that can continue play from any legal state,
choose the best available move (including "do nothing useful → pass/end turn"),
and be trained primarily by **self-play**. Targets: **Player vs AI** and
**AI vs AI** matches. (A later, out-of-scope project: a second AI that builds
decks and hands them to the engine for AI-vs-AI evaluation.)

**Honest framing — this is a research-frontier target.** Lorcana is an
**imperfect-information, stochastic** game (hidden hands, hidden/ordered decks,
facedown inkwell, shuffles, random discards). The famous self-play successes for
*perfect*-information games (AlphaZero in chess/Go) **do not transfer directly**:
their MCTS assumes a fully observable state. The systems that have actually beaten
human professionals at hidden-information games — DeepStack, ReBeL, Player of
Games — rely on **counterfactual subgame solving** and are substantially heavier
to build.

We therefore commit to the **strongest *practical* architecture** that the engine
can support today, with a clearly staged path:

1. A non-learned **heuristic + search baseline** (ships value immediately; serves
   as the evaluation gate and bootstrap opponent).
2. **Information-Set MCTS (IS-MCTS) guided by a self-play-trained policy/value
   network** — an AlphaZero-style loop adapted to hidden information via
   *determinization* and *belief sampling*. This is the v1 "smart" AI.
3. **(Stretch / future)** subgame-solving (ReBeL / Player-of-Games style growing-tree
   CFR with a value net) for genuine champion-level imperfect-info play.

Each stage is independently shippable and measurably better than the last.

### Non-goals (this document)
- Deck construction / the meta-optimizer AI (separate future project).
- A general-purpose RL framework. We build exactly what this engine needs.
- Changing the engine's rules semantics. The AI consumes the engine; it does not
  alter game logic.

## 2. Why the engine is a strong foundation

The engine already provides almost everything a self-play trainer needs:

| Need | Already in the engine |
|------|-----------------------|
| Reproducible rollouts | `GameState` owns its `SeededRng` (ChaCha8) and is `Clone` + `Serialize`/`Deserialize` (`domain/game/state.rs`). Same seed + inputs ⇒ identical state. |
| Uniform action type | `Input` enum (`PlayCard`, `Quest`, `Challenge`, `Sing`, `UseAbility`, `EndTurn`, …) and `Input::Decide(Decision)` for mid-resolution choices (`domain/engine/input.rs`). |
| Legal-move enumeration | `Game::legal_actions()` validates candidates by trying each on a clone, so it can't drift from `apply` (`application/api.rs`). |
| Self-play skeleton | `tests/self_play.rs` + `tests/self_play_official.rs` already drive random legal actions to completion across seeds with invariant checks. |
| Terminal reward | `GameStatus::Finished { winners }`; dense shaping from `lore()` / `lore_to_win()`. |
| Pre-wired workspace | `Cargo.toml` explicitly anticipates *"a future `lorcana-ai` crate for self-play/training against the same core."* |

**Implication:** most of the work is the *agent* (encoding, search, net, training
loop) plus a few **engine extensions** (§4), not a rewrite.

## 3. The hard problems (these drive every design choice)

1. **Imperfect information.** Hands, decks, and facedown inkwell are private (per
   the zone-visibility table in `ARCHITECTURE.md`). Consequences:
   - The agent must **never** see hidden zones. We need a **perspective-redacted
     observation** of `GameState` (flagged as a *Next* item in Slice 10 of the
     implementation plan: "perspective-aware state view").
   - Search must reason over an **information set** (the set of true states
     consistent with what the player has observed), not a single known state.

2. **Stochasticity / chance nodes.** Draws, shuffles, and random discards are
   random. Because the RNG lives *inside* `GameState`, a single clone replays
   **one fixed future** — convenient for determinism, but for unbiased search we
   must **resample** hidden zones/orderings (determinization), not peek at the
   real deck order.

3. **Structured, variable, sequential action space.** A turn is many `Input`s, and
   resolving one ability can demand a chain of `Decide`s (the bag order, "you may",
   choose-target, multi-pick, name-a-card). There is **no fixed-size discrete
   action head** that fits. The natural fit on this engine is **candidate
   scoring**: call `legal_actions()`, encode each candidate, and let the policy
   net score them (pointer/attention over a variable candidate set). This sidesteps
   an enormous fixed action vocabulary and stays correct as new cards are added.
   - **Helpful simplification:** Lorcana has **no MTG-style stack or response
     window** — only the active player acts. So at action-decision time there are
     **no simultaneous moves**; the game-theoretic difficulty is purely the hidden
     information, not concurrent decisions.

4. **Long horizon & credit assignment.** Games run to 20 lore over many turns,
   with a single sparse win/loss at the end. We mitigate with the MCTS value
   target (AlphaZero-style) and optional lore-based reward shaping.

## 4. Engine prerequisites (must land before/with the AI)

These are real gaps found in the current code; the AI is bounded by them.

- [ ] **Complete legal-action enumeration.** `legal_actions()` is explicitly
  "best-effort": multi-pick `Choose` decisions and Shift/Sing/inkwell-target
  combinations are only partially enumerated, and open-ended `NameCard` /
  `NameThenRecur` decisions yield **no** candidates (`application/api.rs`,
  `decision_actions`/`choose_actions`). An agent can only be as good as the action
  space it can see. **Action:** finish enumeration (or define a canonical
  candidate-generation policy for the combinatorial cases, e.g. top-K target
  subsets), and provide named-card candidates from the registry/observable
  context.
- [ ] **Perspective-redacted observation.** A read-only view of `GameState` from a
  given `PlayerId`'s seat that hides opponents' hands, both decks' ordering, and
  facedown inkwell faces, while exposing counts and all public zones (play,
  discard), lore, ink totals, turn/phase/step, and the current `PendingDecision`.
  This is required for **fair self-play, fair PvAI, and correct search** (the agent
  must not learn to exploit hidden info it shouldn't see).
- [ ] **Determinization / resampling hook.** Given an observation, produce a
  *sampled concrete `GameState`* consistent with it: shuffle the unseen cards
  (opponent hand + both decks) into plausible positions, respecting known
  constraints (revealed cards, known counts, cards seen earlier). Start uniform;
  later condition on a learned belief (§6.4). Needs a way to construct a
  `GameState` from "public state + a sampled hidden assignment" — feasible because
  card instances are deterministic `CardId`s and zones are ordered `Vec`s.
- [ ] **Fast clone / cheap rollout.** MCTS clones state per simulated step.
  Benchmark `GameState::clone()` cost and games/sec first (§7). If clone is hot,
  options: arena/CoW zones, a structural diff/undo log, or a slimmer "search state"
  projection. Measure before optimizing.
- [ ] **(Nice to have) batched stepping / no-alloc hot path** to feed many
  parallel self-play games efficiently.

These extend the existing `application` facade and `domain/game` types; none change
rules semantics.

## 5. Algorithm: staged plan

### Stage 0 — Heuristic baseline (no training)
A hand-tuned static evaluation (board lore, on-board strength/willpower, card
advantage, ink available, tempo) plus **1–2 ply search with random/heuristic
rollouts** over `legal_actions()`. Cheap, fast, immediately useful as:
- the default "easy/medium" opponent for PvAI,
- the **evaluation gate** every learned model must beat,
- the bootstrap opponent for early self-play (before the net is competent).

### Stage 1 — IS-MCTS + self-play-trained net (the v1 "smart" AI)
AlphaZero-style loop adapted for imperfect information:

- **Search:** Information-Set MCTS. Each search runs over **determinizations**
  sampled from the current observation (Perfect-Information Monte Carlo / IS-MCTS):
  sample N plausible hidden states, run guided MCTS in each, aggregate visit counts.
- **Guidance:** a neural net `f(observation) → (policy_logits over candidates,
  value)`. Policy is a **pointer over the enumerated candidate `Input`s** (§6.3);
  value predicts win probability from the acting seat.
- **Training:** generate self-play games where each move is chosen by IS-MCTS;
  store `(observation, MCTS visit-count policy target, game outcome)`; train the
  net to match the search policy and the outcome (AlphaZero loss = policy
  cross-entropy + value MSE). Iterate: new net → stronger self-play → better data.
- **Determinism:** every self-play game is seeded; the trainer logs seeds + input
  streams so any game replays exactly (leveraging the engine's core invariant).

### Stage 2 (stretch) — subgame solving for champion-level play
If Stage 1 plateaus below champion strength (likely, for genuinely strong human
opponents), move toward **ReBeL / Player-of-Games**: growing-tree counterfactual
regret minimization over public belief states, guided by a learned
counterfactual-value net. This is the principled fix for the residual weakness of
determinization (it can be exploited because it "averages over" hidden states
rather than reasoning about the opponent's *strategy*). Heavy; only pursue with
evidence from Stage 1.

## 6. Representation

### 6.1 Observation (network input)
A fixed-shape, perspective-relative tensor built from the redacted view (§4):
- **Global scalars:** turn number, phase/step one-hot, active-vs-me flag, my lore,
  each opponent's lore, my ready/total ink, lore-to-win, counts of each hidden
  zone (my hand size, opp hand size, deck sizes), bag size, pending-decision kind.
- **Card slots (per public/own zone):** for each card in my hand, my play, each
  opponent's play, discards — a per-card feature vector: a **learned embedding of
  `CardDefId`** (stable id→index map, with an "unknown/hidden" embedding for cards
  we can't see), current `CharacterStats` (use `current_character_stats` so
  modifiers are included), damage, conditions (ready/exerted/dry/drying), keywords
  (printed + `granted_keywords`), location/item flags, shift-stack depth.
- **Pending decision context** when mid-resolution (the `PendingDecision` and its
  `ChoiceRef` options), so the net can score `Decide` candidates.

Card identity via an **embedding table keyed by `CardDefId`** keeps the input
size bounded and generalizes across the growing card pool; only the embedding
table grows as new cards are added.

### 6.2 Card knowledge / "language" features (optional, strong upside)
Cards carry printed `text` and structured DSL (`Effect`/`Trigger`/keywords).
Feeding a (frozen) text/DSL embedding per card lets the net **generalize to cards
it has seen little of in self-play** — valuable given the huge pool. Start with the
learned `CardDefId` embedding; add text/DSL features if generalization is weak.

### 6.3 Action representation (policy output)
**Candidate scoring, not a fixed action head.** At each decision point:
1. Get `legal_actions()` (after the enumeration fixes in §4).
2. Encode each candidate `Input` into a vector (action-type one-hot + embeddings of
   the referenced `CardId`s, resolved to their slot features).
3. The policy net produces a logit per candidate (attention/pointer over the
   variable set); softmax → prior; MCTS refines into visit counts.

This is correct under a variable, card-dependent action space and needs no
re-architecting when cards are added.

### 6.4 Belief model (for determinization)
Start with **uniform** resampling of unseen cards. Upgrade to a learned belief
`P(hidden | observation)` (e.g. predict opponent hand composition from public
play + discards + draw history) to make determinizations realistic — the single
biggest lever for imperfect-info strength short of full subgame solving.

### 6.5 Action-space reduction (pruning, canonicalization & priors)

Reducing the branching factor is the single biggest search-efficiency lever and a
direct answer to "rule out inefficient choices/paths." But it must be done with a
**strict safety rule** so we don't lobotomize a champion-targeting AI:

> **Hard-prune only moves that are *provably* dominated or *provably* equivalent.
> Everything merely "usually bad" is *soft*-pruned (down-weighted in the prior),
> never removed.** Champion-level play depends on non-obvious lines (chump blocks,
> tempo sacrifices, holding ink); a hard filter that removes them caps strength and
> biases self-play training data toward the heuristic's blind spots.

**Where the branching comes from** (grounded in `candidate_moves` in
`application/api.rs`):

| Source | Growth | Reduction lever |
|--------|--------|-----------------|
| Ink any hand card | `O(hand)` | equivalence dedup (identical defs) |
| Play any hand card | `O(hand)` | affordability + soft prior |
| Quest each character | `O(own)` | strict domination + dedup |
| Boost each character | `O(own)` | strict domination |
| Move char → each location | `O(own × locations)` | dedup + soft prior |
| **Challenge each char × each foe** | **`O(own × foes)`** | domination + top-K |
| Use each activated ability | `O(own × abilities)` | "no legal/useful effect" prune |
| **Multi-pick target subsets** | **`O(2ⁿ)`** | top-K candidate generation |
| **Orderings of independent turn actions** | **permutations** | commutative-action canonical order |

The two genuine explosions are **multi-pick subsets** (exponential) and **action
orderings within a turn** (super-exponential over a turn); the challenge matrix is
quadratic. These are where analysis pays off most.

**Lossless techniques (safe to apply always, incl. during training):**

1. **Equivalence canonicalization.** Group candidate `Input`s whose resulting
   states are isomorphic and keep one representative — e.g. inking one of three
   copies of the same `CardDefId`, or questing one of two characters with identical
   def/stats/conditions/abilities. Define a deterministic *equivalence key* per
   move (def id + relevant instance features) and dedup by it. Pure win, no
   strength loss. (Map the chosen representative back to a concrete `CardId` before
   submitting.)
2. **Commutative-action ordering.** Within a turn, many actions commute (their
   end-state is order-independent). Impose a fixed canonical order on commuting
   actions so the search tree never explores `A→B` *and* `B→A`. Detect commutation
   conservatively (e.g. two inks; an ink then an unrelated quest) and only collapse
   when provably order-independent. This is the largest single reduction for a
   sequential-`Input` turn.
3. **Macro-action option (stretch).** Alternatively, treat a whole turn as one
   planned macro-action (search the intra-turn sequence with a cheaper inner
   search, expose only the resulting end-of-turn state to the top-level tree).
   Bigger change; revisit if (1)+(2) are insufficient.

**Strict-domination pruning (safe; each needs a guard against side effects):**

4. **Quest with 0 current lore** — gains nothing (`apply_quest` adds
   `current_character_stats().lore`, which can be 0) **and** only exerts the
   character — *unless* it has a "whenever this character quests" trigger
   (`Fired::Quests`), **Support**, or another quest-conditional effect. Prune only
   when no such effect exists. (The engine already enqueues these on quest, so the
   guard is a definition/keyword check.)
5. **Boost / activated ability with no possible effect** — e.g. an ability whose
   only target set is empty, or that the engine would reject anyway. The
   `is_legal`-on-a-clone filter already removes rejected inputs; this extends it to
   "legal but provably inert."
6. **Strictly dominated challenges** are *not* generally safe (a losing trade can
   be the right chump block), so challenges get **soft** treatment (below), not
   hard pruning — except the truly inert case (deal 0, take 0, change nothing).

**Soft pruning (the principled default for everything else):**

7. **Heuristic / learned prior + PUCT.** In IS-MCTS the policy prior *is* the
   pruning: low-prior moves get few or zero visits under PUCT. Before the net is
   trained, a cheap heuristic supplies the prior (favor lore-positive quests,
   value-positive challenges, on-curve plays; disfavor inert moves). After training,
   the net does it. This focuses search **without ever removing a move**, so it's
   safe for both play and training-data generation.
8. **Progressive widening** for the combinatorial decisions: generate candidates
   lazily, **top-K by the heuristic/prior**, and widen K with visit count. For
   multi-pick target subsets, generate a handful of *sensible* subsets (e.g. the
   highest-value targets, the "affect-all" set) instead of all `2ⁿ`. This bounds
   branching while leaving the full space reachable with enough search.

**Where it lives & invariants.** A `analysis` / `prune` module in `lorcana-ai`,
sitting **between** `legal_actions()` and the agent — the engine stays pure. The
module exposes both a hard-prune predicate and a prior/score per candidate, and is
**deterministic** (so seeded self-play stays replayable). Configurable per use:
**aggressive** canonicalization + domination for fast bulk self-play; **conservative**
(soft-only) for evaluation/PvAI where we must not miss a winning line.

**Safety tests (must accompany the module):** on hand-built micro-positions with a
known optimal move, assert the pruned candidate set still contains the optimum;
assert canonicalization/dedup never changes the reachable end-states; assert hard
pruning is a strict subset of `legal_actions()` and never empties a non-terminal
position.

### 6.6 `analysis` module API (concrete)

Small composable pieces (per the repo's "small composable algebras" rule): a
**soft** `Prior` (swappable heuristic → net), a **sound** `DominancePrune`
predicate, and an `Analyzer` that combines them with lossless dedup. It consumes
the engine's `legal_actions()` and is engine-pure (lives in `lorcana-ai`).

```rust
//! Action-space reduction (§6.5): turn the engine's flat `legal_actions()` into a
//! smaller, weighted candidate set for search — losslessly where possible, with
//! provably-safe hard pruning only in `Aggressive` mode.

use lorcana_engine::{CardDefId, CardId, Game, GameState, Input};
use std::collections::HashSet;

/// How aggressively to reduce the action space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruneMode {
    /// Lossless only: equivalence dedup (+ search-layer commutative ordering).
    /// Never drops a move that can reach a distinct, valuable outcome.
    /// Use for evaluation / Player-vs-AI.
    Conservative,
    /// Lossless + strict-domination hard pruning. For bulk self-play throughput.
    Aggressive,
}

/// Coarse move category — drives priors/logging; mirrors the `Input` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveClass {
    Ink, Play, Quest, Challenge, Sing, Move, Boost, Ability, EndTurn, Decide,
}

/// A legal move annotated for search.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub input: Input,
    /// Unnormalized soft weight ≥ 0 (heuristic now, policy net later). Search
    /// normalizes these into a PUCT prior. Pruning never sets this to 0 — that is
    /// the job of `DominancePrune` (a removal, not a down-weight).
    pub prior: f32,
    pub class: MoveClass,
}

/// Soft scorer: a non-negative weight per candidate. Implemented by a cheap
/// heuristic first and by the policy net later — same trait, swap freely.
pub trait Prior {
    fn weight(&self, game: &Game, input: &Input) -> f32;
}

/// Provable hard-prune predicate. MUST be *sound*: return `true` only when the
/// move is dominated/inert in **every** continuation (no value is lost), so it is
/// always safe to drop. When in doubt, return `false` (let the prior handle it).
pub trait DominancePrune {
    fn is_pruned(&self, game: &Game, input: &Input) -> bool;
}

/// A canonical, hashable signature of a move *up to isomorphism* of the resulting
/// state: two `Input`s with equal keys lead to equivalent positions, so search
/// keeps only one. Keyed on card **identity + relevant state**, never on raw
/// `CardId`, so duplicate copies collapse but a damaged copy never merges with a
/// healthy one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MoveKey {
    Ink(CardDefId),
    Play { def: CardDefId, shift_onto: Option<CardSig> },
    Quest(CardSig),
    Challenge { attacker: CardSig, defender: CardSig },
    Move { mover: CardSig, location: CardSig },
    Boost(CardSig),
    Ability { src: CardSig, index: usize },
    EndTurn,
    /// Decisions are already canonical enough; key on the decision payload.
    Decide(String),
}

/// All state-relevant attributes that make two in-play cards interchangeable.
/// Derives `Hash`/`Eq` so equal signatures dedup. (Current stats include
/// modifiers via `state.current_character_stats`.)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardSig {
    def: CardDefId,
    strength: u32,
    willpower: u32,
    lore: u32,
    damage: u32,
    ready: bool,
    exerted: bool,
    drying: bool,
    granted: Vec<String>, // sorted granted keyword/restriction tags
    stack_depth: u8,      // Shift stack height
}

fn card_sig(state: &GameState, card: CardId) -> CardSig { /* read instance + state */ }
fn move_key(state: &GameState, input: &Input) -> MoveKey { /* match on Input */ }

/// Combines the pieces. `Prior`/`DominancePrune` are injected so the same
/// `Analyzer` works with a heuristic now and the net later.
pub struct Analyzer<P, D> {
    pub mode: PruneMode,
    pub prior: P,
    pub prune: D,
    /// Optional cap (top-K by prior) for progressive widening; `None` = keep all.
    pub top_k: Option<usize>,
}

impl<P: Prior, D: DominancePrune> Analyzer<P, D> {
    /// The reduced, weighted candidate set for the current decision point.
    /// Deterministic: iterates `legal_actions()` in its (deterministic) order and
    /// keeps the first representative of each equivalence class.
    pub fn candidates(&self, game: &Game) -> Vec<Candidate> {
        let state = game.state();
        let mut seen = HashSet::new();
        let mut out: Vec<Candidate> = game
            .legal_actions()
            .into_iter()
            // (1) sound hard prune — Aggressive only.
            .filter(|i| self.mode == PruneMode::Conservative || !self.prune.is_pruned(game, i))
            // (2) lossless equivalence dedup.
            .filter(|i| seen.insert(move_key(state, i)))
            // (3) soft score.
            .map(|input| Candidate {
                prior: self.prior.weight(game, &input),
                class: classify(&input),
                input,
            })
            .collect();
        // (4) optional top-K widening (stable: ties keep enumeration order).
        if let Some(k) = self.top_k {
            out.sort_by(|a, b| b.prior.total_cmp(&a.prior));
            out.truncate(k);
        }
        out
    }
}
```

Notes:
- **Single-move fast path:** when `candidates()` yields one entry, the search layer
  takes it without expanding a node (very common in Lorcana — forced draws, lone
  decisions). Big real-world saver, lives in the agent, not here.
- **Commutative ordering** (§6.5 #2) needs the *turn-so-far* history, which a
  single-state analyzer lacks, so it's a thin filter applied by the search/env
  layer (given the actions already taken this turn), not part of `Analyzer`.
- The heuristic `Prior` and `DominancePrune` impls are small, well-tested functions
  (e.g. `is_pruned` = inert 0-lore quest with no quest/Support trigger; the net
  `Prior` later returns policy logits for the same `Input`s).

## 7. Performance & the stack decision

**Constraints from you:** *training + play speed matter most*; hardware is an
**M4 MacBook** (Apple Silicon, Metal) with an **optional Radeon 6800XT** (AMD
RDNA2 — **no CUDA**, ROCm support for consumer cards is unreliable).

**Why inference must be in-process (Rust).** IS-MCTS evaluates many leaves per
move; a Python round-trip per node destroys throughput. The simulator is already
fast Rust. So **leaf evaluation should run in the same process as the engine**,
batched.

**Recommendation: an all-Rust stack on `burn` with the `wgpu` backend.**
- `wgpu` runs on **Metal (M4)** *and* **Vulkan (6800XT)** — the only sane way to
  use the AMD card without CUDA/ROCm, and it gives Apple-Silicon GPU acceleration
  on the primary machine.
- Training **and** inference live in one Rust codebase alongside the engine → no
  FFI in the hot self-play loop, maximal throughput, single language/toolchain,
  fits the existing Cargo workspace and `burn`/wgpu portability.
- CPU fallback (and `burn`'s ndarray/candle backends) keeps small-scale runs and CI
  viable.
- **Risk:** `burn` is younger than PyTorch; fewer turnkey layers/recipes. Mitigate
  by keeping the net architecture simple (embeddings + MLP/ResNet + attention
  pooling over candidates) — well within `burn`'s capabilities.

**Documented alternative (hybrid):** train in **PyTorch** (MPS on the Mac; the
6800XT is largely unusable here) and run **Rust inference via ONNX Runtime**
(CoreML execution provider → GPU/ANE on Apple Silicon). Best-in-class training
kernels and ecosystem, but: two languages, an export/serialization boundary, and
the AMD card is effectively shut out of training. Choose this only if `burn`'s
training ergonomics become a bottleneck.

**Throughput plan:** classic **actor–learner** architecture — a pool of parallel
self-play actors (each an engine instance) batches leaf evaluations to a shared
GPU inference context; a learner consumes the replay buffer and periodically
publishes a new net; an arena evaluator gates promotion. Self-play sim is
CPU-parallel (Rust threads/`rayon`); GPU does batched NN eval and training.

**First numbers to gather (before building search):** `GameState::clone()` cost,
random-playout games/sec single-threaded and across cores, and observation-encode
cost. These size the whole training loop and tell us whether clone needs
optimizing (§4).

### 7.1 Further throughput optimizations (training is headless)

Self-play throughput (games × moves × leaf-evals per second) is the bottleneck of
the whole project — every doubling roughly halves wall-clock to a given strength.
Ordered by expected payoff, grounded in this codebase:

**Headless / no rendering (your point — and a bit more).** Training runs entirely
in `lorcana-ai` against the core engine; the web/WASM UI (`crates/lorcana-wasm`,
`web/`) and any board rendering are **never** in the loop. Beyond "don't draw":
- The `audit-log` feature is **off** by default — keep it off for training builds.
- **`apply` always allocates a `Vec<GameEvent>`** (`reducer.rs`); training doesn't
  consume events. Add a **silent path** (e.g. an `EventSink` the reducer writes to,
  with a no-op sink for self-play, or a `submit_silent` that skips event
  construction). Removes per-step allocation on the hottest path.
- Skip serialization except at checkpoints; never `serde` a state mid-rollout.

**Cheap legal-move generation (likely the single biggest engine win).**
`Game::legal_actions()` validates **by cloning the whole `GameState` and running
`apply` once per candidate** (`is_legal` in `application/api.rs`). That is
`O(branching)` full clones *just to enumerate one node* — multiplied by every MCTS
node, it dominates. Add a **native generator** that produces legal moves by
direct precondition checks (reusing the reducer's validation logic) without the
trial-apply, and have the AI use that. Keep the clone-based `legal_actions()` as
the reference oracle the generator is property-tested against.

**Make–unmake instead of clone-per-node.** MCTS clones state to descend. Cheaper
options, in increasing effort: (a) **copy-on-write zones** (`Arc<[CardInstance]>`)
so untouched zones share storage; (b) an **undo log** (apply records a reversible
delta; unwind on backtrack) — the classic chess-engine trick, often the biggest
win; (c) a slimmer **search-state projection** that drops fields search never reads.
Pick after the §7 clone benchmark says how hot it is.

**Don't re-encode observations from scratch.** Maintain the network input tensor
**incrementally** as moves are applied (delta updates), or only encode at MCTS
leaves. Also cache `current_character_stats` / modifier evaluation per state — it
re-scans all modifiers on every call (`state.rs`), which is hot under heavy lookup.

**Search-level savers (no engine change):**
- **Single legal move ⇒ no search** (forced draws, lone decisions — common here).
- **Immediate-win / forced-loss shortcuts** before expanding.
- **Subtree reuse:** keep the chosen child as the new root between moves.
- **Transposition table** keyed by a Zobrist/`MoveKey`-style state hash (within a
  determinization) to share equal nodes.
- **Resignation + game-length cap** in self-play (AlphaZero-style): abandon
  hopeless games early to save compute and cut value-target variance.
- **Leaf-parallel MCTS with virtual loss** to fill GPU inference batches.

**NN inference:**
- **Batch leaf evaluations** across parallel actors/trees into one GPU call (the
  actor–learner design above); inference latency, not FLOPs, is the constraint.
- **`fp16`/`bf16`** on Metal; consider **int8 quantization** and **distilling** the
  training net into a smaller, faster *play* net.
- Keep the net small (embeddings + modest trunk + attention pool over candidates);
  for IS-MCTS, *more determinizations × a small net* often beats *one big net*.

**Parallelism:** self-play sim is embarrassingly parallel across games (`rayon`,
independent seeds → still per-game reproducible). Use the M4 efficiency+performance
cores for actors and the GPU for batched eval/training. Avoid global state; the
engine's RNG already lives in `GameState`, so no shared-RNG contention.

**Build / runtime (easy, immediate wins — none configured today; no `[profile]`
in any `Cargo.toml`):** for training/self-play binaries set
`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `opt-level = 3`, and build
with `-C target-cpu=native` (Apple Silicon). Consider **PGO** for the engine hot
path once stable. These commonly buy 10–30% for free.

**Correctness guard:** every optimization that changes representation (silent
events, make–unmake, COW, native generator) must preserve the engine's core
invariant — *same seed + inputs ⇒ identical state + event log* — verified against
the existing determinism/`self_play` tests (the native generator property-tested to
return exactly `legal_actions()`'s set).

## 8. Proposed crate & module layout

A new workspace member `crates/lorcana-ai` (already anticipated in `Cargo.toml`),
depending only on `lorcana-engine`:

```
crates/lorcana-ai/
├── env/         # Game wrapper: drives both seats + bag/decisions, terminal reward,
│                #   seeded episodes, redacted observation per seat
├── encode/      # observation tensor + candidate-action encoding (CardDefId embeddings)
├── analysis/    # action-space reduction (§6.5): equivalence dedup, commutative
│                #   ordering, strict-domination prune, heuristic prior / top-K
├── belief/      # determinization: resample hidden zones from an observation
├── agent/       # trait Agent { fn act(&mut self, obs) -> Input }
│   ├── random   #   uniform over legal_actions (baseline / smoke tests)
│   ├── heuristic#   Stage 0 eval + shallow search
│   └── ismcts   #   Stage 1 IS-MCTS (+ net guidance)
├── net/         # burn model: embeddings + trunk + (policy-over-candidates, value)
├── selfplay/    # actor pool, replay buffer, episode logging (seed + input stream)
├── train/       # learner loop (AlphaZero loss), checkpointing
├── eval/        # arena: head-to-head win-rate vs baseline / previous best (gate)
└── bin/         # `train`, `selfplay`, `arena`, `play-vs-ai` CLIs
```

The engine stays untouched except for the §4 facade/observation additions in
`application` / `domain/game`.

## 9. Delivery slices (each independently shippable + tested)

Following the repo's slice workflow (a slice isn't done until its acceptance tests
pass; clear deferred TODOs before the next slice).

- **AI-0 — Engine prerequisites.** Complete `legal_actions()` enumeration;
  perspective-redacted observation; determinization/resample hook; clone/throughput
  benchmarks; **the two hot-path wins from §7.1** — a clone-free native legal-move
  generator (property-tested against `legal_actions()`) and a silent/no-event
  `apply` path for self-play. *Tests:* enumeration completeness on real decks;
  redaction never leaks hidden zones; resampled states are always legal and
  observation-consistent; native generator returns exactly `legal_actions()`'s set;
  silent path preserves the determinism invariant.
- **AI-1 — `lorcana-ai` scaffold + env + random/heuristic agents.** Env wrapper
  drives full games for both seats (incl. the bag and all decisions); `Agent`
  trait; random + heuristic agents; AI-vs-AI runner. *Tests:* full games to
  completion, no panics, invariants hold (mirror `tests/self_play.rs`); heuristic
  beats random ≥ ~80%.
- **AI-1.5 — Action-space reduction (§6.5).** `analysis` module: lossless
  equivalence dedup + commutative-action canonical ordering, strict-domination
  pruning (0-lore inert quest, inert ability), and a heuristic prior / top-K +
  progressive widening for the combinatorial decisions. Aggressive vs conservative
  config. Lands here so AI-3's search starts from a smaller tree. *Tests:* pruned
  set always contains the known-optimal move on micro-positions; dedup preserves
  reachable end-states; hard prune ⊆ `legal_actions()` and never empties a
  non-terminal position; pruning is deterministic.
- **AI-2 — Observation/action encoding + `burn` net (no search yet).** Encoders +
  net; train a policy/value net by **behaviour cloning the heuristic** (cheap data,
  validates the whole pipeline end-to-end). *Tests:* encode determinism; net beats
  random; shapes/round-trips.
- **AI-3 — IS-MCTS with net guidance + self-play loop.** Determinized IS-MCTS;
  actor–learner self-play; AlphaZero training; arena gate. *Tests:* MCTS+net beats
  heuristic; training reduces loss; promotion only on arena win-rate threshold.
- **AI-4 — Belief model + scaling + PvAI integration.** Learned belief for
  determinization; multi-core/GPU scaling; expose a "play vs AI" path for the
  CLI/web host. *Tests:* belief improves strength vs uniform; reproducible seeded
  matches; latency budget for interactive PvAI.
- **AI-5 (stretch) — subgame solving (ReBeL/PoG).** Only if AI-4 plateaus below
  target strength against strong human/expert benchmarks.

## 10. Evaluation (how we know it's "smart")

- **Arena win-rate** vs (a) random, (b) heuristic baseline, (c) previous best net —
  the promotion gate, over many seeds and **both** seat orders and multiple
  matchups (reuse the official decklists in `decks/`).
- **Exploitability sanity checks:** does the agent get punished by an opponent that
  exploits determinization (e.g. bluff-like lines)? Tracks Stage-1→Stage-2 need.
- **Human ladder (later):** games vs strong human players / known-strong heuristic
  bots once available.
- **Determinism/replay tests:** any logged self-play game replays to an identical
  state + event log (the engine's core invariant, extended to the AI loop).

## 11. Key risks & mitigations

- **Imperfect-info ceiling.** Determinization (Stage 1) is exploitable in theory;
  may not reach champion level. *Mitigation:* learned belief (AI-4), then Stage-2
  subgame solving with evidence.
- **Action-enumeration completeness.** Missing legal lines (multi-pick, names) cap
  strength and bias training. *Mitigation:* AI-0 closes the gaps first; assert
  enumeration coverage on real decks.
- **Clone/throughput cost** for MCTS. *Mitigation:* measure first (AI-0); optimize
  zones/rollout only if hot.
- **`burn` maturity.** *Mitigation:* keep the architecture simple; the ONNX/PyTorch
  hybrid (§7) is a documented fallback.
- **Card-pool generalization.** Huge pool, sparse per-card self-play coverage.
  *Mitigation:* `CardDefId` embeddings + optional text/DSL features (§6.2).
- **Over-aggressive pruning (§6.5).** A hard filter that removes a sometimes-correct
  line (chump block, tempo sacrifice) caps strength *and* biases training data.
  *Mitigation:* hard-prune only provably dominated/equivalent moves; everything else
  is soft (prior); micro-position tests assert the optimum is never pruned;
  conservative (soft-only) mode for evaluation/PvAI.
- **Reward sparsity / long games.** *Mitigation:* MCTS value target + optional
  lore-based shaping; cap game length in training.

## 12. Open questions (to revisit as data arrives)
- Single shared net for both seats vs separate? (Start shared, seat-relative obs.)
- Determinizations per move (N) and MCTS sims per determinization — tune against the
  measured throughput budget.
- How much does a learned belief help vs uniform, empirically?
- Is text/DSL conditioning worth the complexity for pool generalization?
- At what measured strength do we commit to Stage-2 subgame solving?
- Mixture weights for the deck curriculum (§13), and how fast to anneal toward
  diversity.
- Do we condition the net on the agent's own (fully known) decklist, and how?

## 13. Training deck distribution (what decks to play)

**The deck distribution *is* the training distribution.** Whatever decks self-play
sees is what the agent generalizes to; everything else is out-of-distribution and
played badly. So this is a first-class design decision, not a detail.

**Recommendation: a staged *curriculum mixture* — not pure-meta, not pure-random.**

- **Pure proven/meta decks only** → realistic lines fast, *but* overfits to a narrow
  slice, is brittle against novel cards/decks, leaves most of the card pool
  untrained, and — critically — won't cope with the **deck-builder AI's** output
  (which by design explores outside the human meta).
- **Pure random legal decks** → maximal coverage, *but* most random legal 60-card
  2-ink piles are incoherent (no curve, no synergy). The agent burns compute
  learning to play garbage, the self-play signal is weak, and it can settle into
  degenerate "both decks are bad" equilibria — underrepresenting exactly the
  synergy/combo depth champions exploit.

**The curriculum (anneal the mixture over training):**

1. **Bootstrap — coherent fundamentals.** Start on the **21 official starter decks**
   already in `decks/` (validated, §2.1.1, coherent curves). The net learns the
   core skills — ink management, curve, quest-vs-challenge, the lore race, mulligan
   decisions — on *sane* boards quickly.
2. **Broaden — competitive + structured-random.** Mix in (a) known strong/meta
   lists as they're added, and (b) **structured-random decks** that are legal *and
   playable*: archetype templates (aggro / midrange / control), an enforced ink
   curve, coherent 1–2 ink colours, real win conditions. Cheapest effective source:
   **perturb official decks** (swap a handful of cards) and **archetype-templated
   sampling** from cards that co-occur in real lists.
3. **Tail robustness — fully random legal decks** at a small, growing weight, so the
   agent is never helpless against the unexpected (and so the deck-builder AI's
   off-meta output is in-distribution).
4. **Co-evolution (when the deck-builder AI exists, out of scope here).** The
   builder proposes decks, the play-AI trains against them — a natural curriculum
   *and* the eventual product. Structured-random in (2)/(3) is the stand-in until
   then.

**Sampling rules that matter as much as the deck set:**
- **Sample *both* decks per game** from the current mixture, and **play both seat
  orders** — first-player advantage is real in Lorcana (the starter skips their
  first draw), so the matchup matrix and mirror matches must both be covered, not
  just marginal deck quality.
- **Condition on your own deck.** You legitimately *know your own full decklist*, so
  feed it into the observation (known cards), letting the policy specialize per
  deck; the *opponent's* list stays hidden and is handled by the belief model
  (§6.4). Training across many decks is what teaches this conditioning.
- **Held-out evaluation decks.** Measure generalization on decks/matchups **not**
  used in training (plus the canonical official decks, and human-meta lists later).
  Never leak the eval set into training.
- **Instrument card coverage.** Track per-card play frequency in self-play;
  under-covered cards flag where to add decks — directly attacks the card-pool
  generalization risk (§11).

A small `decks` sampler in `lorcana-ai` (curriculum schedule + structured-random
generator + held-out split) owns all of this; it's deterministic per seed so deck
pairs are part of the reproducible training manifest.

## 14. Additional high-leverage improvements

Beyond the algorithm/encoding/pruning/perf already covered, the items most worth
their complexity for a *champion-targeting, compute-limited* build:

- **Sample-efficient search — Gumbel AlphaZero/MuZero.** Gets strong policy
  improvement from **very few simulations per move**, which is exactly the
  constraint on an M4. High priority: it can be the difference between feasible and
  infeasible self-play throughput.
- **League / opponent pool (AlphaStar-style), not just "vs latest net."** Train
  against a population of past checkpoints (+ a few **exploiters**) to avoid
  strategic cycles and catastrophic forgetting, and to be **robust rather than
  merely good on average** — essential against adaptive human champions.
- **Exploitability as a first-class metric.** Periodically train a dedicated
  **best-response/exploiter** agent against the current net; how badly it wins is
  the real "is this exploitable?" signal for an imperfect-info agent — more honest
  than self-play win-rate. Feeds the Stage-1→Stage-2 decision.
- **Order-invariant set encoder.** Hands and play areas are *sets*; encode them with
  attention/deep-sets pooling so zone order is irrelevant by construction (no need
  for order augmentation, and no spurious order-dependence to learn).
- **Auxiliary prediction heads.** Predict opponent hand composition, final
  lore-margin, and game length as side losses. Cheap, and they sharpen the
  representation *and* bootstrap the belief model (§6.4); a KataGo-style lore-margin
  target also gives denser signal than win/loss alone.
- **Auto-resolve trivial decisions in the env.** When a decision (incl. bag
  ordering) has exactly one sensible/legal option, resolve it automatically so
  neither search nor the policy spends capacity on non-choices — both a speed and a
  signal-quality win (complements §6.5's single-move fast path).
- **Reproducible training manifests.** Log net version + seed + deck pair + input
  stream per game; the engine's determinism then replays any training game exactly
  for debugging/curation — a debugging superpower most RL stacks lack.
- **Distill a fast *play* net** from the larger *training* net for low-latency PvAI
  (already noted in §7.1; restated as it's key to "fast" at inference).

## 15. Deck-builder co-training — should we start it now?

**Short answer: build the *seam* now, but do not co-train a *learned* deck-builder
net from scratch alongside the play model. Introduce deck optimisation as an outer
loop only once the play AI is a trustworthy evaluator.**

**Why not full co-training from day one.** A deck-builder's fitness signal *is* the
play AI: "this deck is good" means "the play AI wins with it." That creates a hard
dependency and two failure modes if both learn from scratch simultaneously:
- **Bootstrapping.** Early on the play AI is weak/noisy, so deck win-rates are mostly
  noise. A learned builder optimising against a bad evaluator learns nonsense —
  often it learns to exploit the *play AI's current weaknesses* rather than real deck
  quality, and that signal evaporates as the play AI improves.
- **Instability (co-evolution / Red Queen).** Two learning systems chasing each
  other is a moving target on both axes — prone to cycling and catastrophic
  forgetting. Hard enough to stabilise *one* self-play loop; two at once multiplies
  the risk and the compute.

**What to do instead (cheap now, no instability):**
- **Put in the abstraction seam today.** The training loop only ever talks to a
  `DeckSource` trait (Appendix B). A random/structured generator now and a future
  learned/evolved builder are both just `DeckSource` impls — so the deck-builder
  slots in later with **zero changes** to the self-play loop.
- **Phase the real thing.** (1) Play AI reaches stable strength on the §13
  curriculum. (2) Then run deck optimisation as an **outer loop** against a
  **frozen / periodically-refreshed strong play-net** as a *stable* evaluator
  (refreshing avoids the moving-target problem). The full learned deck-builder
  remains its own project, but lands on this seam.
- **A safe early on-ramp that actually *helps* the play AI:** a **non-learned
  evolutionary/bandit deck optimiser** as a `DeckSource` — mutate/crossover decks,
  score by play-AI win-rate, keep the winners. It's "co-evolution" but with one side
  being *simple search*, not a second learning net, so it's stable. It yields an
  **adversarial auto-curriculum** ("decks that beat the current AI"), which is
  exactly the kind of pressure that makes the play AI generalise. Gate it on the
  play AI being decent (else it optimises against noise), and feed a frozen snapshot
  as its evaluator.

So: **infrastructure now, learned builder later.** The evolutionary `DeckSource` is
the bridge — worth adding earlier than the full builder because it doubles as
training pressure for the play model.

---

## Appendix A — Engine hot-path change specs (slice AI-0)

The two §7.1 engine-side wins, specified concretely. Both are **engine** changes
(not in `lorcana-ai`) because they touch the reducer's internals. **Sequencing:**
do **A.2 first** (bigger throughput win, less invasive), then **A.1** (more
mechanical churn, smaller win — measure before committing).

### A.1 Silent / no-event apply (`EventSink`)

**Problem.** `apply` always allocates and fills a `Vec<GameEvent>`
(`reducer.rs`); helpers (`game_state_check`, `resolve_bag`, every `apply_*`) build
and `extend` more `Vec`s. Self-play never reads events, so this is pure waste on the
hottest path.

**Why it's provably safe.** Per the architecture, **events are *outputs*** — they
never feed back into `GameState`. So discarding them cannot change game behaviour:
a null sink is behaviour-identical to collecting, by construction. This is the
correctness argument the tests below pin down.

**Design — thread a sink instead of returning `Vec`:**

```rust
// domain/engine/event_sink.rs (new)
/// A consumer of emitted events. Implementors decide whether to keep them.
pub trait EventSink {
    fn emit(&mut self, event: GameEvent);
    fn emit_all(&mut self, events: impl IntoIterator<Item = GameEvent>) {
        for e in events { self.emit(e); }
    }
}

/// Collects events (preserves today's behaviour).
#[derive(Debug, Default)]
pub struct VecSink(pub Vec<GameEvent>);
impl EventSink for VecSink {
    fn emit(&mut self, e: GameEvent) { self.0.push(e); }
}

/// Discards events with zero allocation (self-play / search).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;
impl EventSink for NullSink {
    fn emit(&mut self, _e: GameEvent) {}
}
```

Core reducer becomes generic over the sink; the public `apply` keeps its signature
as a thin wrapper for full back-compat:

```rust
/// New core: emits into `sink`, returns only success/rejection.
pub fn apply_into<S: EventSink>(
    state: &mut GameState,
    registry: &CardRegistry,
    input: Input,
    sink: &mut S,
) -> Result<(), Rejected> { /* refactored body */ }

/// Unchanged public API (callers that want events): collect into a VecSink.
pub fn apply(
    state: &mut GameState,
    registry: &CardRegistry,
    input: Input,
) -> Result<Vec<GameEvent>, Rejected> {
    let mut sink = VecSink::default();
    apply_into(state, registry, input, &mut sink)?;
    Ok(sink.0)
}
```

The body refactor is mechanical: `let mut events = vec![e]; … events.extend(more); Ok(events)`
becomes `sink.emit(e); … sink.emit_all(more); Ok(())`. **Scope/churn:** the dominant
allocations disappear once `apply_*` emit into the sink; the *full* win also requires
threading the sink into `game_state_check` / `resolve_bag` (today they return `Vec`).
Do those two as well; they're the same mechanical change. The AI calls
`apply_into(&mut state, registry, input, &mut NullSink)`.

**Tests:**
- **Equivalence:** for every input in the existing scenario/`self_play` suites,
  `apply_into(.., &mut VecSink)` produces the *same* events and *same* final state
  as today's `apply` (golden-diff).
- **Behaviour-identity of `NullSink`:** running a full game with `NullSink` yields a
  byte-identical final `GameState` to the same game run with `VecSink` (same seed +
  inputs) — i.e. the determinism invariant holds regardless of sink.

### A.2 Native, clone-free legal-move generator

**Problem.** `Game::legal_actions()` validates each candidate by **cloning the
whole `GameState` and running `apply` on the clone** (`is_legal` in
`application/api.rs`): `O(branching)` full clones + applies *per node*. Under MCTS
this dominates enumeration cost.

**Key enabler — the reducer already separates validation from mutation.** Every
`apply_*` is written as a `// --- validate (no mutation yet) ---` block followed by
`// --- mutate ---` (see `apply_quest`, `apply_put_in_inkwell`, …). Extract each
validate block into a pure `check_*` that the generator can call directly with no
clone and no mutation.

**Step 1 — extract checks.** For each action, a pure predicate:

```rust
// domain/engine/legality.rs (new) — pure, no mutation, crate-visible.
pub(crate) fn check_quest(
    state: &GameState, registry: &CardRegistry, character: CardId,
) -> Result<(), Rejected> { /* exactly today's apply_quest validate block */ }

pub(crate) fn check_put_in_inkwell(
    state: &GameState, registry: &CardRegistry, card: CardId,
) -> Result<(), Rejected> { /* … */ }
// check_play_card, check_challenge, check_sing, check_move, check_boost,
// check_use_ability, check_end_turn …
```

Each `apply_*` then starts with `check_*(state, registry, ..)?;` before its mutate
block — so the validation logic has **one** definition shared by `apply` and the
generator (it can never drift, the property the clone-based version gave us for
free).

**Step 2 — the generator** (engine-side, because `check_*` are crate-visible):

```rust
/// Enumerate legal inputs by direct precondition checks — no clone, no mutation.
pub fn legal_moves(state: &GameState, registry: &CardRegistry) -> Vec<Input> {
    // Pending decision / mulligan / non-Playing handled exactly as api.rs does
    // today (read straight from the PendingDecision / status).
    // Otherwise: build the candidate set (same shape as `candidate_moves`,
    // extended per §4 for multi-pick & Shift/Sing) and keep those whose
    // matching `check_*` returns Ok — no `probe = state.clone()`.
}
```

`Game::legal_actions()` is **reimplemented on top of `legal_moves`**, so all
existing callers (CLI, web, tests) get the speedup transparently.

**Correctness — keep the old method as the oracle.** Retain the clone-based
implementation under a debug/test-only name (`legal_actions_via_clone`) and add a
property test asserting, **as sets**, `legal_moves(state) == legal_actions_via_clone(state)`
across the `self_play` / `official` decks and many seeds. This guarantees the fast
generator never drifts from "what `apply` would actually accept."
- *Caveat to test for:* a candidate that passes top-level `check_*` but would still
  be rejected deep in resolution would diverge. For the current action set the
  `apply_*` validate blocks are complete (challenge checks Evasive/Bodyguard, sing
  checks payment, etc.), so the sets should match exactly; the oracle test makes any
  future gap a hard failure rather than a silent bug.

**Combinatorial cases** (multi-pick targets, Shift/Sing singer subsets) reuse the
§4 enumeration work: the generator produces the candidate subsets and validates each
cheaply with `check_*` — and §6.5's canonicalization/top-K then trims the set the AI
actually searches.

**Expected payoff.** Removes one full `GameState` clone + `apply` per candidate per
node; combined with make–unmake descent (§7.1) it targets the two largest per-node
costs in MCTS. Confirm with the §7 benchmark before/after.

---

## Appendix B — `decks` sampler spec (§13)

Lives in `crates/lorcana-ai/src/decks/`. Built on the engine's existing `Deck`
(`{ name, cards: Vec<DeckCard{ card: CardDefId, count }> }`), `Deck::validate`
(§2.1.1) / `Deck::expand`, and `CardRegistry` (`iter()`, `find_by_name()`), with
card facts from `CardDefinition` (`cost`, `card_type`, `ink_types`,
`max_deck_copies`, `has_inkwell_symbol`, `is_song`, `classifications`).

### B.1 The seam — one trait the training loop depends on

```rust
use lorcana_engine::{CardRegistry, Deck};

/// Yields one matchup (deck pair) for a self-play game. The self-play loop knows
/// *only* this trait — fixed pools, structured-random, perturbation, the
/// curriculum mixture, and a future evolved/learned deck-builder (§15) are all
/// just impls, so the loop never changes when the builder arrives.
pub trait DeckSource {
    /// Draw one matchup. `seed` makes the draw deterministic so deck pairs are part
    /// of the reproducible training manifest (§14).
    fn sample_pair(&mut self, registry: &CardRegistry, seed: u64) -> (Deck, Deck);
}
```

### B.2 Concrete sources

```rust
/// Two decks drawn (with replacement, allowing mirrors) from a fixed list — e.g.
/// the 21 official starters in `decks/`. Bootstrap stage of the curriculum.
pub struct FixedPool { pub decks: Vec<Deck> }

/// Take a base deck and swap `k` cards for legal alternatives (ink-compatible,
/// copy-limit-respecting). Cheap "near-meta" diversity (§13 step 2).
pub struct Perturb { pub base: Vec<Deck>, pub swaps: u32 }

/// Archetype-templated coherent generator (B.3). The tail-diversity workhorse.
pub struct StructuredRandom { pub archetypes: Vec<Archetype> }

/// Weighted mixture of sub-sources whose weights move with the global training
/// step (anneal bootstrap → diversity). This is the top-level §13 curriculum.
pub struct Curriculum {
    pub stages: Vec<(Box<dyn DeckSource>, WeightSchedule)>,
    pub step: u64, // current global training step, advanced by the loop
}
```

`Curriculum::sample_pair` evaluates each `WeightSchedule` at `step`, picks a
sub-source proportionally (deterministically from `seed`), and delegates — deriving
a child seed so the choice and the draw are both reproducible.

### B.3 Structured-random generator (the coherence-critical part)

A purely random legal deck is usually unplayable; constrain generation so it is
*legal **and** coherent*:

```rust
/// A coarse deck shape — controls the target ink curve and card-type mix.
pub struct Archetype {
    pub name: String,
    /// Target fraction of cards per cost bucket 0..=7+ (aggro = low, control = high).
    pub curve: [f32; 8],
    pub min_inkable: u32,     // playability: enough cards with the inkwell symbol
    pub min_characters: u32,  // need win conditions / board presence
    pub song_bias: f32,       // how much to favour Songs / singers
}
```

Generation procedure (rejection-sample-then-repair, all deterministic from `seed`):
1. **Pick ink colours.** 1 or 2 `InkType`s (weighted toward 2). Candidate pool =
   cards whose `ink_types()` are a subset of the chosen colours (dual-ink commits
   both — already how `Deck::validate` reads §2.1.1.2).
2. **Fill by curve.** For each cost bucket, draw cards (weighted by the archetype
   `curve`) up to `max_deck_copies` (default 4) and ≤4 per name, until ≥60 total.
3. **Enforce playability constraints:** at least `min_inkable` cards with
   `has_inkwell_symbol()` (so the deck can actually build ink), at least
   `min_characters` characters (`card_type()`), and not an all-Action pile.
4. **Validate & repair.** Run `deck.validate(registry)`; on any `DeckError`
   (ink/copies/count), swap or trim the offending cards and retry. Bounded retries,
   then fall back to a known-good template so generation always terminates.

The same constraint engine powers `Perturb` (swap within ink/copy rules) and the
future evolutionary builder's mutation operator (§15) — one coherence module, reused.

### B.4 Held-out split & coverage

```rust
/// Deterministic partition of a named/official deck pool into disjoint train/eval
/// sets. The eval set feeds the arena (§10) and is NEVER given to a training
/// `DeckSource` — guards against measuring on what we trained on.
pub struct DeckSplit { pub train: Vec<Deck>, pub eval: Vec<Deck> }

/// Counts how often each CardDefId is actually *played* across self-play games.
/// Under-covered cards flag where the curriculum needs more decks (attacks the
/// card-pool generalization risk, §11). Consumes self-play stats; not a DeckSource.
pub struct CoverageTracker { /* CardDefId -> play count */ }
```

### B.5 Invariants & tests
- **Always legal:** every `Deck` a source emits passes `Deck::validate` — property
  test across many seeds and all archetypes.
- **Always coherent:** generated decks meet their archetype's `min_inkable` /
  `min_characters` and curve-shape tolerances.
- **Deterministic:** same `(source, step, seed)` ⇒ identical deck pair (so manifests
  replay exactly).
- **No leakage:** `DeckSplit.train` ∩ `DeckSplit.eval` = ∅; the eval source is never
  wired into a training `DeckSource`.
- **Termination:** structured-random always returns within bounded retries (falls
  back to a template), never loops.
