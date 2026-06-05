<script lang="ts">
  import Board from './lib/components/Board.svelte';
  import { Engine, type GameView } from './lib/engine';
  import { preloadImages } from './lib/preload';
  // Imported (not a `/brand/logo.png` string) so the URL is rewritten for the
  // GitHub Pages project subpath instead of 404-ing at the site root.
  import logo from './lib/assets/logo.png';

  let engine = $state<Engine | null>(null);
  let view = $state<GameView | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let seed = $state(1);
  let auto = $state(false);
  // Progress of the card-art preload, or null when not preloading.
  let preload = $state<{ done: number; total: number } | null>(null);

  async function newGame(): Promise<void> {
    loading = true;
    error = null;
    auto = false;
    preload = null;
    try {
      const created = await Engine.create(BigInt(seed));
      // Preload every deck card's art before revealing the board so nothing
      // pops in blank as cards are drawn/played.
      const urls = [...created.cards.values()].map((c) => c.image);
      preload = { done: 0, total: new Set(urls.filter(Boolean)).size };
      await preloadImages(urls, (done, total) => (preload = { done, total }));
      // Start at the opening: both players have drawn 7 and kept their hands
      // (mulligans auto-resolved), the fields are empty, and it's turn 1.
      // Use Step / Auto-play to drive the game from here.
      engine = created;
      view = created.view();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
      preload = null;
    }
  }

  function step(): void {
    if (!engine) return;
    engine.step();
    view = engine.view();
  }

  // Auto-play: tick while enabled and the game can still progress.
  $effect(() => {
    if (!auto || !engine) return;
    const handle = setInterval(() => {
      if (!engine) return;
      const advanced = engine.step();
      view = engine.view();
      if (!advanced || engine.finished) auto = false;
    }, 400);
    return () => clearInterval(handle);
  });

  void newGame();
</script>

<header class="topbar">
  <div class="brand">
    <img class="logo" src={logo} alt="Disney Lorcana" />
    <span class="subtitle">Board Viewer</span>
  </div>
  <div class="controls">
    <label>
      Seed
      <input type="number" min="0" bind:value={seed} />
    </label>
    <button onclick={() => void newGame()} disabled={loading}>New game</button>
    <button onclick={step} disabled={!engine || loading || auto}>Step</button>
    <button onclick={() => (auto = !auto)} disabled={!engine || loading} aria-pressed={auto}>
      {auto ? 'Pause' : 'Auto-play'}
    </button>
  </div>
</header>

<main>
  {#if error}
    <p class="error" role="alert">Failed to start: {error}</p>
  {:else if loading || !view}
    <div class="loading" role="status">
      {#if preload && preload.total > 0}
        <span>Loading card art… {preload.done}/{preload.total}</span>
        <span class="progress">
          <span class="progress-fill" style="inline-size: {(preload.done / preload.total) * 100}%"
          ></span>
        </span>
      {:else}
        <span>Loading engine…</span>
      {/if}
    </div>
  {:else if engine}
    <Board {view} cards={engine.cards} />
  {/if}
</main>

<style>
  .topbar {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 1.2rem;
    border-block-end: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    backdrop-filter: blur(10px);
    box-shadow: var(--shadow-soft);
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.7rem;
  }

  .logo {
    block-size: 2.1rem;
    inline-size: auto;
    display: block;
    filter: drop-shadow(0 1px 3px rgb(0 0 0 / 55%));
  }

  .subtitle {
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.18em;
    color: var(--muted);
  }

  .controls {
    display: flex;
    gap: 0.6rem;
    align-items: center;
  }

  label {
    display: inline-flex;
    gap: 0.35rem;
    align-items: center;
    font-size: 0.8rem;
    color: var(--muted);
  }

  input {
    inline-size: 5rem;
    font: inherit;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.3rem 0.5rem;
  }

  button[aria-pressed='true'] {
    background: var(--accent);
    color: var(--kelp);
    border-color: var(--accent);
  }

  main {
    flex: 1;
    min-block-size: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0.5rem 1rem 0;
    /* Your hand bleeds to the bottom of the screen; #app clips at the viewport. */
    overflow: visible;
  }

  .error {
    color: var(--danger);
  }

  .loading {
    margin-block: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .progress {
    inline-size: min(60vw, 18rem);
    block-size: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--kelp) 55%, transparent);
    overflow: hidden;
  }

  .progress-fill {
    display: block;
    block-size: 100%;
    background: var(--lore);
    transition: inline-size 0.15s ease;
  }
</style>
