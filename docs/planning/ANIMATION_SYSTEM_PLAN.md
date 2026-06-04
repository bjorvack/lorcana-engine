# Animation System Plan for Lorcana Board Viewer

## Overview
Add smooth FLIP animations for card movements between zones (ink ↔ play, hand ↔ play, play ↔ discard, etc.) and rotation animations for exertion changes.

## Research Summary

### Animation Library Choice: `svelte-auto-animate`
**Selected over alternatives:**
- **Svelte built-in `flip`**: Has known glitches in Svelte 5 with transitions, especially with outro animations
- **AutoAnimate**: Adds enter/exit transitions which are overkill; heavier dependency
- **svelte-auto-animate**: ✅ Lightweight fork focused only on FLIP, allows custom Svelte transitions, designed for Svelte

**Installation:** `npm install svelte-auto-animate`

## State Tracking System Design

### Current Architecture
- Board viewer is a static reflection of WASM game state
- Each `step()` call provides a new `GameView`
- No tracking of state changes between steps

### Proposed State Tracking

#### Data Structures
```typescript
interface ZoneChange {
  instanceId: number;
  fromZone: Zone;     // Where the card was
  toZone: Zone;       // Where the card is now
  changeType: 'move' | 'enter' | 'exit';
}

interface CardAnimationState {
  instanceId: number;
  previousZone?: Zone;
  currentZone: Zone;
  wasExerted: boolean;
  isExerted: boolean;
  needsMoveAnimation: boolean;
  needsRotateAnimation: boolean;
}
```

#### Zone Mapping
```typescript
type Zone = 'hand' | 'play' | 'discard' | 'ink-ready' | 'ink-exerted' | 'deck';

function getCardZone(card: CardView, player: PlayerView): Zone {
  if (player.hand.some(c => c.instanceId === card.instanceId)) return 'hand';
  if (player.play.some(c => c.instanceId === card.instanceId)) return 'play';
  if (player.discard.some(c => c.instanceId === card.instanceId)) return 'discard';
  // Ink tracking would need additional state from engine
  return 'unknown';
}
```

#### State Comparison Algorithm
```typescript
function compareStates(prev: GameView, current: GameView): CardAnimationState[] {
  const changes: CardAnimationState[] = [];

  for (const player of current.players) {
    const prevPlayer = prev.players.find(p => p.id === player.id);
    if (!prevPlayer) continue;

    // Track all cards across zones
    const allCurrentCards = new Map([
      ...player.hand.map(c => [c.instanceId, { card: c, zone: 'hand' as Zone }]),
      ...player.play.map(c => [c.instanceId, { card: c, zone: 'play' as Zone }]),
      ...player.discard.map(c => [c.instanceId, { card: c, zone: 'discard' as Zone }]),
    ]);

    const allPrevCards = new Map([
      ...prevPlayer.hand.map(c => [c.instanceId, { card: c, zone: 'hand' as Zone }]),
      ...prevPlayer.play.map(c => [c.instanceId, { card: c, zone: 'play' as Zone }]),
      ...prevPlayer.discard.map(c => [c.instanceId, { card: c, zone: 'discard' as Zone }]),
    ]);

    // Detect movements
    for (const [instanceId, current] of allCurrentCards) {
      const prev = allPrevCards.get(instanceId);
      if (!prev) {
        // Card entered (new instance)
        changes.push({
          instanceId,
          currentZone: current.zone,
          wasExerted: false,
          isExerted: !current.card.ready,
          needsMoveAnimation: true,
          needsRotateAnimation: !current.card.ready,
        });
      } else if (prev.zone !== current.zone) {
        // Card moved between zones
        changes.push({
          instanceId,
          previousZone: prev.zone,
          currentZone: current.zone,
          wasExerted: !prev.card.ready,
          isExerted: !current.card.ready,
          needsMoveAnimation: true,
          needsRotateAnimation: prev.card.ready !== current.card.ready,
        });
      } else if (prev.card.ready !== current.card.ready) {
        // Card exertion changed within same zone
        changes.push({
          instanceId,
          currentZone: current.zone,
          wasExerted: !prev.card.ready,
          isExerted: !current.card.ready,
          needsMoveAnimation: false,
          needsRotateAnimation: true,
        });
      }
    }
  }

  return changes;
}
```

## Card Identity Tracking

