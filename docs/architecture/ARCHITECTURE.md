# Lorcana Engine Architecture

> **Note**: This is a living document that describes the intended architecture of the lorcana-engine. As implementation progresses, this document will be updated to reflect actual implementation decisions and discovered patterns. Specific struct definitions and implementation details will be added as components are completed.

## Overview

The lorcana-engine is a headless, deterministic game engine for Disney's Lorcana trading card game. It uses a data-driven approach with hybrid effect system (TOML definitions + Rhai scripting) to ensure flexibility for future card releases while maintaining type safety and performance.

## Core Design Principles

### 1. Event-Sourced Game State
- **Deterministic**: Same seed + actions = identical outcome
- **Replayable**: Complete game log for debugging and replays
- **Serializable**: Game state can be saved/loaded at any point
- **Authoritative**: Single source of truth with no conflicting state

### 2. Data-Driven Card Definitions
- **TOML-based**: Card definitions in human-readable TOML format
- **No code changes**: New cards can be added without recompiling
- **Version control friendly**: Easy to diff and review card changes
- **Hot-reload capable**: Card definitions can be reloaded without restart

### 3. Hybrid Effect System
- **Built-in effects**: Common effects (damage, draw, quest, etc.) as TOML
- **Scripted effects**: Complex mechanics via Rhai scripting
- **Extensible**: New effect types can be added to the engine
- **Type-safe**: Rust ensures effect validity at compile time

### 4. Headless Architecture
- **No UI coupling**: Engine has no knowledge of rendering or input
- **Event-driven**: Emits events for UI to consume
- **Multi-platform**: Can be embedded in web, desktop, mobile, or terminal
- **Testable**: Easy to test without UI dependencies

## Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                    UI Layer (External)                   │
│              (Web, Desktop, Mobile, Terminal)            │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                   API Layer                              │
│              (Action validation, Event emission)          │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                 Game Engine Core                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Rules      │  │    State     │  │   Effects    │  │
│  │   Engine     │  │   Manager    │  │   Executor   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │    Turn      │  │    Zone      │  │   Trigger    │  │
│  │   Manager    │  │   Manager    │  │   System     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│              Card Definition Layer                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Card       │  │   Effect     │  │   Script     │  │
│  │   Registry   │  │   Compiler   │  │   Engine     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│              Data Storage Layer                          │
│         (TOML card definitions, Game logs)                │
└─────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Game State

The game state will be the single source of truth containing all game information. It will include:
- Game metadata (ID, turn, phase, step, priority player, random seed)
- Player states (lore, inkwell, hand, deck, discard)
- Zone management (all Lorcana zones)
- Stack for effect resolution
- Event log for determinism and replay

*Implementation details to be defined during Phase 1 development.*

### 2. Zone System

Lorcana has multiple zones where cards can exist:
- Deck
- Hand
- Inkwell
- Field
- Discard
- Stack
- Banished

Each zone will have visibility rules and ownership tracking.

*Implementation details to be defined during Phase 1 development.*

### 3. Card Definition System

Cards are defined in TOML with a hybrid effect system:

```toml
[[cards]]
id = "lor-001"
name = "Mickey Mouse - Brave Little Tailor"
version = "1"
cost = 2
ink_type = "Sapphire"
card_type = "Character"
strength = 3
willpower = 3
quest_value = 2
classifications = ["Hero", "Mouse", "Prince"]

[[cards.keywords]]
name = "Rush"

[[cards.abilities]]
name = "Inspiring"
type = "triggered"
trigger = "etb"  # enters the battlefield

[[cards.abilities.effects]]
type = "draw"
target = "self"
value = 1
```

For complex effects, Rhai scripts can be used:

```toml
[[cards.abilities]]
name = "ComplexEffect"
type = "scripted"
script = "scripts/complex_ability.rhai"
```

### 4. Effect System

The effect system uses a hierarchical approach:

#### Built-in Effects (TOML)
- `draw`: Draw cards
- `damage`: Deal damage to characters
- `heal`: Restore willpower
- `quest`: Gain lore
- `exert`: Exert a character
- `ready`: Ready a character
- `move`: Move cards between zones
- `create`: Create tokens
- `counter`: Add counters
- `modify`: Modify card stats

#### Scripted Effects (Rhai)
Complex mechanics that don't fit built-in patterns:
- Multi-step effects with conditions
- Dynamic targeting based on game state
- Custom timing and priority interactions
- Replacement effects

### 5. Trigger System

