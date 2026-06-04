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
    font-size: 0.7rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .count {
    padding-inline: 0.35rem;
    border-radius: 1rem;
    background: var(--surface-2);
    color: var(--text);
  }

  .row {
    display: flex;
    gap: var(--gap);
    align-items: center;
    justify-content: center;
    min-block-size: calc(var(--card-w) * 7 / 5);
    padding-block: 0.25rem;
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
