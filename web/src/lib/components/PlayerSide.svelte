<script lang="ts">
  import { facedownCards, toDisplayCard, type DisplayCard } from '../cardModel';
  import type { CardDef, PlayerView } from '../engine';
  import Lane from './Lane.svelte';
  import Pile from './Pile.svelte';
  import Card from './Card.svelte';

  let {
    player,
    cards,
    revealHand,
    mirrored = false,
    active = false,
  }: {
    player: PlayerView;
    cards: ReadonlyMap<number, CardDef>;
    revealHand: boolean;
    mirrored?: boolean;
    active?: boolean;
  } = $props();

  let showDiscardModal = $state(false);

  const play = $derived(player.play.map((c) => toDisplayCard(c, cards)));

  // Split play zone into regions
  const characters = $derived(play.filter((c) => c.type === 'Character'));
  const locations = $derived(play.filter((c) => c.type === 'Location'));
  const items = $derived(play.filter((c) => c.type === 'Item'));
  // Hand cards carry the engine's `facedown` condition (hidden from the
  // opponent); when showing your own hand, reveal their faces.
  const hand = $derived<DisplayCard[]>(
    revealHand
      ? player.hand.map((c) => ({ ...toDisplayCard(c, cards), facedown: false }))
      : facedownCards(player.handCount),
  );

  // Create ink cards: ready ink (facedown, not exerted), exerted ink (facedown, exerted)
  const readyInkCards = $derived<DisplayCard[]>(
    Array.from({ length: player.inkwell.ready }, (_, i) => ({
      instanceId: -1000 - i,
      defId: -1,
      name: 'Ink',
      image: undefined,
      cost: 0,
      type: 'Ink',
      exerted: false,
      drying: false,
      facedown: true,
      damage: 0,
      strength: undefined,
      willpower: undefined,
      lore: undefined,
      willpowerRemaining: undefined,
      lethal: false,
      isLocation: false,
      underCount: 0,
    })),
  );

  const exertedInkCards = $derived<DisplayCard[]>(
    Array.from({ length: player.inkwell.exerted }, (_, i) => ({
      instanceId: -2000 - i,
      defId: -1,
      name: 'Ink',
      image: undefined,
      cost: 0,
      type: 'Ink',
      exerted: true,
      drying: false,
      facedown: true,
      damage: 0,
      strength: undefined,
      willpower: undefined,
      lore: undefined,
      willpowerRemaining: undefined,
      lethal: false,
      isLocation: false,
      underCount: 0,
    })),
  );

  // Top card of discard (faceup)
  const topDiscardCard = $derived<DisplayCard | undefined>(
    player.discard.length > 0
      ? { ...toDisplayCard(player.discard[player.discard.length - 1], cards), facedown: false }
      : undefined,
  );

  // All discard cards for modal (faceup)
  const discardCards = $derived<DisplayCard[]>(
    player.discard.map((c) => ({ ...toDisplayCard(c, cards), facedown: false })),
  );
</script>

