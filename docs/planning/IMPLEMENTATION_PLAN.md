# Lorcana Engine Implementation Plan

This implementation plan is based on the architecture defined in [../architecture/ARCHITECTURE.md](../architecture/ARCHITECTURE.md).

## Phase 1: Core Infrastructure (Foundation)

### 1.1 Project Structure & Dependencies
**Priority**: Critical
**Estimated Time**: 1-2 days

**Tasks**:
- [ ] Set up single-crate module structure with domain subdirectories
- [ ] Create module structure: `src/domain/`, `src/infrastructure/`, `src/application/`, `src/shared/`
- [ ] Add core dependencies (serde, toml, rhai, rand, thiserror, uuid)
- [ ] Set up basic testing framework
- [ ] Create example card definitions directory
- [ ] Set up module hierarchy with proper `mod.rs` files

**Dependencies**: None

**Deliverables**:
- Single-crate module structure in place
- Cargo.toml with all dependencies
- Basic test framework working
- Domain module hierarchy established

### 1.2 Core Types & Identifiers
**Priority**: Critical
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Define core type aliases (CardId, PlayerId, ZoneId, GameId)
- [ ] Implement UUID generation for unique identifiers
- [ ] Create basic error types and error handling
- [ ] Define Phase and Step enums for turn structure
- [ ] Define CardType, InkType, and other game enums

**Dependencies**: 1.1

**Deliverables**:
- `src/domain/types/mod.rs` with all core types
- Error handling system in `src/shared/error.rs`
- Comprehensive type safety

### 1.3 Game State Structure
**Priority**: Critical
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Implement GameState struct in `src/domain/game/state.rs`
- [ ] Implement PlayerState struct
- [ ] Implement Zone and ZoneType enums in `src/domain/game/zones.rs`
- [ ] Add serialization/deserialization for game state
- [ ] Implement game state cloning for testing
- [ ] Add state validation functions

**Dependencies**: 1.2

**Deliverables**:
- `src/domain/game/state.rs` with complete state structures
- Serializable game state
- State validation utilities

### 1.4 Event System
**Priority**: Critical
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Define GameEvent enum in `src/domain/game/events.rs`
- [ ] Implement event log in GameState
- [ ] Add event emission functions
- [ ] Implement event filtering and querying
- [ ] Add event serialization

**Dependencies**: 1.3

**Deliverables**:
- `src/domain/game/events.rs` with event system
- Event logging in game state
- Event query utilities

### 1.5 Turn Management
**Priority**: High
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Implement TurnManager in `src/domain/game/turn.rs`
- [ ] Define turn phases and steps
- [ ] Implement turn progression logic
- [ ] Add priority player tracking
- [ ] Implement turn-based event triggers

**Dependencies**: 1.3, 1.4

**Deliverables**:
- `src/domain/game/turn.rs` with turn management
- Turn progression working
- Priority system implemented

## Phase 2: Card System (Data Layer)

### 2.1 Card Definition Types
**Priority**: Critical
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Define CardDefinition struct
- [ ] Define Ability, Effect, and Trigger types
- [ ] Define keyword system
- [ ] Define classification system
- [ ] Add card metadata (rarity, set, etc.)

**Dependencies**: 1.2

**Deliverables**:
- `src/domain/cards/definition.rs` with all card types
- Type-safe card definition system

### 2.2 TOML Parser
**Priority**: Critical
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Implement TOML deserialization in `src/infrastructure/parsing/toml.rs`
- [ ] Add schema validation for card definitions
- [ ] Implement card definition merging (for sets)
- [ ] Add error handling for invalid definitions
- [ ] Create example card definitions

**Dependencies**: 2.1

**Deliverables**:
- `src/infrastructure/parsing/toml.rs` with TOML parsing
- Example card definitions
- Validation system

### 2.3 Card Registry
**Priority**: High
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Implement CardRegistry in `src/domain/cards/registry.rs`
- [ ] Add card lookup by ID
- [ ] Add card search functionality
- [ ] Implement card versioning
- [ ] Add registry serialization

**Dependencies**: 2.2