Event-driven trigger system for card abilities. The system will support:
- Card enters/leaves play triggers
- Turn start/end triggers
- Phase start/end triggers
- Damage dealing triggers
- Quest completion triggers
- Custom triggers for unique mechanics

*Implementation details to be defined during Phase 3 development.*

### 6. Turn Structure

Lorcana's turn structure with phases and steps:
- **Beginning Phase**: Ready, Set, Draw steps
- **Main Phase**: Main actions (play cards, quest, challenge)
- **End Phase**: End of turn effects and cleanup

The system will track current turn, active player, current phase, step, and priority player.

*Implementation details to be defined during Phase 1 development.*

### 7. Event System

Comprehensive event system for UI updates and replays. Events will include:
- Game lifecycle events (start, end)
- Turn progression events
- Card movement events
- Effect resolution events
- State change events

*Implementation details to be defined during Phase 1 development.*

## Data Flow

### 1. Player Action Flow

```
Player Action
    ↓
API Layer (validate action format)
    ↓
Rules Engine (validate legality)
    ↓
State Manager (apply changes)
    ↓
Effect Executor (process effects)
    ↓
Trigger System (check triggers)
    ↓
Event Emitter (generate events)
    ↓
UI Update (consume events)
```

### 2. Card Resolution Flow

```
Card Played
    ↓
Check costs (ink, requirements)
    ↓
Move card to play zone
    ↓
Check for ETB triggers
    ↓
Resolve triggered abilities (stack if multiple)
    ↓
Apply effects
    ↓
Check state-based actions
    ↓
Emit events
```

## Card Definition Examples

### Simple Character with Built-in Effects

```toml
[[cards]]
id = "lor-002"
name = "Elsa - Snow Queen"
version = "1"
cost = 4
ink_type = "Amethyst"
card_type = "Character"
strength = 4
willpower = 4
quest_value = 3
classifications = ["Queen", "Frost"]

[[cards.abilities]]
name = "Frozen Shield"
type = "static"
effect_type = "protection"
condition = "weather"
```

### Complex Character with Scripted Effect

```toml
[[cards]]
id = "lor-003"
name = "Maleficent - Mistress of Evil"
version = "1"
cost = 6
ink_type = "Amethyst"
card_type = "Character"
strength = 5
willpower = 7
quest_value = 2
classifications = ["Villain", "Fairy"]

[[cards.abilities]]
name = "Curse of Sleeping Beauty"
type = "scripted"
script = "scripts/maleficent_curse.rhai"
```

### Action Card

```toml
[[cards]]
id = "lor-004"
name = "Let It Go"
version = "1"
cost = 3
ink_type = "Amethyst"
card_type = "Action"

[[cards.effects]]
type = "return_to_hand"
target = "opponent_character"
condition = "cost_less_than_4"
```

## Technology Stack

### Core Dependencies
- **serde**: Serialization/deserialization of game state and card definitions
- **toml**: Parsing card definitions
- **rhai**: Embedded scripting for complex effects
- **rand**: Deterministic random number generation
- **thiserror**: Error handling

### Optional Dependencies
- **uuid**: Unique identifiers for games, players, cards
- **chrono**: Timestamps for game logs
- **tracing**: Structured logging and instrumentation

## File Structure

