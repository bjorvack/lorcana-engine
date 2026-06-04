<script lang="ts">
  import type { CardDef, GameView, PlayerView } from '../engine';
  import PlayerSide from './PlayerSide.svelte';

  let {
    view,
    cards,
    viewpoint = 0,
  }: { view: GameView; cards: ReadonlyMap<number, CardDef>; viewpoint?: number } = $props();

  const you = $derived(view.players.find((p) => p.id === viewpoint) ?? view.players[0]);
  const opponent = $derived(view.players.find((p) => p.id !== viewpoint) ?? view.players[1]);

  const loreFor = (p: PlayerView | undefined): number => p?.lore ?? 0;
</script>

<div class="board">
  {#if opponent}
    <PlayerSide
      player={opponent}
      {cards}
      revealHand={false}
      mirrored
      active={view.activePlayer === opponent.id}
    />
  {/if}

  <div class="center" role="status">
    <div class="score">
      <span class="lore you">You · {loreFor(you)} lore</span>
      <span class="vs">vs</span>
      <span class="lore opp">Opponent · {loreFor(opponent)} lore</span>
    </div>
    <div class="phase">
      <span class="turn">Turn {view.turnNumber}</span>
      <span class="dot">•</span>
      <span>P{view.activePlayer}'s {view.phase} / {view.step}</span>
      {#if view.status !== 'Playing'}
        <span class="dot">•</span>
        <span class="status">{view.status}</span>
      {/if}
    </div>
    {#if view.pending}
      <p class="pending" title={view.pending}>Awaiting: {view.pending}</p>
    {/if}
  </div>

  {#if you}
    <PlayerSide player={you} {cards} revealHand={true} active={view.activePlayer === you.id} />
  {/if}
</div>

<style>
  .board {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    inline-size: min(100%, 1280px);
    flex: 1;
    min-block-size: 0;
    padding: clamp(0.5rem, 1.2vw, 1rem);
    padding-block-end: 0;
    /* No frame: the table is grouped by a faint tint + soft shadow only. */
    background: color-mix(in srgb, var(--surface) 38%, transparent);
    backdrop-filter: blur(7px);
    /* Let your hand spill out the bottom to the screen edge. */
    overflow: visible;
  }

  /* Each player's mat shares the height equally; the centre strip is fixed. */
  .board > :global(.side) {
    flex: 1 1 0;
    min-block-size: 0;
  }

  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1rem;
    padding-block: 0.35rem;
    /* A single faint gold hairline marks the line between the two seats. */
    border-block: 1px solid transparent;
    border-image: linear-gradient(90deg, transparent, var(--divider), transparent) 1;
  }

  .score {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    font-weight: 700;
  }

  .lore.you {
    color: var(--lore);
  }

  .lore.opp {
    color: var(--accent);
  }

  .vs {
    color: var(--muted);
    font-weight: 400;
  }

  .phase {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .dot {
    opacity: 0.5;
  }

  .status {
    color: var(--accent);
    font-weight: 700;
  }

  .pending {
    margin: 0;
    font-size: 0.75rem;
    color: var(--accent);
    max-inline-size: 40ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