**Deliverables**:
- `src/domain/cards/registry.rs` with card registry
- Efficient card lookup
- Card search capabilities

### 2.4 Card Loader
**Priority**: High
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Implement card loading in `src/domain/cards/loader.rs`
- [ ] Add directory scanning for card files
- [ ] Implement hot-reload functionality
- [ ] Add card dependency resolution
- [ ] Create loader tests

**Dependencies**: 2.3

**Deliverables**:
- `src/domain/cards/loader.rs` with loading system
- Hot-reload capability
- Comprehensive loading tests

## Phase 3: Effect System (Logic Layer)

### 3.1 Effect Types & Built-in Effects
**Priority**: Critical
**Estimated Time**: 4-5 days

**Tasks**:
- [ ] Define Effect enum with all built-in types
- [ ] Implement draw effect
- [ ] Implement damage effect
- [ ] Implement heal effect
- [ ] Implement quest effect
- [ ] Implement exert/ready effects
- [ ] Implement move card effect
- [ ] Implement create token effect
- [ ] Implement modify stats effect

**Dependencies**: 2.1

**Deliverables**:
- `src/domain/effects/builtin.rs` with all built-in effects
- Comprehensive effect library

### 3.2 Effect Executor
**Priority**: Critical
**Estimated Time**: 4-5 days

**Tasks**:
- [ ] Implement EffectExecutor in `src/domain/effects/executor.rs`
- [ ] Add effect validation before execution
- [ ] Implement effect targeting system
- [ ] Add effect cost payment
- [ ] Implement effect resolution
- [ ] Add effect undo/rollback for testing

**Dependencies**: 3.1

**Deliverables**:
- `src/domain/effects/executor.rs` with effect execution
- Targeting system
- Cost payment system

### 3.3 Trigger System
**Priority**: High
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Define TriggerEvent enum
- [ ] Implement Trigger struct in `src/domain/effects/trigger.rs`
- [ ] Add trigger registration system
- [ ] Implement trigger condition evaluation
- [ ] Add trigger queue management
- [ ] Implement trigger resolution

**Dependencies**: 1.4, 3.2

**Deliverables**:
- `src/domain/effects/trigger.rs` with trigger system
- Event-driven trigger system
- Trigger queue management

### 3.4 Rhai Integration
**Priority**: Medium
**Estimated Time**: 5-7 days

**Tasks**:
- [ ] Set up Rhai engine integration in `src/infrastructure/scripting/rhai.rs`
- [ ] Define game state API for scripts
- [ ] Implement script loading and caching
- [ ] Add script execution in effect system
- [ ] Implement script sandboxing
- [ ] Create example scripts
- [ ] Add script error handling

**Dependencies**: 3.2

**Deliverables**:
- `src/infrastructure/scripting/rhai.rs` with Rhai integration
- Script API documentation
- Example scripts

## Phase 4: Rules Engine (Validation Layer)

### 4.1 Action Types
**Priority**: High
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Define PlayerAction enum
- [ ] Define all action types (play card, use ability, quest, etc.)
- [ ] Add action validation types
- [ ] Implement action serialization

**Dependencies**: 1.2

**Deliverables**:
- `src/application/api/actions.rs` with action types
- Comprehensive action system

### 4.2 Action Validator
**Priority**: Critical
**Estimated Time**: 4-5 days

**Tasks**:
- [ ] Implement ActionValidator in `src/application/rules/validator.rs`
- [ ] Add turn-based validation
- [ ] Add resource validation (ink, etc.)
- [ ] Add zone validation
- [ ] Add timing validation
- [ ] Add targeting validation

**Dependencies**: 4.1, 1.5

**Deliverables**:
- `src/application/rules/validator.rs` with validation system
- Comprehensive action validation

### 4.3 Rules Engine
**Priority**: Critical
**Estimated Time**: 5-7 days

**Tasks**:
- [ ] Implement RulesEngine in `src/application/rules/engine.rs`
- [ ] Add action processing pipeline
- [ ] Implement state-based actions
- [ ] Add replacement effect handling
- [ ] Implement prevention effects
- [ ] Add continuous effect evaluation

