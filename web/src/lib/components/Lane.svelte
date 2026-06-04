<script lang="ts">
  import type { DisplayCard } from '../cardModel';
  import Card from './Card.svelte';

  let {
    label,
    cards,
    empty = '—',
  }: { label: string; cards: DisplayCard[]; empty?: string } = $props();
</script>

<section class="lane" aria-label={label}>
  <header>
    <span class="name">{label}</span>
    <span class="count">{cards.length}</span>
  </header>
  <div class="row">
    {#each cards as card (card.instanceId)}
      <Card {card} />
    {:else}
      <p class="empty">{empty}</p>
    {/each}
  </div>
</section>

<style>
  .lane {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-inline-size: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.66rem;
    font-weight: 600;
    color: color-mix(in srgb, var(--illuminary-gold) 80%, var(--parchment));
    text-transform: uppercase;
    letter-spacing: 0.12em;
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
    justify-content: center;
    /* Fixed height: one full card tall, always. Cards never wrap; extra
       cards scroll horizontally so the zone never changes size. */
    block-size: var(--card-h);
    flex-wrap: nowrap;
    padding-block: 0.25rem;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
  }

  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