```
lorcana-engine/
├── src/
│   ├── main.rs                   # CLI entry point
│   ├── lib.rs                    # Library exports and public API
│   ├── domain/                   # Core domain logic (no external dependencies)
│   │   ├── mod.rs
│   │   ├── game/                 # Game state, turn structure, zones
│   │   │   ├── mod.rs
│   │   │   ├── state.rs          # GameState and related types
│   │   │   ├── turn.rs           # Turn management
│   │   │   ├── zones.rs          # Zone management
│   │   │   └── events.rs         # Event system
│   │   ├── cards/                # Card definitions and registry
│   │   │   ├── mod.rs
│   │   │   ├── definition.rs     # Card definition types
│   │   │   ├── registry.rs       # Card registry
│   │   │   └── loader.rs         # Card loader
│   │   ├── effects/              # Effect system
│   │   │   ├── mod.rs
│   │   │   ├── executor.rs       # Effect execution
│   │   │   ├── builtin.rs        # Built-in effect implementations
│   │   │   └── trigger.rs       # Trigger system
│   │   └── types/                # Shared domain types
│   │       ├── mod.rs
│   │       ├── ids.rs            # Type-safe IDs
│   │       └── common.rs         # Common domain types
│   ├── infrastructure/           # External dependencies and adapters
│   │   ├── mod.rs
│   │   ├── parsing/              # TOML parsing
│   │   │   ├── mod.rs
│   │   │   └── toml.rs           # TOML parser implementation
│   │   ├── scripting/            # Rhai scripting integration
│   │   │   ├── mod.rs
│   │   │   └── rhai.rs           # Rhai engine wrapper
│   │   ├── random/               # Deterministic RNG
│   │   │   ├── mod.rs
│   │   │   └── rng.rs            # RNG implementation
│   │   └── serialization/        # Serde integration
│   │       ├── mod.rs
│   │       └── serde.rs          # Serialization helpers
│   ├── application/              # Application services and orchestration
│   │   ├── mod.rs
│   │   ├── engine/               # Game engine orchestration
│   │   │   ├── mod.rs
│   │   │   └── core.rs           # Core game engine
│   │   ├── rules/                # Rules validation
│   │   │   ├── mod.rs
│   │   │   ├── validator.rs      # Action validation
│   │   │   ├── lorcana.rs        # Lorcana-specific rules
│   │   │   └── engine.rs         # Rules engine
│   │   └── api/                  # Public API interface
│   │       ├── mod.rs
│   │       ├── actions.rs        # Action types
│   │       └── interface.rs      # API interface
│   └── shared/                   # Shared utilities
│       ├── mod.rs
│       ├── error.rs              # Error types
│       └── result.rs             # Result types
├── cards/                        # Card definitions
│   ├── set1.toml
│   ├── set2.toml
│   └── scripts/                  # Rhai scripts
│       ├── ability1.rhai
│       └── ability2.rhai
├── tests/
│   ├── integration/
│   │   └── game_scenarios.rs
│   └── unit/
│       ├── domain/
│       ├── infrastructure/
│       └── application/
├── examples/
│   ├── simple_game.rs
│   └── custom_card.rs
├── benches/                      # Performance benchmarks
├── docs/                         # Documentation
│   ├── architecture/             # Architecture documentation
│   │   └── ARCHITECTURE.md       # This file
│   ├── planning/                 # Implementation planning
│   │   └── IMPLEMENTATION_PLAN.md # Implementation roadmap
│   └── development/              # Development guides
│       └── CONTRIBUTING.md       # Contributing guidelines
```

## Implementation Phases

### Phase 1: Core Infrastructure
- Game state structure
- Zone system
- Turn management
- Event system
- Basic API interface

### Phase 2: Card System
- Card definition types
- TOML parser
- Card registry
- Basic card loading

### Phase 3: Effect System
- Built-in effect implementations
- Effect executor
- Trigger system
- Rhai integration

### Phase 4: Rules Engine
- Action validation
- Lorcana-specific rules
- Turn structure enforcement
- State-based actions

### Phase 5: Advanced Features
- Complex trigger interactions
- Replacement effects
- Timing system
- Priority and pass system

### Phase 6: Tooling
- Card validation tools
- Game log viewer
- Testing framework
- Benchmarking

## Future Extensibility

### New Card Types
- Add new `CardType` enum variants
- Extend zone system if needed
- Add new built-in effects

### New Mechanics
- Add new trigger events
- Extend effect system
- Add new scripting APIs

### Performance Optimizations
- Effect caching
- State diff optimization
- Parallel trigger evaluation
- WASM compilation for web

## Testing Strategy

### Unit Tests
- Individual component testing
- Effect system testing
- Trigger system testing

### Integration Tests
- Complete game scenarios
- Card interaction testing
- Turn structure testing

### Property Tests
- Determinism verification
- State consistency
- Event log integrity

### Golden Tests
- Known game states
- Expected event sequences
- Card definition validation

## Implementation Status

This architecture document represents the planned design for the lorcana-engine. As implementation progresses according to the [implementation plan](../planning/IMPLEMENTATION_PLAN.md), this document will be updated to reflect:

### Current Status
- **Phase 1**: Not started - Core infrastructure
- **Phase 2**: Not started - Card system
- **Phase 3**: Not started - Effect system
- **Phase 4**: Not started - Rules engine
- **Phase 5**: Not started - API layer
- **Phase 6**: Not started - Testing and validation

### Documentation Updates
As each phase is completed, the corresponding sections in this document will be updated with:
- Actual struct definitions and types used
- Implementation decisions and trade-offs
- Performance characteristics
- Discovered patterns and best practices
- Lessons learned during implementation

### Review Process
This architecture document should be reviewed and updated:
- After each major phase completion
- When significant architectural decisions are made
- When patterns emerge that differ from initial design
- Before releasing new versions