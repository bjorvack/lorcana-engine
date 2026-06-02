# Documentation Reminders

## Architecture Document Updates

**IMPORTANT**: The `docs/architecture/ARCHITECTURE.md` file is a living document that must be updated as implementation progresses.

### When to Update ARCHITECTURE.md

Update the architecture document after completing each implementation phase:

- **After Phase 1 (Core Infrastructure)**: Add actual struct definitions for:
  - GameState and related types
  - Zone system implementation
  - Turn management
  - Event system

- **After Phase 2 (Card System)**: Add actual implementation details for:
  - Card definition types and TOML schema
  - Actual card definition examples from the first set
  - TOML parser implementation
  - Card registry structure
  - Card loading mechanism
  - Real card definitions in the `cards/` directory

- **After Phase 3 (Effect System)**: Add actual implementation details for:
  - Built-in effect implementations
  - Effect executor structure
  - Trigger system implementation
  - Rhai integration details

- **After Phase 4 (Rules Engine)**: Add actual implementation details for:
  - Action validation system
  - Lorcana-specific rules implementation
  - Rules engine structure

- **After Phase 5 (API Layer)**: Add actual implementation details for:
  - Public API interface
  - Action types
  - API design decisions

### What to Add

When updating the architecture document, include:
- Actual struct definitions and types used
- Implementation decisions and trade-offs made
- Performance characteristics discovered
- Any patterns that emerged during implementation
- Lessons learned
- Any deviations from the original architecture design

### Review Process

Review and update the architecture document:
- After each major phase completion
- When significant architectural decisions are made
- When patterns emerge that differ from initial design
- Before releasing new versions

## Implementation Plan Updates

The `docs/planning/IMPLEMENTATION_PLAN.md` should also be updated:
- Mark tasks as completed as they are finished
- Adjust time estimates based on actual experience
- Add new tasks discovered during implementation
- Update dependencies as the architecture evolves

## Card Definitions Development

**IMPORTANT**: Card definitions will be developed while implementing Phase 2 (Card System). The architecture document currently contains high-level descriptions of the card definition system, but specific TOML examples and schemas will be created during implementation.

### Card Definition Development Process

When working on Phase 2:
1. Start with basic card properties and structure
2. Develop TOML schema incrementally
3. Create example card definitions as the parser is developed
4. Add real Lorcana card definitions once the system is working
5. Update architecture document with actual examples

### Card Definition Files

Card definitions will be stored in the `cards/` directory:
- `cards/set1.toml` - First set of card definitions
- `cards/set2.toml` - Second set of card definitions
- `cards/scripts/` - Rhai scripts for complex effects

### Documentation Updates

As card definitions are developed:
- Add actual TOML examples to architecture document
- Document the final TOML schema
- Include real card examples from the first Lorcana set
- Update card definition system section with implementation details

## Contributing Guide Updates

The `docs/development/CONTRIBUTING.md` should be updated:
- As new development patterns emerge
- When new tools or processes are added
- When contribution guidelines change
- As the codebase structure evolves

---

**Last Updated**: 2025-06-02
**Purpose**: Ensure documentation stays synchronized with implementation progress