# Lorcana Engine Architecture

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

The `GameState` struct is the single source of truth containing all game information:

```rust
pub struct GameState {
    // Game metadata
    pub game_id: GameId,
    pub turn: u32,
    pub phase: Phase,
    pub step: Step,
    pub priority_player: PlayerId,
    pub random_seed: u64,

    // Players
    pub players: HashMap<PlayerId, PlayerState>,

    // Zones
    pub zones: HashMap<ZoneId, Zone>,

    // Stack (for effect resolution)
    pub stack: Vec<StackItem>,

    // Event log (for determinism and replay)
    pub event_log: Vec<GameEvent>,
}

pub struct PlayerState {
    pub player_id: PlayerId,
    pub lore: u32,
    pub inkwell: Vec<CardId>,
    pub hand: Vec<CardId>,
    pub deck: Vec<CardId>,
    pub discard: Vec<CardId>,
}
```

### 2. Zone System

Lorcana has multiple zones where cards can exist:

```rust
pub enum ZoneType {
    Deck,
    Hand,
    Inkwell,
    Field,
    Discard,
    Stack,
    Banished,
}

pub struct Zone {
    pub zone_id: ZoneId,
    pub zone_type: ZoneType,
    pub owner: PlayerId,
    pub cards: Vec<CardId>,
    pub visibility: ZoneVisibility,
}
```

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

Event-driven trigger system for card abilities:

```rust
pub enum TriggerEvent {
    CardEntersPlay { card_id: CardId, zone: ZoneId },
    CardLeavesPlay { card_id: CardId, zone: ZoneId },
    TurnStart { player_id: PlayerId },
    TurnEnd { player_id: PlayerId },
    PhaseStart { phase: Phase },
    PhaseEnd { phase: Phase },
    DamageDealt { source: CardId, target: CardId, amount: u32 },
    QuestCompleted { character_id: CardId, player_id: PlayerId },
    // ... more triggers
}

pub struct Trigger {
    pub id: TriggerId,
    pub card_id: CardId,
    pub event_type: TriggerEvent,
    pub condition: Option<EffectCondition>,
    pub effect: Effect,
    pub optional: bool,
}
```

### 6. Turn Structure

Lorcana's turn structure with phases and steps:

```rust
pub enum Phase {
    Beginning,
    Main,
    End,
}

pub enum Step {
    Ready,
    Set,
    Draw,
}

pub struct TurnManager {
    pub current_turn: u32,
    pub current_player: PlayerId,
    pub current_phase: Phase,
    pub current_step: Option<Step>,
    pub priority_player: PlayerId,
}
```

### 7. Event System

Comprehensive event system for UI updates and replays:

```rust
pub enum GameEvent {
    GameStarted { players: Vec<PlayerId> },
    TurnStarted { player_id: PlayerId, turn: u32 },
    PhaseChanged { phase: Phase },
    CardPlayed { card_id: CardId, player_id: PlayerId },
    CardMoved { card_id: CardId, from_zone: ZoneId, to_zone: ZoneId },
    EffectTriggered { trigger_id: TriggerId },
    DamageDealt { source: CardId, target: CardId, amount: u32 },
    LoreGained { player_id: PlayerId, amount: u32 },
    QuestCompleted { character_id: CardId, player_id: PlayerId },
    GameEnded { winner: PlayerId },
    // ... more events
}
```

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