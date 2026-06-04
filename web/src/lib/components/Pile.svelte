<script lang="ts">
  let {
    label,
    count,
    faceDown = true,
  }: { label: string; count: number; faceDown?: boolean } = $props();
</script>

<section class="pile" aria-label={`${label}: ${count}`}>
  <div class="stack" class:back={faceDown} class:empty={count === 0}>
    <span class="count">{count}</span>
  </div>
  <span class="name">{label}</span>
</section>

<style>
  .pile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2rem;
  }

  .stack {
    inline-size: var(--card-w);
    aspect-ratio: 5 / 7;
    display: grid;
    place-items: center;
    border-radius: 0.4rem;
    border: 1px solid var(--border);
    background: var(--surface-2);
  }

  .stack.back {
    background: repeating-linear-gradient(
      45deg,
      oklch(40% 0.07 280deg) 0 6px,
      oklch(34% 0.06 280deg) 6px 12px
    );
  }

  .stack.empty {
    opacity: 0.4;
    border-style: dashed;
  }

  .count {
    font-weight: 700;
    font-size: 0.95rem;
    padding-inline: 0.4rem;
    border-radius: 0.5rem;
    background: oklch(15% 0.02 280deg / 75%);
  }

  .name {
    font-size: 0.68rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
