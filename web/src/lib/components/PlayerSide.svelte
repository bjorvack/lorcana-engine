<script lang="ts">
  import { facedownCards, toDisplayCard, type DisplayCard } from '../cardModel';
  import type { CardDef, PlayerView } from '../engine';
  import InkwellMeter from './InkwellMeter.svelte';
  import Lane from './Lane.svelte';
  import Pile from './Pile.svelte';

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

  const play = $derived(player.play.map((c) => toDisplayCard(c, cards)));
  const hand = $derived<DisplayCard[]>(
    revealHand ? player.hand.map((c) => toDisplayCard(c, cards)) : facedownCards(player.handCount),
  );
</script>

<section class="side" class:mirrored class:active>
  <div class="piles">
    <Pile label="Deck" count={player.deckCount} />
    <Pile label="Discard" count={player.discard.length} faceDown={false} />
    <InkwellMeter inkwell={player.inkwell} />
  </div>

  <div class="play">
    <Lane label="Play" cards={play} empty="No cards in play" />
  </div>

  <div class="hand">
    <Lane label={revealHand ? 'Hand' : 'Hand (hidden)'} cards={hand} empty="Empty hand" />
  </div>
</section>

<style>
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    padding: var(--gap);
    border-radius: var(--radius);
  }

  /* The opponent's side is the vertical mirror of yours. */
  .side.mirrored {
    flex-direction: column-reverse;
  }

  .side.active {
    background: oklch(80% 0.15 85 / 8%);
    outline: 1px solid oklch(80% 0.15 85 / 30%);
  }

  .piles {
    display: flex;
    gap: 1rem;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .play {
    background: var(--bg-felt);
    border-radius: var(--radius);
    padding-inline: var(--gap);
  }
</style>
