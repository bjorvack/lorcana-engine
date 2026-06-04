<script lang="ts">
  import type { DisplayCard } from '../cardModel';
  import Card from './Card.svelte';

  let {
    label,
    cards,
    empty = '—',
    variant = 'full',
    clip = false,
  }: {
    label: string;
    cards: DisplayCard[];
    empty?: string;
    /** How each card renders: square art crop (`art`) or whole card (`full`). */
    variant?: 'full' | 'art';
    /** Show only the top of the cards (used for the hand to save space). */
    clip?: boolean;
  } = $props();
</script>

<section class="lane" class:art={variant === 'art'} class:clip aria-label={label}>
  <header>
    <span class="name">{label}</span>
    <span class="count">{cards.length}</span>
  </header>
  <div class="row">
    {#each cards as card (card.instanceId)}
      <Card {card} {variant} />
    {:else}
      <p class="empty">{empty}</p>
    {/each}
  </div>
</section>

<style>
  .lane {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-inline-size: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.6rem;
    font-weight: 600;
    color: color-mix(in srgb, var(--illuminary-gold) 80%, var(--parchment));
    text-transform: uppercase;
    letter-spacing: 0.1em;
    line-height: 1.1;
  }

  .count {
    padding-inline: 0.4rem;
    padding-block: 0.05rem;
    border-radius: 1rem;
    background: color-mix(in srgb, var(--illuminary-gold) 16%, transparent);
    border: 1px solid var(--border);
    color: var(--parchment);
    font-variant-numeric: tabular-nums;
  }

  .row {
    display: flex;
    gap: var(--gap);
    align-items: center;
    justify-content: safe center;

    /* Fixed height so the zone never changes size as cards come and go; extra
       cards scroll horizontally. `full` reserves a whole card, `art` a square. */
    block-size: var(--card-h);
    flex-wrap: nowrap;
    padding-block: 0.1rem;
    overflow: auto hidden;
    scrollbar-width: thin;
  }

  .lane.art .row {
    block-size: var(--card-w);
  }

  /* Clipped lane (the hand): only the top of each card shows, freeing space
     for the rest of the mat. The full card is available on hover. */
  .lane.clip .row {
    block-size: calc(var(--card-h) * 0.5);
    align-items: flex-start;
    overflow: visible hidden;
  }

  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