**Dependencies**: 4.2, 3.3

**Deliverables**:
- `src/application/rules/engine.rs` with rules engine
- Complete rule enforcement

### 4.4 Lorcana-Specific Rules
**Priority**: High
**Estimated Time**: 4-5 days

**Tasks**:
- [ ] Implement ink system rules in `src/application/rules/lorcana.rs`
- [ ] Implement questing rules
- [ ] Implement challenging rules
- [ ] Implement shift mechanics
- [ ] Implement location rules
- [ ] Implement song mechanics

**Dependencies**: 4.3

**Deliverables**:
- `src/application/rules/lorcana.rs` with Lorcana rules
- Complete Lorcana rule implementation

## Phase 5: API Layer (Interface)

### 5.1 Public API
**Priority**: High
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Define public API interface
- [ ] Implement game creation
- [ ] Implement action submission
- [ ] Implement state queries
- [ ] Implement event subscription
- [ ] Add API documentation

**Dependencies**: 4.3

**Deliverables**:
- `src/application/api/interface.rs` with public API
- Clean API interface
- API documentation

### 5.2 CLI / Example Client
**Priority**: Medium
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Implement basic CLI in `src/main.rs`
- [ ] Add game state visualization
- [ ] Add action input system
- [ ] Add event display
- [ ] Create example game scenarios

**Dependencies**: 5.1

**Deliverables**:
- `src/main.rs` with CLI client
- Example game scenarios in `examples/`

## Phase 6: Testing & Validation

### 6.1 Unit Tests
**Priority**: Critical
**Estimated Time**: 5-7 days

**Tasks**:
- [ ] Write unit tests for all components
- [ ] Test effect system
- [ ] Test trigger system
- [ ] Test validation system
- [ ] Test state management
- [ ] Achieve >80% code coverage

**Dependencies**: All previous phases

**Deliverables**:
- Comprehensive unit test suite
- High code coverage

### 6.2 Integration Tests
**Priority**: High
**Estimated Time**: 5-7 days

**Tasks**:
- [ ] Write integration tests for game scenarios
- [ ] Test complete game flows
- [ ] Test card interactions
- [ ] Test turn structure
- [ ] Test complex trigger chains

**Dependencies**: 5.1

**Deliverables**:
- Integration test suite
- Game scenario tests

### 6.3 Property Tests
**Priority**: Medium
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Set up proptest or similar
- [ ] Test determinism properties
- [ ] Test state consistency
- [ ] Test event log integrity
- [ ] Test card definition validation

**Dependencies**: 6.1

**Deliverables**:
- Property test suite
- Determinism verification

### 6.4 Golden Tests
**Priority**: Medium
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Create golden test scenarios
- [ ] Test known game states
- [ ] Test expected event sequences
- [ ] Add regression tests

**Dependencies**: 6.2

**Deliverables**:
- Golden test suite
- Regression prevention

## Phase 7: Tooling & Documentation

### 7.1 Card Validation Tools
**Priority**: Medium
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Implement card definition validator
- [ ] Add card balance checker
- [ ] Implement card dependency checker
- [ ] Create card documentation generator

**Dependencies**: 2.4

**Deliverables**:
- Card validation tools
- Documentation generator

### 7.2 Game Log Viewer
**Priority**: Low
**Estimated Time**: 2-3 days

**Tasks**:
- [ ] Implement game log parser
- [ ] Add log visualization
- [ ] Implement replay functionality
- [ ] Add log export features

**Dependencies**: 1.4

**Deliverables**:
- Game log viewer
- Replay system

### 7.3 Documentation
**Priority**: High
**Estimated Time**: 3-4 days

**Tasks**:
- [ ] Write API documentation
- [ ] Write card definition guide
- [ ] Write scripting guide
- [ ] Write contribution guide
- [ ] Add examples and tutorials

**Dependencies**: All previous phases

**Deliverables**:
- Complete documentation
- Tutorials and examples

## Phase 8: Advanced Features (Future)

### 8.1 Performance Optimizations
**Priority**: Low
**Estimated Time**: 5-7 days

