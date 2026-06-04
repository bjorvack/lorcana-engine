<script lang="ts">
  import type { DisplayCard } from '../cardModel';

  let { card }: { card: DisplayCard } = $props();

  let isHovered = $state(false);
  let mouseX = $state(0);
  let mouseY = $state(0);

  function handleMouseMove(e: MouseEvent) {
    mouseX = e.clientX;
    mouseY = e.clientY;
  }
</script>

<article
  class="card"
  class:exerted={card.exerted}
  class:drying={card.drying}
  class:lethal={card.lethal}
  class:location={card.isLocation}
  title={card.name}
  onmouseenter={() => (isHovered = true)}
  onmouseleave={() => (isHovered = false)}
  onmousemove={handleMouseMove}
>
  {#if card.facedown}
    <img class="back" src="/back.webp" alt="Card back" loading="lazy" decoding="async" />
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

{#if isHovered && !card.facedown && card.image}
  <div class="card-preview" style="left: {mouseX + 15}px; top: {mouseY + 15}px;">
    <img src={card.image} alt={card.name} loading="eager" decoding="sync" />
    <div class="preview-stats">
      <span class="preview-cost">{card.cost}</span>
      {#if card.strength !== undefined}
        <span class="preview-strength">{card.strength}</span>
      {/if}
      {#if card.willpower !== undefined}
        <span class="preview-willpower">{card.willpowerRemaining}/{card.willpower}</span>
      {/if}
      {#if card.lore !== undefined}
        <span class="preview-lore">{card.lore}</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .card {
    position: relative;
    inline-size: var(--card-w);
    aspect-ratio: 5 / 7;
    border-radius: 0.4rem;
    border: 1px solid oklch(0% 0 0deg / 40%);
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
    background: linear-gradient(135deg, oklch(70% 0.12 250deg / 28%), transparent 60%);
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
    inline-size: 100%;
    block-size: 100%;
    object-fit: cover;
    display: block;
  }

  .chip {
    position: absolute;
    min-inline-size: 1.1rem;
    padding-inline: 0.2rem;
    font-size: 0.62rem;
    font-weight: 700;
    text-align: center;
    border-radius: 0.5rem;
    background: oklch(15% 0.02 280deg / 85%);
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
    color: oklch(95% 0.02 25deg);
    background: var(--danger);
  }

  .under {
    inset-block-start: 0.15rem;
    inset-inline-start: 50%;
    translate: -50% 0;
    color: var(--muted);
  }

  .card-preview {
    position: fixed;
    z-index: 9999;
    pointer-events: none;
    inline-size: 280px;
    background: var(--surface);
    border-radius: 0.5rem;
    padding: 0.5rem;
    box-shadow: 0 8px 32px oklch(0% 0 0deg / 40%);
    border: 1px solid var(--border);
    max-inline-size: calc(100vw - 30px);
    max-block-size: calc(100vh - 30px);
    overflow: auto;
  }

  .card-preview img {
    inline-size: 100%;
    aspect-ratio: 5 / 7;
    object-fit: cover;
    border-radius: 0.3rem;
    display: block;
  }

  .preview-stats {
    display: flex;
    gap: 0.5rem;
    margin-block-start: 0.5rem;
    flex-wrap: wrap;
  }

  .preview-stats span {
    padding: 0.25rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.8rem;
    font-weight: 700;
    background: var(--surface-2);
  }

  .preview-cost {
    color: var(--ink);
  }

  .preview-strength {
    color: var(--text);
  }

  .preview-willpower {
    color: var(--text);
  }

  .preview-lore {
    color: var(--lore);
  }
</style>
