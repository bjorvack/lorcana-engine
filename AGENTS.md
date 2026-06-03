# AGENTS.md — working agreement for this repo

Guidance for AI agents (and humans) working on `lorcana-engine`. The full process
lives in [`docs/development/CONTRIBUTING.md`](docs/development/CONTRIBUTING.md);
this file highlights the non-negotiable rules.

## Test-driven development (required)

Engine behaviour is developed **test-first**. For every behaviour change or bug fix:

1. **Understand the real behaviour first** — the comprehensive rules in
   `docs/rules/` and, for card mechanics, the actual card text (use the
   `lorcast-cards` skill). The rules docs win over card text on rules points.
2. **Think up concrete board states** that exercise the behaviour and its edge
   cases / interactions (self vs opponent, "choose" vs "affect all", §1.2.3 "do as
   much as you can", etc.).
3. **Write the tests first and run them** — confirm they fail (or pass) for the
   *right reason* before touching the implementation.
4. **Implement the minimal change** to go red → green.
5. **Refactor** with the suite green; prefer small composable primitives over
   special-cased flags (e.g. `Effect::May(..)` wrapping any effect instead of a
   per-effect `optional` bool; keywords map to restrictions/permissions rather than
   being read as game logic).
6. **Encode worked rules examples** (the §-numbered examples in `docs/rules/`) as
   conformance tests where practical — they are the highest-value tests.

See `docs/development/CONTRIBUTING.md` → "Test-driven development" for detail.

## Before every commit

All three must pass (CI enforces them — `.github/workflows/ci.yml`):

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## Composable algebras (required)

Prefer **small composable algebras + a general continuation** over bespoke,
per-case enums/handlers. When you reach for a new one-off type or flag, first
check whether an existing algebra already expresses it (or can be extended):

- **Candidate selection** uses the `CharacterFilter` boolean algebra
  (`And` / `Or` / `Not` over leaf predicates incl. `IsSource` / `IsCard`).
  Express *exclusions* with the algebra — "another" is
  `And([.., Not(IsSource)])`, "not the already-chosen card" is `Not(IsCard(..))` —
  **never** a bespoke `another`/`exclude_*` flag, helper, or an ad-hoc
  `options.retain`.
- **Amounts** (`Amount`: `Fixed` / `PerMatchingCharacter(filter)` / `StatOf`),
  **player scope** (`PlayerScope`), and **zone moves** (`Effect::Move` with
  `MoveSource` / `Destination`) are the other canonical algebras — reuse them.
- A new "choose / target / filter / amount / move" need should **extend the
  existing algebra**, not add a parallel specific type. Interactive choices go
  through the general choose primitive (resolve options + a continuation that can
  thread earlier results into later filters), not a new `Choice`/`PendingDecision`
  triple per case.

If a card genuinely can't be expressed, that's a signal to grow an algebra (add a
predicate/variant), not to special-case it.

## Other standing rules

- **Atomic, Conventional Commits** (see CONTRIBUTING) — each commit builds and
  passes all checks on its own.
- **Slice workflow:** before moving to the next slice, clear the doable deferred
  items / back-linked `TODO`s it left behind (CONTRIBUTING → "Slice workflow").
- **Keep the plan honest:** update `docs/planning/IMPLEMENTATION_PLAN.md` as work
  lands; keep back-link `TODO`s accurate.
- **New configuration** (skills, rules, MCP, project settings) goes under `.devin/`.