<section class="side" class:active class:mirrored>
  <!-- Play area, split into character / location / item regions -->
  <div class="left-zone">
    <div class="play-regions">
      <div class="play-region characters">
        <Lane label="Characters" cards={characters} empty="No characters" variant="art" />
      </div>
      <div class="play-bottom-row">
        <div class="play-region locations">
          <Lane label="Locations" cards={locations} empty="No locations" variant="art" />
        </div>
        <div class="play-region items">
          <Lane label="Items" cards={items} empty="No items" variant="art" />
        </div>
      </div>
    </div>
  </div>

  <!-- Deck + Discard piles -->
  <div class="right-zone">
    <div class="piles-vertical">
      <Pile label="Deck" count={player.deckCount} />
      <div
        class="discard-pile"
        onclick={() => (showDiscardModal = true)}
        onkeydown={(e) => e.key === 'Enter' && (showDiscardModal = true)}
        role="button"
        tabindex="0"
        title="Click to view discard pile"
      >
        <span class="pile-label">Discard</span>
        {#if topDiscardCard}
          <Card card={topDiscardCard} variant="art" />
        {:else}
          <div class="empty-discard">Empty</div>
        {/if}
        <span class="pile-count">{player.discard.length}</span>
      </div>
    </div>
  </div>

  <!-- Ink zone -->
  <div class="ink-zone">
    <div class="ink-group">
      <div class="ink-label">
        <span>Ready</span>
        <span class="ink-count">{player.inkwell.ready}/{player.inkwell.total}</span>
      </div>
      <div class="ink-cards" style="--ink-count-n: {readyInkCards.length}">
        {#each readyInkCards as card (card.instanceId)}
          <Card {card} variant="art" />
        {/each}
      </div>
    </div>
    <div class="ink-group">
      <div class="ink-label">
        <span>Exerted</span>
        <span class="ink-count">{player.inkwell.exerted}</span>
      </div>
      <div class="ink-cards" style="--ink-count-n: {exertedInkCards.length}">
        {#each exertedInkCards as card (card.instanceId)}
          <Card {card} variant="art" />
        {/each}
      </div>
    </div>
  </div>

  <!-- Hand -->
  <div class="hand-zone">
    <Lane label={revealHand ? 'Hand' : 'Hand (hidden)'} cards={hand} empty="Empty hand" clip />
  </div>
</section>

{#if showDiscardModal}
  <div
    class="modal-backdrop"
    onclick={() => (showDiscardModal = false)}
    onkeydown={(e) => e.key === 'Escape' && (showDiscardModal = false)}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="modal-content" role="document">
      <div class="modal-header">
        <h2>Discard Pile</h2>
        <button
          class="close-button"
          onclick={() => (showDiscardModal = false)}
          aria-label="Close"
          type="button">✕</button
        >
      </div>
      <div class="modal-body">
        {#if discardCards.length > 0}
          <div class="discard-grid">
            {#each discardCards as card (card.instanceId)}
              <Card {card} />
            {/each}
          </div>
        {:else}
          <p class="empty-message">Discard pile is empty</p>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .side {
    display: grid;
    grid-template-columns: 1fr auto;
    /* minmax(0, …) lets the play row actually shrink to fit instead of forcing
       its min-content height (which would push the hand off-screen). */
    grid-template-rows: minmax(0, 1fr) auto auto;
    gap: var(--gap);
    padding: var(--gap);
    border-radius: var(--radius);
    min-block-size: 0;
    overflow: hidden;
  }

  .side.active {
    background: oklch(80% 0.15 85deg / 8%);
    outline: 1px solid oklch(80% 0.15 85deg / 30%);
  }

  /*
   * The opponent's mat is a 180° point-mirror of yours: hand at the very top,
   * then ink, then the play area nearest the centre line — with the piles on
   * their left (our right→left swap) and the play rows flipped. The cards
   * themselves stay upright so they remain readable from our seat.
   */
  .side.mirrored {
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto minmax(0, 1fr);
  }

  .left-zone {
    grid-column: 1;
    grid-row: 1;
    display: flex;
    align-items: flex-end;
  }

  /* Mirrored: play sits in the bottom-right, nearest the centre line. */
  .side.mirrored .left-zone {
    grid-column: 2;
    grid-row: 3;
    align-items: flex-start;
  }

  /* Mirrored: piles move to the opponent's left (our left), bottom row. */
  .side.mirrored .right-zone {
    grid-column: 1;
    grid-row: 3;
    align-items: flex-start;
  }

  /* Mirrored: ink sits above the play area (which moved to column 2). */
  .side.mirrored .ink-zone {
    grid-column: 2;
    grid-row: 2;
  }

  /* Mirrored: hand at the very top. */
  .side.mirrored .hand-zone {
    grid-row: 1;
  }

  /* Mirrored: flip the play rows so characters are nearest the centre line,
     and swap locations/items horizontally to complete the point-mirror. */
  .side.mirrored .play-regions {
    flex-direction: column-reverse;
  }

  .side.mirrored .play-bottom-row {
    direction: rtl;
  }

  .side.mirrored .play-bottom-row > * {
    direction: ltr;
  }

  .play-regions {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    background: var(--bg-felt);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--gap);
    flex: 1;
    min-block-size: 0;
    overflow: hidden;
    box-shadow:
      var(--shadow-panel),
      inset 0 1px 0 color-mix(in srgb, var(--illuminary-gold) 14%, transparent);
  }

  .play-region {
    flex: 1;
    min-block-size: 0;
    padding: calc(var(--gap) * 0.5);
    border-radius: calc(var(--radius) - 0.2rem);
    background: color-mix(in srgb, var(--kelp) 30%, transparent);
    border: 1px solid color-mix(in srgb, var(--illuminary-gold) 10%, transparent);
    overflow: hidden;
  }

  .play-bottom-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--gap);
    flex: 1;
    min-block-size: 0;
  }

  .play-region.characters {
    flex: 1;
  }

  .play-region.locations,
  .play-region.items {
    flex: 1;
  }

  .right-zone {
    grid-column: 2;
    grid-row: 1;
    display: flex;
    align-items: flex-end;
  }

  .piles-vertical {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .ink-zone {
    grid-column: 1;
    grid-row: 2;
    display: flex;
    justify-content: center;
    align-items: center;
    gap: clamp(0.75rem, 2vw, 2rem);
    padding-block: 0.35rem;
    padding-inline: var(--gap);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--surface) 45%, transparent);
    border: 1px solid color-mix(in srgb, var(--illuminary-gold) 10%, transparent);
  }

  .ink-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    flex: 0 1 auto;
    min-inline-size: 0;
    overflow: hidden;
  }

  .ink-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.66rem;
    font-weight: 600;
    color: color-mix(in srgb, var(--illuminary-gold) 80%, var(--parchment));
    text-transform: uppercase;
    letter-spacing: 0.12em;
  }

  .ink-count {
    padding-inline: 0.4rem;
    padding-block: 0.15rem;
    border-radius: 1rem;
    background: color-mix(in srgb, var(--illuminary-gold) 16%, transparent);
    border: 1px solid var(--border);
    color: var(--parchment);
    font-weight: 700;
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }

  .ink-cards {
    --card-w: var(--ink-card-w);
    /* Normal spacing between ink cards before they start to overlap. */
    --ink-step-full: calc(var(--ink-card-w) + 0.25rem);
    /* Step needed to fit every card inside the fixed band; clamp to the full
       step so a few cards sit side-by-side and a banked inkwell fans/stacks. */
    --ink-step: min(
      var(--ink-step-full),
      calc((var(--ink-band) - var(--ink-card-w)) / max(var(--ink-count-n) - 1, 1))
    );

    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    justify-content: center;
    block-size: var(--ink-card-w);
    max-inline-size: var(--ink-band);
    padding-inline: 0.25rem;
  }

  /* Overlap cards by shifting each one back by (card width − step). When the
     step equals a full card+gap there is no overlap; tighter steps fan them. */
  .ink-cards > :global(.card) {
    margin-inline-start: calc(var(--ink-step) - var(--ink-card-w));
  }

  .ink-cards > :global(.card):first-child {
    margin-inline-start: 0;
  }

  .hand-zone {
    grid-column: 1 / -1;
    grid-row: 3;
    justify-self: center;
  }

  /* Modal styles */

  .discard-pile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    cursor: pointer;
    padding: 0.5rem;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
    border: 1px solid var(--border);
    transition:
      background 0.2s,
      border-color 0.2s,
      box-shadow 0.2s;
  }

  .discard-pile:hover {
    background: var(--surface-3);
    border-color: var(--border-strong);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--illuminary-gold) 25%, transparent);
  }

  .discard-pile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .pile-label {
    font-size: 0.62rem;
    font-weight: 600;
    color: color-mix(in srgb, var(--illuminary-gold) 80%, var(--parchment));
    text-transform: uppercase;
    letter-spacing: 0.12em;
  }

  .pile-count {
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--parchment);
    font-variant-numeric: tabular-nums;
  }

  .empty-discard {
    inline-size: var(--card-w);
    aspect-ratio: 1 / 1;
    background: var(--surface-3);
    border-radius: 0.4rem;
    display: grid;
    place-items: center;
    font-size: 0.6rem;
    color: var(--muted);
  }

  /* Modal styles */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: oklch(0% 0 0deg / 70%);
    display: grid;
    place-items: center;
    z-index: 1000;
    padding: 1rem;
  }

  .modal-content {
    background: var(--surface);
    border-radius: var(--radius);
    max-inline-size: 90vw;
    max-block-size: 90vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 40px oklch(0% 0 0deg / 30%);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    border-block-end: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
  }

  .close-button {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: var(--muted);
    padding: 0.25rem;
    line-height: 1;
    border-radius: 0.25rem;
    transition:
      color 0.2s,
      background 0.2s;
  }

  .close-button:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .modal-body {
    padding: 1rem;
    overflow-y: auto;
  }

  .discard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 1rem;
    justify-items: center;
  }

  .empty-message {
    margin: 0;
    text-align: center;
    color: var(--muted);
    font-size: 0.9rem;
  }
</style>