### Challenge: Ink Cards
Ink cards are synthetic (created in JS, not from engine) and don't have real instanceIds.

### Solution
```typescript
// Generate stable instanceIds for ink cards
function getInkInstanceId(playerIndex: number, type: 'ready' | 'exerted', index: number): number {
  // Use negative IDs that won't conflict with real cards
  const base = playerIndex === 0 ? -1000 : -2000;
  const offset = type === 'ready' ? 0 : 1000;
  return base + offset + index;
}

// Track ink card count changes
function detectInkChanges(prev: PlayerView, current: PlayerView): CardAnimationState[] {
  const changes: CardAnimationState[] = [];

  // Ready ink changes
  const readyDiff = current.inkwell.ready - prev.inkwell.ready;
  if (readyDiff > 0) {
    // Ink became ready (exerted → ready)
    for (let i = 0; i < readyDiff; i++) {
      changes.push({
        instanceId: getInkInstanceId(current.id, 'ready', current.inkwell.ready - i),
        previousZone: 'ink-exerted',
        currentZone: 'ink-ready',
        wasExerted: true,
        isExerted: false,
        needsMoveAnimation: true,
        needsRotateAnimation: true,
      });
    }
  } else if (readyDiff < 0) {
    // Ink was exerted (ready → exerted)
    for (let i = 0; i < -readyDiff; i++) {
      changes.push({
        instanceId: getInkInstanceId(current.id, 'exerted', current.inkwell.exerted - i),
        previousZone: 'ink-ready',
        currentZone: 'ink-exerted',
        wasExerted: false,
        isExerted: true,
        needsMoveAnimation: true,
        needsRotateAnimation: true,
      });
    }
  }

  return changes;
}
```

## Animation System Architecture

### Component Structure
```
App.svelte
├── Engine (handles WASM)
├── AnimationTracker (NEW - tracks state changes)
└── Board.svelte
    └── PlayerSide.svelte
        ├── AnimatedZone (NEW - wraps each zone with FLIP)
        │   ├── Lane (for hand/play/discard)
        │   └── InkZone
        └── Card.svelte (with rotation transitions)
```

### AnimationTracker Component
```typescript
// AnimationTracker.svelte
<script lang="ts">
  import { flip } from 'svelte-auto-animate';

  let { previousView, currentView }: {
    previousView: GameView | null;
    currentView: GameView;
  } = $props();

  const animations = $derived(
    previousView ? compareStates(previousView, currentView) : []
  );

  // Provide animation context to child components
  setContext('animations', animations);
</script>

<slot />
```

### AnimatedZone Component
```typescript
// AnimatedZone.svelte
<script lang="ts">
  import { flip } from 'svelte-auto-animate';
  import { getContext } from 'svelte';

  const animations = getContext('animations');

  let { zone, children }: {
    zone: Zone;
    children: Snippet;
  } = $props();
</script>

<div class="animated-zone" use:flip>
  {@render children()}
</div>

<style>
  .animated-zone {
    /* Ensure stable positioning for FLIP */
    position: relative;
  }
</style>
```

### Card Component with Rotation
```typescript
// Card.svelte - add rotation transition
<script lang="ts">
  import { flip, crossfade } from 'svelte/transition';

  const [send, receive] = crossfade({
    duration: 300,
    easing: (t) => t * (2 - t), // ease-out
  });

  let { card, animationState }: {
    card: DisplayCard;
    animationState?: CardAnimationState;
  } = $props();

  const shouldRotate = $derived(
    animationState?.needsRotateAnimation ?? false
  );
</script>

<article
  class="card"
  class:exerted={card.exerted}
  transition:rotate={shouldRotate}
  in:fly={{ duration: 300 }}
  out:fly={{ duration: 300 }}
  animate:flip
>
  <!-- card content -->
</article>

<style>
  .card {
    transition: transform 300ms ease-out;
  }

  .card.exerted {
    transform: rotate(90deg) scale(0.82);
  }
</style>
```

## Implementation Steps

### Phase 1: Foundation
1. Install `svelte-auto-animate`
2. Create `AnimationTracker` component
3. Add state tracking to Engine class (store previous view)
4. Implement `compareStates()` function
5. Add ink card instanceId generation

### Phase 2: Zone Wrappers
6. Create `AnimatedZone` component
7. Wrap each zone (hand, play regions, discard, ink) with `AnimatedZone`
8. Test basic FLIP animations with dummy data

