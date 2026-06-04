<script lang="ts">
  import type { InkwellView } from '../engine';

  let { inkwell }: { inkwell: InkwellView } = $props();

  const pips = $derived(
    Array.from({ length: inkwell.total }, (_, i) => ({ id: i, ready: i < inkwell.ready })),
  );
</script>

<section class="inkwell" aria-label="Inkwell">
  <header>
    <span class="name">Ink</span>
    <span class="count">{inkwell.ready}/{inkwell.total}</span>
  </header>
  <div class="pips" role="img" aria-label={`${inkwell.ready} of ${inkwell.total} ink ready`}>
    {#each pips as pip (pip.id)}
      <span class="pip" class:ready={pip.ready}></span>
    {/each}
  </div>
</section>

<style>
  .inkwell {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  header {
    display: flex;
    gap: 0.4rem;
    align-items: center;
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

  .pips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem;
    max-inline-size: 9rem;
  }

  .pip {
    inline-size: 0.7rem;
    block-size: 0.95rem;
    border-radius: 0.2rem;
    background: var(--surface-2);
    border: 1px solid var(--border);
  }

  .pip.ready {
    background: var(--ink);
    border-color: var(--ink);
  }
</style>
