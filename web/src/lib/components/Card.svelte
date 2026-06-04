<script lang="ts">
  import type { DisplayCard } from '../cardModel';

  let { card }: { card: DisplayCard } = $props();
</script>

<article
  class="card"
  class:exerted={card.exerted}
  class:drying={card.drying}
  class:lethal={card.lethal}
  class:location={card.isLocation}
  title={card.name}
>
  {#if card.facedown}
    <div class="back" aria-label="Facedown card"></div>
  {:else}
    {#if card.image}
      <img class="art" src={card.image} alt={card.name} loading="lazy" decoding="async" />
    {:else}
      <div class="art placeholder"><span>{card.name}</span></div>
    {/if}

    <span class="chip cost">{card.cost}</span>

    {#if card.strength !== undefined}
      <span class="chip strength" title="Strength">{card.strength}</span>
    {/if}
    {#if card.willpower !== undefined}
      <span class="chip willpower" title="Willpower (remaining/printed)">
        {card.willpowerRemaining}/{card.willpower}
      </span>
    {/if}
    {#if card.lore !== undefined}
      <span class="chip lore" title="Lore">{card.lore}</span>
    {/if}
    {#if card.damage > 0}
      <span class="chip damage" title="Damage">{card.damage}</span>
    {/if}
    {#if card.underCount > 0}
      <span class="chip under" title="Cards underneath">+{card.underCount}</span>
    {/if}
  {/if}
</article>

<style>
  .card {
    position: relative;
    inline-size: var(--card-w);
    aspect-ratio: 5 / 7;
    border-radius: 0.4rem;
    border: 1px solid oklch(0% 0 0 / 40%);
    background: var(--surface);
    overflow: hidden;
    flex: 0 0 auto;
    transition:
      transform 180ms ease,
      filter 180ms ease,
      outline-color 180ms ease;
    outline: 2px solid transparent;
  }

  .card.location {
    aspect-ratio: 7 / 5;
    inline-size: calc(var(--card-w) * 1.4);
  }

  .card.exerted {
    transform: rotate(90deg) scale(0.82);
  }

  .card.drying {
    filter: brightness(0.7) saturate(0.7);
  }

  .card.drying::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, oklch(70% 0.12 250 / 28%), transparent 60%);
  }

  .card.lethal {
    outline-color: var(--danger);
  }

  .art {
    inline-size: 100%;
    block-size: 100%;
    object-fit: cover;
    display: block;
  }

  .placeholder {
    display: grid;
    place-items: center;
    padding: 0.3rem;
    text-align: center;
    font-size: 0.55rem;
    line-height: 1.1;
    color: var(--muted);
    background: var(--surface-2);
  }

  .back {
    block-size: 100%;
    background:
      repeating-linear-gradient(45deg, oklch(40% 0.07 280) 0 6px, oklch(34% 0.06 280) 6px 12px);
  }

  .chip {
    position: absolute;
    min-inline-size: 1.1rem;
    padding-inline: 0.2rem;
    font-size: 0.62rem;
    font-weight: 700;
    text-align: center;
    border-radius: 0.5rem;
    background: oklch(15% 0.02 280 / 85%);
    line-height: 1.4;
  }

  .cost {
    inset-block-start: 0.15rem;
    inset-inline-start: 0.15rem;
    color: var(--ink);
  }
  .strength {
    inset-block-end: 0.15rem;
    inset-inline-start: 0.15rem;
  }
  .willpower {
    inset-block-end: 0.15rem;
    inset-inline-end: 0.15rem;
  }
  .lore {
    inset-block-start: 0.15rem;
    inset-inline-end: 0.15rem;
    color: var(--lore);
  }
  .damage {
    inset-block-start: 50%;
    inset-inline-start: 50%;
    translate: -50% -50%;
    color: oklch(95% 0.02 25);
    background: var(--danger);
  }
  .under {
    inset-block-start: 0.15rem;
    inset-inline-start: 50%;
    translate: -50% 0;
    color: var(--muted);
  }
</style>
