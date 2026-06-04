<script lang="ts">
  import type { CardDef, GameView, PlayerView } from '../engine';
  import PlayerSide from './PlayerSide.svelte';

  /** Lore needed to win a game of Lorcana. */
  const WIN_LORE = 20;

  let {
    view,
    cards,
    viewpoint = 0,
  }: { view: GameView; cards: ReadonlyMap<number, CardDef>; viewpoint?: number } = $props();

  const you = $derived(view.players.find((p) => p.id === viewpoint) ?? view.players[0]);
  const opponent = $derived(view.players.find((p) => p.id !== viewpoint) ?? view.players[1]);

  const loreFor = (p: PlayerView | undefined): number => p?.lore ?? 0;
  const lorePct = (p: PlayerView | undefined): number =>
    Math.min(100, (loreFor(p) / WIN_LORE) * 100);

  const youActive = $derived(!!you && view.activePlayer === you.id);
  const oppActive = $derived(!!opponent && view.activePlayer === opponent.id);

  const finished = $derived(view.status === 'Finished');
  const youWon = $derived(finished && !!you && view.winners.includes(you.id));
  // Humanised phase label: avoid raw "P0's Main / Main" jargon.
  const phaseLabel = $derived(
    view.phase === view.step ? view.phase : `${view.phase} · ${view.step}`,
  );
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
    <div class="hud">
      <div class="seat" class:active={youActive}>
        <span class="seat-name">You</span>
        <span class="lore"
          ><span class="lore-val">{loreFor(you)}</span><span class="lore-max">/{WIN_LORE}</span
          ></span
        >
        <span class="lore-track"
          ><span class="lore-fill" style="inline-size: {lorePct(you)}%"></span></span
        >
      </div>

      <div class="match">
        {#if finished}
          <span class="result">{youWon ? 'You win' : 'Opponent wins'}</span>
        {:else}
          <span class="turn">Turn {view.turnNumber}</span>
          <span class="phase">{phaseLabel}</span>
        {/if}
      </div>

      <div class="seat opp" class:active={oppActive}>
        <span class="seat-name">Opponent</span>
        <span class="lore"
          ><span class="lore-val">{loreFor(opponent)}</span><span class="lore-max">/{WIN_LORE}</span
          ></span
        >
        <span class="lore-track"
          ><span class="lore-fill" style="inline-size: {lorePct(opponent)}%"></span></span
        >
      </div>
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
    gap: 0.25rem;
    padding-block: 0.4rem;
    /* A single faint gold hairline marks the line between the two seats. */
    border-block: 1px solid transparent;
    border-image: linear-gradient(90deg, transparent, var(--divider), transparent) 1;
  }

  /* Scoreboard: lore (the win condition) is the focal point; the active seat
     is clearly emphasised. */
  .hud {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: clamp(1rem, 4vw, 3rem);
    inline-size: min(100%, 640px);
  }

  .seat {
    display: grid;
    grid-template-columns: auto auto;
    align-items: baseline;
    gap: 0.2rem 0.5rem;
    padding: 0.25rem 0.6rem;
    border-radius: var(--radius);
    transition: background 0.2s;
  }

  .seat.opp {
    justify-items: end;
    text-align: end;
  }

  /* Active seat: a soft warm wash + brighter name — clearly whose turn it is. */
  .seat.active {
    background: color-mix(in srgb, var(--illuminary-gold) 12%, transparent);
  }

  .seat-name {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .seat.active .seat-name {
    color: var(--accent);
  }

  .lore {
    justify-self: end;
    font-weight: 800;
    line-height: 1;
    color: var(--lore);
  }

  .seat.opp .lore {
    justify-self: start;
    order: -1;
  }

  .lore-val {
    font-size: 1.6rem;
  }

  .lore-max {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }

  .lore-track {
    grid-column: 1 / -1;
    block-size: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--kelp) 55%, transparent);
    overflow: hidden;
  }

  .lore-fill {
    display: block;
    block-size: 100%;
    background: var(--lore);
    transition: inline-size 0.3s ease;
  }

  .match {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1rem;
    white-space: nowrap;
  }

  .turn {
    font-weight: 700;
    font-size: 0.95rem;
    color: var(--text);
  }

  .phase {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
  }

  .result {
    font-weight: 800;
    color: var(--accent);
    text-transform: uppercase;
    letter-spacing: 0.08em;
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