### Phase 3: Card Animations
9. Add rotation transitions to Card component
10. Connect animation state from tracker to cards
11. Implement zone-specific animations (different durations/easing)

### Phase 4: Integration
12. Connect AnimationTracker to Engine step cycle
13. Test with real game state changes
14. Fine-tune animation timings and easing

### Phase 5: Polish
15. Add animation preferences (enable/disable)
16. Performance optimization (debounce, virtualization for large zones)
17. Accessibility (respect `prefers-reduced-motion`)

## Testing Strategy

### Unit Tests
- Test `compareStates()` with known state transitions
- Test zone detection logic
- Test ink card instanceId generation

### Integration Tests
- Test AnimationTracker with mock game states
- Test AnimatedZone with DOM manipulation
- Test Card rotation transitions

### Visual Tests
- Playwright tests to verify animations trigger correctly
- Screenshot comparisons for animation endpoints
- Performance profiling (measure animation frame rates)

### Manual Testing Checklist
- [ ] Card moves from hand to play
- [ ] Card moves from play to discard
- [ ] Card exertion changes in place
- [ ] Ink card ready ↔ exerted transitions
- [ ] Multiple simultaneous animations
- [ ] Animation with `prefers-reduced-motion`

## Edge Cases

### 1. Card Creation/Deletion
- **Issue**: New cards don't have previous position for FLIP
- **Solution**: Use `in:fly` transition for new cards, no FLIP

### 2. Simultaneous Zone Changes
- **Issue**: Multiple cards moving at once can cause FLIP conflicts
- **Solution**: Stagger animations or use AutoAnimate's built-in handling

### 3. Ink Card Synthetic InstanceIds
- **Issue**: Ink cards are created/destroyed each render
- **Solution**: Use stable negative IDs, track count changes

### 4. Large Zone Sizes
- **Issue**: Animating 50+ cards simultaneously causes performance issues
- **Solution**: Virtualization, limit concurrent animations, or disable for large zones

### 5. Rapid State Changes
- **Issue**: Quick successive steps interrupt ongoing animations
- **Solution**: Animation queue, cancel previous animations, or skip intermediate states

## Performance Considerations

### Memory
- Store only previous state (not full history)
- Clean up animation state after completion
- Use weak references where possible

### CPU
- Limit concurrent animations (max 10-15 at once)
- Use `will-change` sparingly (only during active animation)
- Debounce rapid state changes

### Bundle Size
- `svelte-auto-animate`: ~3KB gzipped
- No additional dependencies needed
- Animation code: ~2KB additional TypeScript

### Accessibility
- Respect `prefers-reduced-motion` media query
- Provide animation toggle in settings
- Ensure animations don't interfere with keyboard navigation

## Dependencies

### New Dependencies
- `svelte-auto-animate` - FLIP animations

### Existing Dependencies (No Changes)
- Svelte 5
- Vitest (for testing)
- Playwright (for visual tests)

## Timeline Estimate

- **Phase 1**: 2-3 hours
- **Phase 2**: 2-3 hours
- **Phase 3**: 3-4 hours
- **Phase 4**: 2-3 hours
- **Phase 5**: 2-3 hours

**Total**: 11-16 hours

## Design Decisions (Confirmed)

1. **Animation Durations**: Context-aware
   - Hand→play: 200ms (feels like playing a card)
   - Play→discard: 350ms (feels like resolution)
   - Ink exertion: 150ms (feels like tapping)
   - Other: 250ms default

2. **Animation Easing**: Movement-specific
   - Hand→play: `ease-out-back` (slight overshoot for "snap" feel)
   - Play→discard: `ease-in-out` (gentler resolution)
   - Ink exertion: `ease-out` (quick tap)
   - Other: `ease-out`

3. **Animation Scope**: Visible zones + deck draws + mulligans
   - Animate: hand, play (characters/locations/items), discard, ink, deck→hand (draws), hand→deck (mulligans)
   - Don't animate: Other hidden zone movements

4. **Performance Threshold**: Dynamic frame rate monitoring
   - Monitor animation frame rate in real-time
   - Disable animations if drops below 30fps
   - Adaptive, works well on all devices

5. **Animation Preferences**: Always on
   - Animations always enabled
   - No user toggle
   - Ignore `prefers-reduced-motion` system preference