**Tasks**:
- [ ] Implement effect caching
- [ ] Optimize state diffing
- [ ] Add parallel trigger evaluation
- [ ] Implement WASM compilation
- [ ] Benchmark and profile

**Dependencies**: All previous phases

### 8.2 Advanced Mechanics
**Priority**: Low
**Estimated Time**: 7-10 days

**Tasks**:
- [ ] Implement complex timing system
- [ ] Add priority and pass system
- [ ] Implement multiplayer support
- [ ] Add tournament mode
- [ ] Implement deck building validation

**Dependencies**: 7.3

### 8.3 AI Integration
**Priority**: Low
**Estimated Time**: 10-14 days

**Tasks**:
- [ ] Design AI interface
- [ ] Implement basic AI opponent
- [ ] Add game tree search
- [ ] Implement evaluation heuristics
- [ ] Add AI configuration

**Dependencies**: 5.1

## Total Estimated Time

- **Phase 1**: 10-15 days (Core Infrastructure)
- **Phase 2**: 10-14 days (Card System)
- **Phase 3**: 16-21 days (Effect System)
- **Phase 4**: 15-22 days (Rules Engine)
- **Phase 5**: 6-8 days (API Layer)
- **Phase 6**: 15-21 days (Testing)
- **Phase 7**: 7-10 days (Tooling)
- **Phase 8**: 22-31 days (Advanced Features)

**Minimum Viable Product**: Phases 1-5 (57-80 days)
**Production Ready**: Phases 1-7 (79-111 days)
**Full Featured**: All phases (101-142 days)

## Dependencies Between Phases

```
Phase 1 (Core)
    ↓
Phase 2 (Cards)
    ↓
Phase 3 (Effects)
    ↓
Phase 4 (Rules)
    ↓
Phase 5 (API)
    ↓
Phase 6 (Testing)
    ↓
Phase 7 (Tooling)
    ↓
Phase 8 (Advanced)
```

## Success Criteria

### Phase 1-5 (MVP)
- [ ] Can create a game state
- [ ] Can load card definitions
- [ ] Can execute basic effects
- [ ] Can validate actions
- [ ] Can play a complete simple game

### Phase 6 (Production Ready)
- [ ] >80% code coverage
- [ ] All integration tests passing
- [ ] Determinism verified
- [ ] No known critical bugs

### Phase 7 (Complete)
- [ ] Full documentation
- [ ] Card validation tools
- [ ] Game log viewer
- [ ] Example scenarios

## Risk Assessment

### Technical Risks
- **Complex trigger interactions**: Medium risk - mitigate with comprehensive testing
- **Script performance**: Medium risk - mitigate with performance profiling
- **State consistency**: Low risk - mitigate with property tests
- **Determinism**: Low risk - mitigate with seed management

### Timeline Risks
- **Scope creep**: High risk - mitigate by strict phase boundaries
- **Complex card mechanics**: Medium risk - mitigate with scripting system
- **Testing complexity**: Medium risk - mitigate with golden tests

## Next Steps

1. **Immediate**: Start Phase 1.1 (Project Structure)
2. **Short-term**: Complete Phase 1 (Core Infrastructure)
3. **Medium-term**: Complete Phases 1-3 (Core + Cards + Effects)
4. **Long-term**: Complete all phases for production-ready engine

## Notes

- This plan assumes one developer working full-time
- Adjust timelines based on team size and experience
- Regular reviews after each phase to adjust plan
- Prioritize MVP (Phases 1-5) for initial release
- Advanced features can be added incrementally

## Evolution to Multi-Crate Workspace

This implementation plan uses a single-crate structure with domain modules. If the project grows and needs the benefits of multi-crate organization, the following domains can be extracted to separate crates:

- `crates/lorcana-domain/` - Core domain logic (game, cards, effects, types)
- `crates/lorcana-infrastructure/` - External dependencies (parsing, scripting, random)
- `crates/lorcana-application/` - Application services (engine, rules, API)
- `crates/lorcana-cli/` - CLI interface

The module structure is designed to make this extraction straightforward when needed.